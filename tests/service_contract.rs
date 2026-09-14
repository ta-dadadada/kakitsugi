use std::time::Duration;

use kakitsugi::{
    domain::{CreateThreadInput, ReplyInput, SearchInput, ThreadStatus, UpdateThreadInput},
    service::{AppError, AppService},
};
use tempfile::TempDir;

async fn service() -> (TempDir, AppService) {
    let dir = TempDir::new().unwrap();
    let service = AppService::open(dir.path().join("kakitsugi.sqlite3"))
        .await
        .unwrap();
    (dir, service)
}

fn thread_input(body: &str) -> CreateThreadInput {
    CreateThreadInput {
        title: "Shared investigation".into(),
        author: "claude".into(),
        body: body.into(),
        tags: vec!["research".into(), "handoff".into()],
    }
}

#[tokio::test]
async fn create_reply_and_reopen_database_preserves_complete_exchange() {
    let (dir, service) = service().await;
    let created = service
        .create_thread(thread_input("Finding from the first session"))
        .await
        .unwrap();

    let reply = service
        .reply(
            &created.thread.id,
            ReplyInput {
                author: "codex".into(),
                body: "Implemented in the second session".into(),
            },
        )
        .await
        .unwrap();
    assert!(reply.event_id > created.event_id);

    drop(service);
    let reopened = AppService::open(dir.path().join("kakitsugi.sqlite3"))
        .await
        .unwrap();
    let detail = reopened
        .get_thread(&created.thread.id, 0, 100)
        .await
        .unwrap();

    assert_eq!(detail.thread.tags, vec!["handoff", "research"]);
    assert_eq!(detail.posts.len(), 2);
    assert_eq!(detail.posts[0].body, "Finding from the first session");
    assert_eq!(detail.posts[1].author, "codex");
}

#[tokio::test]
async fn closed_thread_rejects_reply_until_reopened() {
    let (_dir, service) = service().await;
    let created = service
        .create_thread(thread_input("Initial"))
        .await
        .unwrap();

    service
        .update_thread(
            &created.thread.id,
            UpdateThreadInput {
                actor: "codex".into(),
                status: Some(ThreadStatus::Closed),
                tags: None,
            },
        )
        .await
        .unwrap();

    let error = service
        .reply(
            &created.thread.id,
            ReplyInput {
                author: "claude".into(),
                body: "too late".into(),
            },
        )
        .await
        .unwrap_err();
    assert!(matches!(error, AppError::ThreadClosed));

    service
        .update_thread(
            &created.thread.id,
            UpdateThreadInput {
                actor: "codex".into(),
                status: Some(ThreadStatus::Open),
                tags: None,
            },
        )
        .await
        .unwrap();
    service
        .reply(
            &created.thread.id,
            ReplyInput {
                author: "claude".into(),
                body: "now accepted".into(),
            },
        )
        .await
        .unwrap();
}

#[tokio::test]
async fn search_finds_thread_title_and_post_body_with_filters() {
    let (_dir, service) = service().await;
    let first = service
        .create_thread(thread_input("contains ultraviolet evidence"))
        .await
        .unwrap();
    service
        .create_thread(CreateThreadInput {
            title: "Ultraviolet title".into(),
            author: "codex".into(),
            body: "different body".into(),
            tags: vec!["other".into()],
        })
        .await
        .unwrap();

    let result = service
        .search(SearchInput {
            query: "ultraviolet".into(),
            status: Some(ThreadStatus::Open),
            tag: Some("research".into()),
            limit: 50,
            offset: 0,
        })
        .await
        .unwrap();

    assert_eq!(result.items.len(), 1);
    assert_eq!(result.items[0].id, first.thread.id);
}

#[tokio::test]
async fn search_supports_japanese_substrings_without_spaces() {
    let (_dir, service) = service().await;
    let created = service
        .create_thread(CreateThreadInput {
            title: "調査引き継ぎ".into(),
            author: "claude".into(),
            body: "依存関係の確認が完了しました".into(),
            tags: vec![],
        })
        .await
        .unwrap();

    let result = service
        .search(SearchInput {
            query: "関係の確認".into(),
            status: None,
            tag: None,
            limit: 50,
            offset: 0,
        })
        .await
        .unwrap();

    assert_eq!(result.items.len(), 1);
    assert_eq!(result.items[0].id, created.thread.id);
}

#[tokio::test]
async fn wait_for_updates_returns_durable_event_after_cursor() {
    let (_dir, service) = service().await;
    let after = service.latest_event_id().await.unwrap();
    let waiter = tokio::spawn({
        let service = service.clone();
        async move {
            service
                .wait_for_updates(after, Duration::from_secs(2))
                .await
        }
    });

    let created = service
        .create_thread(thread_input("wake up"))
        .await
        .unwrap();
    let events = waiter.await.unwrap().unwrap();

    assert_eq!(events[0].id, created.event_id);
    assert_eq!(events[0].kind, "thread.created");
}

#[tokio::test]
async fn wait_for_updates_detects_writes_from_an_independent_service() {
    let dir = TempDir::new().unwrap();
    let database = dir.path().join("kakitsugi.sqlite3");
    let reader = AppService::open(database.clone()).await.unwrap();
    let writer = AppService::open(database).await.unwrap();
    let after = reader.latest_event_id().await.unwrap();
    let waiter =
        tokio::spawn(async move { reader.wait_for_updates(after, Duration::from_secs(2)).await });

    let created = writer
        .create_thread(thread_input("written through another service"))
        .await
        .unwrap();
    let events = waiter.await.unwrap().unwrap();

    assert_eq!(events.len(), 1);
    assert_eq!(events[0].id, created.event_id);
}

#[tokio::test]
async fn invalid_input_does_not_create_an_event() {
    let (_dir, service) = service().await;
    let before = service.latest_event_id().await.unwrap();
    let error = service
        .create_thread(CreateThreadInput {
            title: " ".into(),
            author: "claude".into(),
            body: "content".into(),
            tags: vec![],
        })
        .await
        .unwrap_err();

    assert!(matches!(error, AppError::InvalidInput(_)));
    assert_eq!(service.latest_event_id().await.unwrap(), before);
}
