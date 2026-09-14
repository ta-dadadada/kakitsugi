use std::{collections::BTreeSet, time::Duration};

use kakitsugi::{
    api,
    domain::{CreateThreadResponse, CursorResponse, EventsResponse, ReplyResponse, ThreadDetail},
    mcp,
    service::AppService,
};
use rmcp::{
    ServiceExt,
    model::{CallToolRequestParams, ClientInfo},
    transport::{StreamableHttpClientTransport, TokioChildProcess},
};
use serde::de::DeserializeOwned;
use serde_json::{Map, Value, json};
use tempfile::TempDir;
use tokio::process::Command;
use tokio_util::sync::CancellationToken;

fn arguments(value: Value) -> Map<String, Value> {
    value.as_object().cloned().unwrap()
}

fn structured<T: DeserializeOwned>(result: rmcp::model::CallToolResult) -> T {
    assert_ne!(result.is_error, Some(true));
    serde_json::from_value(result.structured_content.unwrap()).unwrap()
}

#[tokio::test]
async fn stdio_and_http_clients_share_tools_and_database() -> anyhow::Result<()> {
    let dir = TempDir::new()?;
    let database = dir.path().join("kakitsugi.sqlite3");
    let binary = env!("CARGO_BIN_EXE_kakitsugi");

    let stdio_transport = TokioChildProcess::new({
        let mut command = Command::new(binary);
        command.arg("--database").arg(&database).arg("mcp");
        command
    })?;
    let stdio_client = ().serve(stdio_transport).await?;
    let stdio_tools = stdio_client.list_tools(Default::default()).await?;

    let created: CreateThreadResponse = structured(
        stdio_client
            .call_tool(
                CallToolRequestParams::new("create_thread").with_arguments(arguments(json!({
                    "title": "Cross-transport handoff",
                    "author": "claude",
                    "body": "Created through stdio",
                    "tags": ["handoff"]
                }))),
            )
            .await?,
    );

    let service = AppService::open(database.clone()).await?;
    let cancellation = CancellationToken::new();
    let app = api::router(service.clone()).nest_service(
        "/mcp",
        mcp::http_service(service, cancellation.child_token()),
    );
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
    let address = listener.local_addr()?;
    let server = tokio::spawn({
        let cancellation = cancellation.clone();
        async move {
            axum::serve(listener, app)
                .with_graceful_shutdown(cancellation.cancelled_owned())
                .await
        }
    });

    let http_client = ClientInfo::default()
        .serve(StreamableHttpClientTransport::from_uri(format!(
            "http://{address}/mcp"
        )))
        .await?;
    let http_tools = http_client.list_tools(Default::default()).await?;

    let stdio_names: BTreeSet<_> = stdio_tools
        .tools
        .into_iter()
        .map(|tool| tool.name.into_owned())
        .collect();
    let http_names: BTreeSet<_> = http_tools
        .tools
        .into_iter()
        .map(|tool| tool.name.into_owned())
        .collect();
    assert_eq!(stdio_names, http_names);
    assert!(stdio_names.contains("get_cursor"));
    assert!(stdio_names.contains("wait_for_updates"));

    let (waited, replied): (EventsResponse, ReplyResponse) = {
        let wait = http_client.call_tool(
            CallToolRequestParams::new("wait_for_updates").with_arguments(arguments(json!({
                "after": created.event_id,
                "timeout_ms": 2_000
            }))),
        );
        tokio::pin!(wait);
        assert!(
            tokio::time::timeout(Duration::from_millis(100), &mut wait)
                .await
                .is_err(),
            "wait_for_updates should still be pending before the reply"
        );

        let replied = structured(
            stdio_client
                .call_tool(
                    CallToolRequestParams::new("reply").with_arguments(arguments(json!({
                        "thread_id": created.thread.id,
                        "author": "codex",
                        "body": "Replied through stdio while HTTP waits"
                    }))),
                )
                .await?,
        );
        let waited = structured(
            tokio::time::timeout(Duration::from_secs(2), &mut wait)
                .await
                .expect("wait_for_updates should complete after the reply")?,
        );
        (waited, replied)
    };
    assert_eq!(waited.items.len(), 1);
    assert_eq!(waited.items[0].id, replied.event_id);

    let cursor: CursorResponse = structured(
        http_client
            .call_tool(CallToolRequestParams::new("get_cursor"))
            .await?,
    );
    assert_eq!(cursor.latest_event_id, replied.event_id);

    structured::<ReplyResponse>(
        http_client
            .call_tool(
                CallToolRequestParams::new("reply").with_arguments(arguments(json!({
                    "thread_id": created.thread.id,
                    "author": "codex",
                    "body": "Second reply through Streamable HTTP"
                }))),
            )
            .await?,
    );
    let detail: ThreadDetail = structured(
        http_client
            .call_tool(
                CallToolRequestParams::new("get_thread").with_arguments(arguments(json!({
                    "thread_id": created.thread.id,
                    "after": 0,
                    "limit": 100
                }))),
            )
            .await?,
    );
    assert_eq!(detail.posts.len(), 3);
    assert_eq!(detail.posts[0].author, "claude");
    assert_eq!(detail.posts[1].author, "codex");
    assert_eq!(detail.posts[2].body, "Second reply through Streamable HTTP");

    let _ = http_client.cancel().await;
    let _ = stdio_client.cancel().await;
    cancellation.cancel();
    server.await??;
    Ok(())
}
