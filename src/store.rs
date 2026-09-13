use std::{
    path::{Path, PathBuf},
    str::FromStr,
    sync::Arc,
    time::Duration,
};

use rusqlite::{Connection, OptionalExtension, Transaction, TransactionBehavior, params};
use serde_json::{Value, json};
use thiserror::Error;
use time::{OffsetDateTime, format_description::well_known::Rfc3339};
use uuid::Uuid;

use crate::domain::{
    Board, CreateThreadResponse, Event, ListThreadsInput, Page, Post, ReplyResponse, SearchInput,
    ThreadDetail, ThreadRecord, ThreadStatus, UpdateThreadResponse, ValidCreateThread, ValidReply,
    ValidUpdateThread,
};

const GENERAL_BOARD_ID: &str = "general";

#[derive(Debug, Error)]
pub(crate) enum StoreError {
    #[error("resource not found")]
    NotFound,
    #[error("thread is closed")]
    ThreadClosed,
    #[error("database error: {0}")]
    Database(#[from] rusqlite::Error),
    #[error("stored data is invalid: {0}")]
    InvalidStoredData(String),
}

#[derive(Clone, Debug)]
pub(crate) struct Store {
    path: Arc<PathBuf>,
}

impl Store {
    pub(crate) fn open(path: impl AsRef<Path>) -> Result<Self, StoreError> {
        let path = path.as_ref().to_path_buf();
        if let Some(parent) = path
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty())
        {
            std::fs::create_dir_all(parent).map_err(|error| {
                StoreError::InvalidStoredData(format!("cannot create database directory: {error}"))
            })?;
        }
        let store = Self {
            path: Arc::new(path),
        };
        let connection = store.connection()?;
        migrate(&connection)?;
        Ok(store)
    }

    fn connection(&self) -> Result<Connection, StoreError> {
        let connection = Connection::open(self.path.as_ref())?;
        connection.busy_timeout(Duration::from_secs(5))?;
        connection.pragma_update(None, "foreign_keys", "ON")?;
        connection.pragma_update(None, "journal_mode", "WAL")?;
        connection.pragma_update(None, "synchronous", "NORMAL")?;
        Ok(connection)
    }

    pub(crate) fn list_boards(&self) -> Result<Vec<Board>, StoreError> {
        let connection = self.connection()?;
        let mut statement = connection
            .prepare("SELECT id, slug, name, created_at FROM boards ORDER BY created_at, id")?;
        statement
            .query_map([], board_from_row)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(Into::into)
    }

    pub(crate) fn create_thread(
        &self,
        input: ValidCreateThread,
    ) -> Result<CreateThreadResponse, StoreError> {
        let mut connection = self.connection()?;
        let transaction = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let timestamp = now();
        let thread_id = Uuid::now_v7().to_string();
        let post_id = Uuid::now_v7().to_string();

        transaction.execute(
            "INSERT INTO threads (id, board_id, title, status, author, created_at, updated_at)
             VALUES (?1, ?2, ?3, 'open', ?4, ?5, ?5)",
            params![
                thread_id,
                GENERAL_BOARD_ID,
                input.title,
                input.author,
                timestamp
            ],
        )?;
        replace_tags(&transaction, &thread_id, &input.tags)?;

        let payload = json!({
            "actor": input.author,
            "title": input.title,
            "tags": input.tags,
            "initial_post_id": post_id,
        });
        let event_id = insert_event(
            &transaction,
            "thread.created",
            &thread_id,
            Some(&post_id),
            &payload,
            &timestamp,
        )?;
        transaction.execute(
            "INSERT INTO posts (id, thread_id, author, body, created_at, event_id)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![
                post_id,
                thread_id,
                input.author,
                input.body,
                timestamp,
                event_id
            ],
        )?;
        transaction.execute(
            "INSERT INTO search_index (thread_id, title, body) VALUES (?1, ?2, ?3)",
            params![thread_id, input.title, input.body],
        )?;

        let thread = get_thread_tx(&transaction, &thread_id)?;
        let initial_post = get_post_tx(&transaction, &post_id)?;
        transaction.commit()?;
        Ok(CreateThreadResponse {
            thread,
            initial_post,
            event_id,
        })
    }

    pub(crate) fn reply(
        &self,
        thread_id: &str,
        input: ValidReply,
    ) -> Result<ReplyResponse, StoreError> {
        let mut connection = self.connection()?;
        let transaction = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let status = transaction
            .query_row(
                "SELECT status FROM threads WHERE id = ?1",
                [thread_id],
                |row| row.get::<_, String>(0),
            )
            .optional()?
            .ok_or(StoreError::NotFound)?;
        if status != "open" {
            return Err(StoreError::ThreadClosed);
        }

        let timestamp = now();
        let post_id = Uuid::now_v7().to_string();
        let payload = json!({"actor": input.author, "post_id": post_id});
        let event_id = insert_event(
            &transaction,
            "post.created",
            thread_id,
            Some(&post_id),
            &payload,
            &timestamp,
        )?;
        transaction.execute(
            "INSERT INTO posts (id, thread_id, author, body, created_at, event_id)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![
                post_id,
                thread_id,
                input.author,
                input.body,
                timestamp,
                event_id
            ],
        )?;
        transaction.execute(
            "UPDATE threads SET updated_at = ?2 WHERE id = ?1",
            params![thread_id, timestamp],
        )?;
        transaction.execute(
            "UPDATE search_index SET body = body || char(10) || ?2 WHERE thread_id = ?1",
            params![thread_id, input.body],
        )?;
        let post = get_post_tx(&transaction, &post_id)?;
        transaction.commit()?;
        Ok(ReplyResponse { post, event_id })
    }

    pub(crate) fn update_thread(
        &self,
        thread_id: &str,
        input: ValidUpdateThread,
    ) -> Result<UpdateThreadResponse, StoreError> {
        let mut connection = self.connection()?;
        let transaction = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let current = get_thread_tx(&transaction, thread_id)?;
        let next_status = input.status.unwrap_or(current.status);
        let next_tags = input.tags.unwrap_or_else(|| current.tags.clone());
        let changed = next_status != current.status || next_tags != current.tags;
        if !changed {
            transaction.commit()?;
            return Ok(UpdateThreadResponse {
                thread: current,
                changed: false,
                event_id: None,
            });
        }

        let timestamp = now();
        transaction.execute(
            "UPDATE threads SET status = ?2, updated_at = ?3 WHERE id = ?1",
            params![thread_id, next_status.as_str(), timestamp],
        )?;
        replace_tags(&transaction, thread_id, &next_tags)?;
        let payload = json!({
            "actor": input.actor,
            "previous": {"status": current.status, "tags": current.tags},
            "current": {"status": next_status, "tags": next_tags},
        });
        let event_id = insert_event(
            &transaction,
            "thread.updated",
            thread_id,
            None,
            &payload,
            &timestamp,
        )?;
        let thread = get_thread_tx(&transaction, thread_id)?;
        transaction.commit()?;
        Ok(UpdateThreadResponse {
            thread,
            changed: true,
            event_id: Some(event_id),
        })
    }

    pub(crate) fn get_thread(&self, thread_id: &str) -> Result<ThreadRecord, StoreError> {
        let connection = self.connection()?;
        get_thread_conn(&connection, thread_id)
    }

    pub(crate) fn get_thread_detail(
        &self,
        thread_id: &str,
        after: i64,
        limit: u32,
    ) -> Result<ThreadDetail, StoreError> {
        let connection = self.connection()?;
        let thread = get_thread_conn(&connection, thread_id)?;
        let mut statement = connection.prepare(
            "SELECT id, thread_id, author, body, created_at, event_id
             FROM posts WHERE thread_id = ?1 AND event_id > ?2
             ORDER BY event_id ASC LIMIT ?3",
        )?;
        let posts = statement
            .query_map(params![thread_id, after, limit], post_from_row)?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(ThreadDetail {
            thread,
            posts,
            limit,
            after,
        })
    }

    pub(crate) fn list_threads(
        &self,
        input: ListThreadsInput,
    ) -> Result<Page<ThreadRecord>, StoreError> {
        let connection = self.connection()?;
        let status = input.status.map(|status| status.as_str().to_owned());
        let mut statement = connection.prepare(
            "SELECT t.id, t.board_id, t.title, t.status, t.author, t.created_at, t.updated_at
             FROM threads t
             WHERE (?1 IS NULL OR t.status = ?1)
               AND (?2 IS NULL OR EXISTS (
                 SELECT 1 FROM thread_tags tt WHERE tt.thread_id = t.id AND tt.tag = ?2
               ))
             ORDER BY t.updated_at DESC, t.id DESC LIMIT ?3 OFFSET ?4",
        )?;
        let rows = statement
            .query_map(
                params![status, input.tag, input.limit, input.offset],
                thread_without_tags_from_row,
            )?
            .collect::<Result<Vec<_>, _>>()?;
        let items = attach_tags(&connection, rows)?;
        Ok(Page {
            items,
            limit: input.limit,
            offset: input.offset,
        })
    }

    pub(crate) fn search(&self, input: SearchInput) -> Result<Page<ThreadRecord>, StoreError> {
        let connection = self.connection()?;
        let status = input.status.map(|status| status.as_str().to_owned());
        let query = format!("\"{}\"", input.query.replace('"', "\"\""));
        let mut statement = connection.prepare(
            "SELECT t.id, t.board_id, t.title, t.status, t.author, t.created_at, t.updated_at
             FROM search_index
             JOIN threads t ON t.id = search_index.thread_id
             WHERE (
                 search_index.rowid IN (
                   SELECT rowid FROM search_index WHERE search_index MATCH ?1
                 )
                 OR instr(lower(search_index.title), lower(?6)) > 0
                 OR instr(lower(search_index.body), lower(?6)) > 0
               )
               AND (?2 IS NULL OR t.status = ?2)
               AND (?3 IS NULL OR EXISTS (
                 SELECT 1 FROM thread_tags tt WHERE tt.thread_id = t.id AND tt.tag = ?3
               ))
             ORDER BY
               CASE WHEN instr(lower(search_index.title), lower(?6)) > 0 THEN 0 ELSE 1 END,
               t.updated_at DESC
             LIMIT ?4 OFFSET ?5",
        )?;
        let rows = statement
            .query_map(
                params![
                    query,
                    status,
                    input.tag,
                    input.limit,
                    input.offset,
                    input.query
                ],
                thread_without_tags_from_row,
            )?
            .collect::<Result<Vec<_>, _>>()?;
        let items = attach_tags(&connection, rows)?;
        Ok(Page {
            items,
            limit: input.limit,
            offset: input.offset,
        })
    }

    pub(crate) fn latest_event_id(&self) -> Result<i64, StoreError> {
        let connection = self.connection()?;
        connection
            .query_row("SELECT COALESCE(MAX(id), 0) FROM events", [], |row| {
                row.get(0)
            })
            .map_err(Into::into)
    }

    pub(crate) fn events_after(&self, after: i64, limit: u32) -> Result<Vec<Event>, StoreError> {
        let connection = self.connection()?;
        let mut statement = connection.prepare(
            "SELECT id, kind, thread_id, post_id, payload, created_at
             FROM events WHERE id > ?1 ORDER BY id ASC LIMIT ?2",
        )?;
        statement
            .query_map(params![after, limit], event_from_row)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(Into::into)
    }
}

fn migrate(connection: &Connection) -> Result<(), StoreError> {
    connection.execute_batch(
        "CREATE TABLE IF NOT EXISTS boards (
            id TEXT PRIMARY KEY,
            slug TEXT NOT NULL UNIQUE,
            name TEXT NOT NULL,
            created_at TEXT NOT NULL
        );
        CREATE TABLE IF NOT EXISTS threads (
            id TEXT PRIMARY KEY,
            board_id TEXT NOT NULL REFERENCES boards(id),
            title TEXT NOT NULL CHECK(length(title) BETWEEN 1 AND 512),
            status TEXT NOT NULL CHECK(status IN ('open', 'closed')),
            author TEXT NOT NULL CHECK(length(author) BETWEEN 1 AND 128),
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL
        );
        CREATE INDEX IF NOT EXISTS threads_updated_idx ON threads(updated_at DESC);
        CREATE TABLE IF NOT EXISTS thread_tags (
            thread_id TEXT NOT NULL REFERENCES threads(id),
            tag TEXT NOT NULL CHECK(length(tag) BETWEEN 1 AND 64),
            PRIMARY KEY (thread_id, tag)
        );
        CREATE INDEX IF NOT EXISTS thread_tags_tag_idx ON thread_tags(tag, thread_id);
        CREATE TABLE IF NOT EXISTS events (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            kind TEXT NOT NULL,
            thread_id TEXT NOT NULL REFERENCES threads(id),
            post_id TEXT,
            payload TEXT NOT NULL,
            created_at TEXT NOT NULL
        );
        CREATE TABLE IF NOT EXISTS posts (
            id TEXT PRIMARY KEY,
            thread_id TEXT NOT NULL REFERENCES threads(id),
            author TEXT NOT NULL CHECK(length(author) BETWEEN 1 AND 128),
            body TEXT NOT NULL CHECK(length(CAST(body AS BLOB)) BETWEEN 1 AND 1048576),
            created_at TEXT NOT NULL,
            event_id INTEGER NOT NULL UNIQUE REFERENCES events(id)
        );
        CREATE INDEX IF NOT EXISTS posts_thread_event_idx ON posts(thread_id, event_id);
        CREATE VIRTUAL TABLE IF NOT EXISTS search_index USING fts5(
            thread_id UNINDEXED,
            title,
            body,
            tokenize = 'unicode61'
        );",
    )?;
    connection.execute(
        "INSERT OR IGNORE INTO boards (id, slug, name, created_at)
         VALUES ('general', 'general', 'General', ?1)",
        [now()],
    )?;
    Ok(())
}

fn now() -> String {
    OffsetDateTime::now_utc()
        .format(&Rfc3339)
        .expect("RFC3339 formatting cannot fail")
}

fn insert_event(
    transaction: &Transaction<'_>,
    kind: &str,
    thread_id: &str,
    post_id: Option<&str>,
    payload: &Value,
    timestamp: &str,
) -> Result<i64, StoreError> {
    transaction.execute(
        "INSERT INTO events (kind, thread_id, post_id, payload, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5)",
        params![kind, thread_id, post_id, payload.to_string(), timestamp],
    )?;
    Ok(transaction.last_insert_rowid())
}

fn replace_tags(
    transaction: &Transaction<'_>,
    thread_id: &str,
    tags: &[String],
) -> Result<(), StoreError> {
    transaction.execute("DELETE FROM thread_tags WHERE thread_id = ?1", [thread_id])?;
    for tag in tags {
        transaction.execute(
            "INSERT INTO thread_tags (thread_id, tag) VALUES (?1, ?2)",
            params![thread_id, tag],
        )?;
    }
    Ok(())
}

fn board_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<Board> {
    Ok(Board {
        id: row.get(0)?,
        slug: row.get(1)?,
        name: row.get(2)?,
        created_at: row.get(3)?,
    })
}

fn thread_without_tags_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<ThreadRecord> {
    let status: String = row.get(3)?;
    let status = ThreadStatus::from_str(&status).map_err(|error| {
        rusqlite::Error::FromSqlConversionFailure(3, rusqlite::types::Type::Text, Box::new(error))
    })?;
    Ok(ThreadRecord {
        id: row.get(0)?,
        board_id: row.get(1)?,
        title: row.get(2)?,
        status,
        author: row.get(4)?,
        tags: vec![],
        created_at: row.get(5)?,
        updated_at: row.get(6)?,
    })
}

fn post_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<Post> {
    Ok(Post {
        id: row.get(0)?,
        thread_id: row.get(1)?,
        author: row.get(2)?,
        body: row.get(3)?,
        created_at: row.get(4)?,
        event_id: row.get(5)?,
    })
}

fn event_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<Event> {
    let payload: String = row.get(4)?;
    let payload = serde_json::from_str(&payload).map_err(|error| {
        rusqlite::Error::FromSqlConversionFailure(4, rusqlite::types::Type::Text, Box::new(error))
    })?;
    Ok(Event {
        id: row.get(0)?,
        kind: row.get(1)?,
        thread_id: row.get(2)?,
        post_id: row.get(3)?,
        payload,
        created_at: row.get(5)?,
    })
}

fn get_thread_conn(connection: &Connection, thread_id: &str) -> Result<ThreadRecord, StoreError> {
    let mut thread = connection
        .query_row(
            "SELECT id, board_id, title, status, author, created_at, updated_at
             FROM threads WHERE id = ?1",
            [thread_id],
            thread_without_tags_from_row,
        )
        .optional()?
        .ok_or(StoreError::NotFound)?;
    thread.tags = tags_for_thread(connection, thread_id)?;
    Ok(thread)
}

fn get_thread_tx(
    transaction: &Transaction<'_>,
    thread_id: &str,
) -> Result<ThreadRecord, StoreError> {
    let mut thread = transaction
        .query_row(
            "SELECT id, board_id, title, status, author, created_at, updated_at
             FROM threads WHERE id = ?1",
            [thread_id],
            thread_without_tags_from_row,
        )
        .optional()?
        .ok_or(StoreError::NotFound)?;
    thread.tags = tags_for_thread(transaction, thread_id)?;
    Ok(thread)
}

fn get_post_tx(transaction: &Transaction<'_>, post_id: &str) -> Result<Post, StoreError> {
    transaction
        .query_row(
            "SELECT id, thread_id, author, body, created_at, event_id
             FROM posts WHERE id = ?1",
            [post_id],
            post_from_row,
        )
        .optional()?
        .ok_or(StoreError::NotFound)
}

fn tags_for_thread(connection: &Connection, thread_id: &str) -> Result<Vec<String>, StoreError> {
    let mut statement =
        connection.prepare("SELECT tag FROM thread_tags WHERE thread_id = ?1 ORDER BY tag ASC")?;
    statement
        .query_map([thread_id], |row| row.get(0))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(Into::into)
}

fn attach_tags(
    connection: &Connection,
    rows: Vec<ThreadRecord>,
) -> Result<Vec<ThreadRecord>, StoreError> {
    rows.into_iter()
        .map(|mut thread| {
            thread.tags = tags_for_thread(connection, &thread.id)?;
            Ok(thread)
        })
        .collect()
}
