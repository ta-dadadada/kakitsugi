use std::time::Duration;

use agent_bbs::{
    api,
    domain::{
        CreateThreadResponse, ErrorEnvelope, PostPage, ReplyResponse, ThreadStatus,
        UpdateThreadResponse,
    },
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
async fn rest_lifecycle_covers_boards_lists_posts_replies_and_updates() {
    let (_dir, app) = app().await;

    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/v1/boards")
                .header(header::HOST, "127.0.0.1:8787")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let boards: Value = body_json(response).await;
    assert_eq!(boards[0]["id"], "general");

    let response = app
        .clone()
        .oneshot(json_request(
            "POST",
            "/api/v1/threads",
            json!({
                "title": "REST lifecycle",
                "author": "claude",
                "body": "initial",
                "tags": ["contract"]
            }),
        ))
        .await
        .unwrap();
    let created: CreateThreadResponse = body_json(response).await;

    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/v1/threads?status=open&tag=contract")
                .header(header::HOST, "127.0.0.1:8787")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let threads: Value = body_json(response).await;
    assert_eq!(threads["items"][0]["id"], created.thread.id);

    let response = app
        .clone()
        .oneshot(json_request(
            "POST",
            &format!("/api/v1/threads/{}/posts", created.thread.id),
            json!({"author":"codex", "body":"reply"}),
        ))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::CREATED);
    let reply: ReplyResponse = body_json(response).await;
    assert!(reply.event_id > created.event_id);

    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!(
                    "/api/v1/threads/{}/posts?after=0&limit=100",
                    created.thread.id
                ))
                .header(header::HOST, "127.0.0.1:8787")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let posts: PostPage = body_json(response).await;
    assert_eq!(posts.items.len(), 2);
    assert_eq!(posts.items[1].body, "reply");

    let response = app
        .clone()
        .oneshot(json_request(
            "PATCH",
            &format!("/api/v1/threads/{}", created.thread.id),
            json!({"actor":"codex", "status":"closed", "tags":["done"]}),
        ))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let updated: UpdateThreadResponse = body_json(response).await;
    assert!(updated.changed);
    assert_eq!(updated.thread.status, ThreadStatus::Closed);
    assert_eq!(updated.thread.tags, vec!["done"]);
}

#[tokio::test]
async fn invalid_query_unknown_resource_and_method_use_stable_errors() {
    let (_dir, app) = app().await;
    for (method, uri, expected_status, expected_code) in [
        (
            "GET",
            "/api/v1/threads?limit=0",
            StatusCode::UNPROCESSABLE_ENTITY,
            "invalid_input",
        ),
        (
            "GET",
            "/api/v1/threads/missing",
            StatusCode::NOT_FOUND,
            "not_found",
        ),
        ("GET", "/api/v1/unknown", StatusCode::NOT_FOUND, "not_found"),
        (
            "DELETE",
            "/api/v1/boards",
            StatusCode::METHOD_NOT_ALLOWED,
            "method_not_allowed",
        ),
    ] {
        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method(method)
                    .uri(uri)
                    .header(header::HOST, "127.0.0.1:8787")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), expected_status);
        let error: ErrorEnvelope = body_json(response).await;
        assert_eq!(error.error.code, expected_code);
    }
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
async fn external_or_malformed_host_is_rejected() {
    let (_dir, app) = app().await;
    for host in [
        "attacker.example",
        "[::1]evil",
        "localhost:+80",
        "[::1]:+80",
    ] {
        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri("/api/v1/boards")
                    .header(header::HOST, host)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::FORBIDDEN);
        let error: ErrorEnvelope = body_json(response).await;
        assert_eq!(error.error.code, "invalid_host");
    }
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

#[tokio::test]
async fn sse_waits_for_an_event_created_after_the_cursor() {
    let (_dir, app) = app().await;
    let response = app
        .clone()
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
    let mut body = response.into_body();
    let frame = body.frame();
    tokio::pin!(frame);
    assert!(
        tokio::time::timeout(Duration::from_millis(100), &mut frame)
            .await
            .is_err(),
        "SSE should still be waiting before a new event is created"
    );

    let created_response = app
        .oneshot(json_request(
            "POST",
            "/api/v1/threads",
            json!({
                "title":"Wake the SSE stream",
                "author":"claude",
                "body":"created after subscription",
                "tags":[]
            }),
        ))
        .await
        .unwrap();
    let created: CreateThreadResponse = body_json(created_response).await;

    let frame = tokio::time::timeout(Duration::from_secs(2), &mut frame)
        .await
        .expect("SSE should emit before the timeout")
        .expect("SSE stream should remain open")
        .expect("SSE body should not fail");
    let data = String::from_utf8(frame.into_data().unwrap().to_vec()).unwrap();
    assert!(data.contains(&format!("id: {}", created.event_id)));
    assert!(data.contains("event: thread.created"));
}
