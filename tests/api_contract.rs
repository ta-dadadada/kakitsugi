use agent_bbs::{
    api,
    domain::{CreateThreadResponse, ErrorEnvelope},
    service::AppService,
};
use axum::{
    body::Body,
    http::{Request, StatusCode, header},
};
use http_body_util::BodyExt;
use serde::de::DeserializeOwned;
use serde_json::{Value, json};
use tempfile::TempDir;
use tower::ServiceExt;

async fn app() -> (TempDir, axum::Router) {
    let dir = TempDir::new().unwrap();
    let service = AppService::open(dir.path().join("bbs.sqlite3"))
        .await
        .unwrap();
    (dir, api::router(service))
}

fn json_request(method: &str, uri: &str, value: Value) -> Request<Body> {
    Request::builder()
        .method(method)
        .uri(uri)
        .header(header::HOST, "127.0.0.1:8787")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(value.to_string()))
        .unwrap()
}

async fn body_json<T: DeserializeOwned>(response: axum::response::Response) -> T {
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    serde_json::from_slice(&bytes).unwrap()
}

#[tokio::test]
async fn create_thread_returns_contract_shape_and_can_be_read() {
    let (_dir, app) = app().await;
    let response = app
        .clone()
        .oneshot(json_request(
            "POST",
            "/api/v1/threads",
            json!({
                "title": "Handoff",
                "author": "claude",
                "body": "Please continue",
                "tags": ["work"]
            }),
        ))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::CREATED);
    let created: CreateThreadResponse = body_json(response).await;

    let response = app
        .oneshot(
            Request::builder()
                .uri(format!("/api/v1/threads/{}", created.thread.id))
                .header(header::HOST, "localhost:8787")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
}

#[tokio::test]
async fn validation_error_has_stable_json_shape() {
    let (_dir, app) = app().await;
    let response = app
        .oneshot(json_request(
            "POST",
            "/api/v1/threads",
            json!({"title":"", "author":"claude", "body":"x", "tags":[]}),
        ))
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);
    let error: ErrorEnvelope = body_json(response).await;
    assert_eq!(error.error.code, "invalid_input");
}

#[tokio::test]
async fn unknown_or_external_host_is_rejected() {
    let (_dir, app) = app().await;
    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/v1/boards")
                .header(header::HOST, "attacker.example")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::FORBIDDEN);
    let error: ErrorEnvelope = body_json(response).await;
    assert_eq!(error.error.code, "invalid_host");
}

#[tokio::test]
async fn closed_thread_reply_returns_conflict_without_post() {
    let (_dir, app) = app().await;
    let response = app
        .clone()
        .oneshot(json_request(
            "POST",
            "/api/v1/threads",
            json!({"title":"Done", "author":"claude", "body":"x", "tags":[]}),
        ))
        .await
        .unwrap();
    let created: CreateThreadResponse = body_json(response).await;

    let response = app
        .clone()
        .oneshot(json_request(
            "PATCH",
            &format!("/api/v1/threads/{}", created.thread.id),
            json!({"actor":"claude", "status":"closed"}),
        ))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    let response = app
        .oneshot(json_request(
            "POST",
            &format!("/api/v1/threads/{}/posts", created.thread.id),
            json!({"author":"codex", "body":"late"}),
        ))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::CONFLICT);
    let error: ErrorEnvelope = body_json(response).await;
    assert_eq!(error.error.code, "thread_closed");
}

#[tokio::test]
async fn search_and_sse_expose_created_thread() {
    let (_dir, app) = app().await;
    let response = app
        .clone()
        .oneshot(json_request(
            "POST",
            "/api/v1/threads",
            json!({
                "title":"SSE observation",
                "author":"claude",
                "body":"needle in a durable event",
                "tags":["events"]
            }),
        ))
        .await
        .unwrap();
    let created: CreateThreadResponse = body_json(response).await;

    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/v1/search?q=needle&tag=events")
                .header(header::HOST, "localhost:8787")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let search: Value = body_json(response).await;
    assert_eq!(search["items"][0]["id"], created.thread.id);

    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/v1/events?after=0")
                .header(header::HOST, "127.0.0.1:8787")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        response.headers().get(header::CONTENT_TYPE).unwrap(),
        "text/event-stream"
    );
    let frame = response.into_body().frame().await.unwrap().unwrap();
    let data = String::from_utf8(frame.into_data().unwrap().to_vec()).unwrap();
    assert!(data.contains("event: thread.created"));
    assert!(data.contains(&created.thread.id));
}
