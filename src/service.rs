use std::{path::PathBuf, time::Duration};

use thiserror::Error;
use tokio::{sync::broadcast, task::JoinError, time::Instant};

use crate::{
    domain::{
        Board, CreateThreadInput, CreateThreadResponse, Event, ListThreadsInput, Page, ReplyInput,
        ReplyResponse, SearchInput, ThreadDetail, ThreadRecord, UpdateThreadInput,
        UpdateThreadResponse, ValidationError, validate_limit,
    },
    store::{Store, StoreError},
};

#[derive(Debug, Error)]
pub enum AppError {
    #[error("invalid input: {0}")]
    InvalidInput(String),
    #[error("resource not found")]
    NotFound,
    #[error("thread is closed")]
    ThreadClosed,
    #[error("internal error: {0}")]
    Internal(String),
}

impl From<ValidationError> for AppError {
    fn from(error: ValidationError) -> Self {
        Self::InvalidInput(error.to_string())
    }
}

impl From<StoreError> for AppError {
    fn from(error: StoreError) -> Self {
        match error {
            StoreError::NotFound => Self::NotFound,
            StoreError::ThreadClosed => Self::ThreadClosed,
            other => Self::Internal(other.to_string()),
        }
    }
}

impl From<JoinError> for AppError {
    fn from(error: JoinError) -> Self {
        Self::Internal(format!("database task failed: {error}"))
    }
}

#[derive(Clone, Debug)]
pub struct AppService {
    store: Store,
    updates: broadcast::Sender<i64>,
}

impl AppService {
    pub async fn open(path: PathBuf) -> Result<Self, AppError> {
        let store = tokio::task::spawn_blocking(move || Store::open(path)).await??;
        let (updates, _) = broadcast::channel(256);
        Ok(Self { store, updates })
    }

    pub async fn list_boards(&self) -> Result<Vec<Board>, AppError> {
        self.run(|store| store.list_boards()).await
    }

    pub async fn create_thread(
        &self,
        input: CreateThreadInput,
    ) -> Result<CreateThreadResponse, AppError> {
        let input = input.validate()?;
        let result = self.run(move |store| store.create_thread(input)).await?;
        self.notify(result.event_id);
        Ok(result)
    }

    pub async fn reply(
        &self,
        thread_id: &str,
        input: ReplyInput,
    ) -> Result<ReplyResponse, AppError> {
        let input = input.validate()?;
        let thread_id = thread_id.to_owned();
        let result = self
            .run(move |store| store.reply(&thread_id, input))
            .await?;
        self.notify(result.event_id);
        Ok(result)
    }

    pub async fn update_thread(
        &self,
        thread_id: &str,
        input: UpdateThreadInput,
    ) -> Result<UpdateThreadResponse, AppError> {
        let input = input.validate()?;
        let thread_id = thread_id.to_owned();
        let result = self
            .run(move |store| store.update_thread(&thread_id, input))
            .await?;
        if let Some(event_id) = result.event_id {
            self.notify(event_id);
        }
        Ok(result)
    }

    pub async fn get_thread(
        &self,
        thread_id: &str,
        after: i64,
        limit: u32,
    ) -> Result<ThreadDetail, AppError> {
        validate_cursor(after)?;
        validate_limit(limit)?;
        let thread_id = thread_id.to_owned();
        self.run(move |store| store.get_thread_detail(&thread_id, after, limit))
            .await
    }

    pub async fn get_thread_record(&self, thread_id: &str) -> Result<ThreadRecord, AppError> {
        let thread_id = thread_id.to_owned();
        self.run(move |store| store.get_thread(&thread_id)).await
    }

    pub async fn list_threads(
        &self,
        input: ListThreadsInput,
    ) -> Result<Page<ThreadRecord>, AppError> {
        let input = input.validate()?;
        self.run(move |store| store.list_threads(input)).await
    }

    pub async fn search(&self, input: SearchInput) -> Result<Page<ThreadRecord>, AppError> {
        let input = input.validate()?;
        self.run(move |store| store.search(input)).await
    }

    pub async fn latest_event_id(&self) -> Result<i64, AppError> {
        self.run(|store| store.latest_event_id()).await
    }

    pub async fn events_after(&self, after: i64) -> Result<Vec<Event>, AppError> {
        validate_cursor(after)?;
        self.run(move |store| store.events_after(after, 100)).await
    }

    pub async fn wait_for_updates(
        &self,
        after: i64,
        timeout: Duration,
    ) -> Result<Vec<Event>, AppError> {
        validate_cursor(after)?;
        if timeout.is_zero() || timeout > Duration::from_secs(30) {
            return Err(AppError::InvalidInput(
                "timeout must be between 1ms and 30000ms".into(),
            ));
        }

        let mut receiver = self.updates.subscribe();
        let deadline = Instant::now() + timeout;
        loop {
            let events = self.events_after(after).await?;
            if !events.is_empty() {
                return Ok(events);
            }
            let now = Instant::now();
            if now >= deadline {
                return Ok(vec![]);
            }
            let remaining = deadline - now;
            tokio::select! {
                _ = tokio::time::sleep(remaining.min(Duration::from_millis(200))) => {},
                _ = receiver.recv() => {},
            }
        }
    }

    fn notify(&self, event_id: i64) {
        let _ = self.updates.send(event_id);
    }

    async fn run<T, F>(&self, operation: F) -> Result<T, AppError>
    where
        T: Send + 'static,
        F: FnOnce(Store) -> Result<T, StoreError> + Send + 'static,
    {
        let store = self.store.clone();
        Ok(tokio::task::spawn_blocking(move || operation(store)).await??)
    }
}

fn validate_cursor(after: i64) -> Result<(), AppError> {
    if after < 0 {
        Err(AppError::InvalidInput(
            "after must be a non-negative event id".into(),
        ))
    } else {
        Ok(())
    }
}
