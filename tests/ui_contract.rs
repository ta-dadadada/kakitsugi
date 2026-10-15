use axum::{
    body::Body,
    http::{Request, StatusCode, header},
};
use http_body_util::BodyExt;
use kakitsugi::{api, service::AppService};
use tempfile::TempDir;
use tower::ServiceExt;

#[tokio::test]
async fn ui_and_assets_are_embedded_and_served() {
    let dir = TempDir::new().unwrap();
    let service = AppService::open(dir.path().join("kakitsugi.sqlite3"))
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
    assert!(html.contains("<h1><span aria-hidden=\"true\">■</span> Kakitsugi</h1>"));
    assert!(html.contains("<html lang=\"en\">"));
    assert!(
        html.contains("href=\"#main-content\" data-i18n=\"skipLink\">Skip to main content</a>")
    );
    assert!(html.contains("<main id=\"main-content\" class=\"workspace\" tabindex=\"-1\">"));
    assert!(html.contains("role=\"search\""));
    assert!(html.contains("role=\"status\""));
    assert!(html.contains("role=\"list\" aria-label=\"Tags\" data-i18n-aria-label=\"tagsLabel\""));
    assert!(html.contains("[ general ]"));

    let response = app
        .clone()
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
    let script = String::from_utf8(
        response
            .into_body()
            .collect()
            .await
            .unwrap()
            .to_bytes()
            .to_vec(),
    )
    .unwrap();
    assert!(script.contains("restoreListFocus: !state.thread"));
    assert!(script.contains("new Intl.DateTimeFormat(locale, {"));
    assert_messages_cover_html(&html, &script);

    let response = app
        .oneshot(
            Request::builder()
                .uri("/assets/styles.css")
                .header(header::HOST, "127.0.0.1:8787")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        response.headers().get(header::CONTENT_TYPE).unwrap(),
        "text/css; charset=utf-8"
    );
    let styles = String::from_utf8(
        response
            .into_body()
            .collect()
            .await
            .unwrap()
            .to_bytes()
            .to_vec(),
    )
    .unwrap();
    assert!(styles.contains(".status-badge.closed {\n  color: #555555;"));
}

/// HTML の data-i18n／data-i18n-aria-label が参照するキーを、英語と日本語の辞書が両方とも定義していることを確認する。
fn assert_messages_cover_html(html: &str, script: &str) {
    let en_start = script
        .find("    en: {\n")
        .expect("English messages are missing");
    let ja_start = script
        .find("    ja: {\n")
        .expect("Japanese messages are missing");
    let ja_end = script[ja_start..]
        .find("\n    },\n")
        .map(|offset| ja_start + offset)
        .expect("Japanese messages are not terminated");
    let english = &script[en_start..ja_start];
    let japanese = &script[ja_start..ja_end];

    let mut keys = Vec::new();
    for attribute in ["data-i18n=\"", "data-i18n-aria-label=\""] {
        for (index, _) in html.match_indices(attribute) {
            let rest = &html[index + attribute.len()..];
            keys.push(&rest[..rest.find('"').unwrap()]);
        }
    }
    assert!(
        keys.len() >= 15,
        "expected translated UI text, found {keys:?}"
    );
    for key in keys {
        let definition = format!("\n      {key}: ");
        assert!(
            english.contains(&definition),
            "English message {key} is missing"
        );
        assert!(
            japanese.contains(&definition),
            "Japanese message {key} is missing"
        );
    }

    let count_keys = |messages: &str| {
        messages
            .lines()
            .filter(|line| {
                line.strip_prefix("      ")
                    .and_then(|rest| rest.split_once(": "))
                    .is_some_and(|(key, _)| key.chars().all(|c| c.is_ascii_alphanumeric()))
            })
            .count()
    };
    assert_eq!(
        count_keys(english),
        count_keys(japanese),
        "English and Japanese messages must define the same keys"
    );
}
