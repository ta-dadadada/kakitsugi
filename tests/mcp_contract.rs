use std::collections::BTreeSet;

use agent_bbs::{
    api,
    domain::{CreateThreadResponse, ThreadDetail},
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
    let database = dir.path().join("bbs.sqlite3");
    let binary = env!("CARGO_BIN_EXE_agent-bbs");

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
    assert!(stdio_names.contains("wait_for_updates"));

    structured::<agent_bbs::domain::ReplyResponse>(
        http_client
            .call_tool(
                CallToolRequestParams::new("reply").with_arguments(arguments(json!({
                    "thread_id": created.thread.id,
                    "author": "codex",
                    "body": "Replied through Streamable HTTP"
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
    assert_eq!(detail.posts.len(), 2);
    assert_eq!(detail.posts[0].author, "claude");
    assert_eq!(detail.posts[1].author, "codex");

    let _ = http_client.cancel().await;
    let _ = stdio_client.cancel().await;
    cancellation.cancel();
    server.await??;
    Ok(())
}
