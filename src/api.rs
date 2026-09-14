use std::{collections::VecDeque, convert::Infallible, time::Duration};

use axum::{
    Json, Router,
    body::Body,
    extract::{
        DefaultBodyLimit, Path, Query, State,
        rejection::{JsonRejection, QueryRejection},
    },
    http::{Request, StatusCode, header},
    middleware::{self, Next},
    response::{
        IntoResponse, Response, Sse,
        sse::{Event as SseEvent, KeepAlive},
    },
    routing::get,
};
use futures::stream;
use serde::Deserialize;
use serde_json::json;
use tower_http::trace::TraceLayer;
use tracing::error;

use crate::{
    domain::{
        CreateThreadInput, ErrorBody, ErrorEnvelope, ListThreadsInput, PostPage, ReplyInput,
        SearchInput, ThreadStatus, UpdateThreadInput, default_limit,
    },
    service::{AppError, AppService},
};

pub fn router(service: AppService) -> Router {
    Router::new()
        .route("/api/v1/boards", get(list_boards))
        .route("/api/v1/threads", get(list_threads).post(create_thread))
        .route(
            "/api/v1/threads/{thread_id}",
            get(get_thread).patch(update_thread),
        )
        .route(
            "/api/v1/threads/{thread_id}/posts",
            get(list_posts).post(reply),
        )
        .route("/api/v1/search", get(search_threads))
        .route("/api/v1/events", get(events))
        .merge(crate::ui::routes())
        .fallback(not_found)
        .method_not_allowed_fallback(method_not_allowed)
        .with_state(service)
        .layer(DefaultBodyLimit::max(8 * 1024 * 1024))
        .layer(TraceLayer::new_for_http())
        .layer(middleware::from_fn(validate_host))
}

#[derive(Debug, Deserialize)]
struct ListQuery {
    status: Option<ThreadStatus>,
    tag: Option<String>,
    #[serde(default = "default_limit")]
    limit: u32,
    #[serde(default)]
    offset: u32,
}

#[derive(Debug, Deserialize)]
struct PostsQuery {
    #[serde(default)]
    after: i64,
    #[serde(default = "default_limit")]
    limit: u32,
}

#[derive(Debug, Deserialize)]
struct SearchQuery {
    q: String,
    status: Option<ThreadStatus>,
    tag: Option<String>,
    #[serde(default = "default_limit")]
    limit: u32,
    #[serde(default)]
    offset: u32,
}

#[derive(Debug, Deserialize)]
struct EventsQuery {
    after: Option<i64>,
}

async fn list_boards(
    State(service): State<AppService>,
) -> Result<Json<serde_json::Value>, ApiError> {
    Ok(Json(json!(service.list_boards().await?)))
}

async fn list_threads(
    State(service): State<AppService>,
    query: Result<Query<ListQuery>, QueryRejection>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let Query(query) = query.map_err(ApiError::query)?;
    let page = service
        .list_threads(ListThreadsInput {
            status: query.status,
            tag: query.tag,
            limit: query.limit,
            offset: query.offset,
        })
        .await?;
    Ok(Json(json!(page)))
}

async fn create_thread(
    State(service): State<AppService>,
    body: Result<Json<CreateThreadInput>, JsonRejection>,
) -> Result<impl IntoResponse, ApiError> {
    let Json(input) = body.map_err(ApiError::json)?;
    let created = service.create_thread(input).await?;
    Ok((StatusCode::CREATED, Json(created)))
}

async fn get_thread(
    State(service): State<AppService>,
    Path(thread_id): Path<String>,
) -> Result<Json<serde_json::Value>, ApiError> {
    Ok(Json(json!(service.get_thread_record(&thread_id).await?)))
}

async fn list_posts(
    State(service): State<AppService>,
    Path(thread_id): Path<String>,
    query: Result<Query<PostsQuery>, QueryRejection>,
) -> Result<Json<PostPage>, ApiError> {
    let Query(query) = query.map_err(ApiError::query)?;
    let detail = service
        .get_thread(&thread_id, query.after, query.limit)
        .await?;
    Ok(Json(PostPage {
        items: detail.posts,
        limit: detail.limit,
        after: detail.after,
    }))
}

async fn reply(
    State(service): State<AppService>,
    Path(thread_id): Path<String>,
    body: Result<Json<ReplyInput>, JsonRejection>,
) -> Result<impl IntoResponse, ApiError> {
    let Json(input) = body.map_err(ApiError::json)?;
    let created = service.reply(&thread_id, input).await?;
    Ok((StatusCode::CREATED, Json(created)))
}

async fn update_thread(
    State(service): State<AppService>,
    Path(thread_id): Path<String>,
    body: Result<Json<UpdateThreadInput>, JsonRejection>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let Json(input) = body.map_err(ApiError::json)?;
    Ok(Json(json!(service.update_thread(&thread_id, input).await?)))
}

async fn search_threads(
    State(service): State<AppService>,
    query: Result<Query<SearchQuery>, QueryRejection>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let Query(query) = query.map_err(ApiError::query)?;
    let page = service
        .search(SearchInput {
            query: query.q,
            status: query.status,
            tag: query.tag,
            limit: query.limit,
            offset: query.offset,
        })
        .await?;
    Ok(Json(json!(page)))
}

async fn events(
    State(service): State<AppService>,
    query: Result<Query<EventsQuery>, QueryRejection>,
) -> Result<Sse<impl futures::Stream<Item = Result<SseEvent, Infallible>>>, ApiError> {
    let Query(query) = query.map_err(ApiError::query)?;
    let after = match query.after {
        Some(after) if after < 0 => {
            return Err(ApiError::from(AppError::InvalidInput(
                "after must be a non-negative event id".into(),
            )));
        }
        Some(after) => after,
        None => service.latest_event_id().await?,
    };

    let stream = stream::unfold(
        (service, after, VecDeque::new(), false),
        |(service, mut cursor, mut queue, failed)| async move {
            if failed {
                return None;
            }
            if queue.is_empty() {
                match service
                    .wait_for_updates(cursor, Duration::from_secs(30))
                    .await
                {
                    Ok(events) => queue.extend(events),
                    Err(error) => {
                        let event = SseEvent::default()
                            .event("error")
                            .data(format!("internal stream error: {error}"));
                        return Some((Ok(event), (service, cursor, queue, true)));
                    }
                }
            }
            let event = match queue.pop_front() {
                Some(event) => event,
                None => {
                    return Some((
                        Ok(SseEvent::default().comment("keep-alive")),
                        (service, cursor, queue, false),
                    ));
                }
            };
            cursor = event.id;
            let frame = SseEvent::default()
                .id(event.id.to_string())
                .event(event.kind.clone())
                .json_data(event)
                .expect("serializing an event cannot fail");
            Some((Ok(frame), (service, cursor, queue, false)))
        },
    );

    Ok(Sse::new(stream).keep_alive(KeepAlive::new().interval(Duration::from_secs(30))))
}

async fn not_found() -> ApiError {
    ApiError::new(StatusCode::NOT_FOUND, "not_found", "resource not found")
}

async fn method_not_allowed() -> ApiError {
    ApiError::new(
        StatusCode::METHOD_NOT_ALLOWED,
        "method_not_allowed",
        "method not allowed",
    )
}

async fn validate_host(request: Request<Body>, next: Next) -> Response {
    let host = request
        .headers()
        .get(header::HOST)
        .and_then(|value| value.to_str().ok());
    if !host.is_some_and(is_local_host) {
        return ApiError::new(
            StatusCode::FORBIDDEN,
            "invalid_host",
            "Host must identify the local machine",
        )
        .into_response();
    }
    next.run(request).await
}

fn is_local_host(host: &str) -> bool {
    let (name, port) = if let Some(rest) = host.strip_prefix('[') {
        match rest.split_once(']') {
            Some((name, "")) => (format!("[{name}]"), None),
            Some((name, suffix)) => match suffix.strip_prefix(':') {
                Some(port) => (format!("[{name}]"), Some(port)),
                None => return false,
            },
            None => return false,
        }
    } else {
        match host.rsplit_once(':') {
            Some((name, port)) if !name.contains(':') => (name.to_owned(), Some(port)),
            _ => (host.to_owned(), None),
        }
    };
    if port.is_some_and(|port| {
        port.is_empty()
            || !port.bytes().all(|byte| byte.is_ascii_digit())
            || port.parse::<u16>().is_err()
    }) {
        return false;
    }
    matches!(name.as_str(), "localhost" | "127.0.0.1" | "[::1]")
}

#[derive(Debug)]
pub struct ApiError {
    status: StatusCode,
    code: &'static str,
    message: String,
}

impl ApiError {
    fn new(status: StatusCode, code: &'static str, message: impl Into<String>) -> Self {
        Self {
            status,
            code,
            message: message.into(),
        }
    }

    fn json(error: JsonRejection) -> Self {
        Self::new(
            StatusCode::UNPROCESSABLE_ENTITY,
            "invalid_input",
            error.body_text(),
        )
    }

    fn query(error: QueryRejection) -> Self {
        Self::new(
            StatusCode::UNPROCESSABLE_ENTITY,
            "invalid_input",
            error.body_text(),
        )
    }
}

impl From<AppError> for ApiError {
    fn from(error: AppError) -> Self {
        match error {
            AppError::InvalidInput(message) => {
                Self::new(StatusCode::UNPROCESSABLE_ENTITY, "invalid_input", message)
            }
            AppError::NotFound => {
                Self::new(StatusCode::NOT_FOUND, "not_found", "resource not found")
            }
            AppError::ThreadClosed => {
                Self::new(StatusCode::CONFLICT, "thread_closed", "thread is closed")
            }
            AppError::Internal(message) => {
                error!(error = %message, "request failed");
                Self::new(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "internal_error",
                    "internal server error",
                )
            }
        }
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let body = ErrorEnvelope {
            error: ErrorBody {
                code: self.code.to_owned(),
                message: self.message,
            },
        };
        (self.status, Json(body)).into_response()
    }
}

#[cfg(test)]
mod tests {
    use super::is_local_host;

    #[test]
    fn host_validation_accepts_only_loopback_names() {
        assert!(is_local_host("localhost"));
        assert!(is_local_host("localhost:8080"));
        assert!(is_local_host("127.0.0.1:8080"));
        assert!(is_local_host("[::1]:8080"));
        assert!(!is_local_host("[::1]evil"));
        assert!(!is_local_host("[::1]:"));
        assert!(!is_local_host("[::1]:invalid"));
        assert!(!is_local_host("localhost:+80"));
        assert!(!is_local_host("[::1]:+80"));
        assert!(!is_local_host("localhost.example"));
        assert!(!is_local_host("127.0.0.1.example:8080"));
        assert!(!is_local_host("0.0.0.0:8080"));
    }
}
