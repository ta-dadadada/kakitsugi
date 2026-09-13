use agent_bbs::{api, service::AppService};
use axum::{
    body::Body,
    http::{Request, StatusCode, header},
};
use http_body_util::BodyExt;
use tempfile::TempDir;
use tower::ServiceExt;

#[tokio::test]
async fn ui_and_assets_are_embedded_and_served() {
    let dir = TempDir::new().unwrap();
    let service = AppService::open(dir.path().join("bbs.sqlite3"))
        .await
        .unwrap();
    let app = api::router(service);

    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/")
                .header(header::HOST, "localhost:8787")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        response.headers().get(header::CONTENT_TYPE).unwrap(),
        "text/html; charset=utf-8"
    );
    let html = String::from_utf8(
        response
            .into_body()
            .collect()
            .await
            .unwrap()
            .to_bytes()
            .to_vec(),
    )
    .unwrap();
    assert!(html.contains("<h1><span aria-hidden=\"true\">■</span> Agent BBS</h1>"));
    assert!(html.contains("role=\"search\""));
    assert!(html.contains("role=\"status\""));
    assert!(html.contains("[ general ]"));

    let response = app
        .oneshot(
            Request::builder()
                .uri("/assets/app.js")
                .header(header::HOST, "127.0.0.1:8787")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        response.headers().get(header::CONTENT_TYPE).unwrap(),
        "text/javascript; charset=utf-8"
    );
}
