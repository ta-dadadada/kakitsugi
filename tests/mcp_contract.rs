use std::{collections::BTreeSet, time::Duration};

use kakitsugi::{
    api,
    domain::{
        CreateThreadResponse, CursorResponse, ErrorBody, ErrorEnvelope, EventsResponse, Page,
        ReplyResponse, ThreadDetail, ThreadRecord, ThreadStatus, UpdateThreadResponse,
    },
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

async fn stdio_client(
    database: &std::path::Path,
) -> anyhow::Result<rmcp::service::RunningService<rmcp::RoleClient, ()>> {
    let transport = TokioChildProcess::new({
        let mut command = Command::new(env!("CARGO_BIN_EXE_kakitsugi"));
        command.arg("--database").arg(database).arg("mcp");
        command
    })?;
    Ok(().serve(transport).await?)
}

async fn call(
    client: &rmcp::service::RunningService<rmcp::RoleClient, ()>,
    name: &'static str,
    value: Value,
) -> anyhow::Result<rmcp::model::CallToolResult> {
    Ok(client
        .call_tool(CallToolRequestParams::new(name).with_arguments(arguments(value)))
        .await?)
}

/// ツールエラーの本文から REST と同じエラー形状を取り出す。
fn tool_error(result: rmcp::model::CallToolResult) -> ErrorBody {
    assert_eq!(result.is_error, Some(true), "{result:?}");
    assert!(result.structured_content.is_none());
    let text = result
        .content
        .iter()
        .find_map(|content| content.as_text())
        .expect("tool error should contain text")
        .text
        .clone();
    serde_json::from_str::<ErrorEnvelope>(&text).unwrap().error
}

#[tokio::test]
async fn business_errors_are_tool_errors_with_rest_codes() -> anyhow::Result<()> {
    let dir = TempDir::new()?;
    let client = stdio_client(&dir.path().join("kakitsugi.sqlite3")).await?;

    let missing = tool_error(
        call(
            &client,
            "reply",
            json!({"thread_id": "missing", "author": "codex", "body": "hello"}),
        )
        .await?,
    );
    assert_eq!(missing.code, "not_found");
    assert_eq!(missing.message, "resource not found");

    let invalid = tool_error(
        call(
            &client,
            "create_thread",
            json!({"title": "  ", "author": "claude", "body": "body"}),
        )
        .await?,
    );
    assert_eq!(invalid.code, "invalid_input");

    let timeout = tool_error(
        call(
            &client,
            "wait_for_updates",
            json!({"after": 0, "timeout_ms": 30_001}),
        )
        .await?,
    );
    assert_eq!(timeout.code, "invalid_input");

    let threads: Page<ThreadRecord> = structured(call(&client, "list_threads", json!({})).await?);
    assert!(threads.items.is_empty(), "failed writes must not persist");
    let cursor: CursorResponse = structured(call(&client, "get_cursor", json!({})).await?);
    assert_eq!(
        cursor.latest_event_id, 0,
        "failed writes must not emit events"
    );

    let _ = client.cancel().await;
    Ok(())
}

#[tokio::test]
async fn closed_thread_rejects_replies_until_reopened_over_mcp() -> anyhow::Result<()> {
    let dir = TempDir::new()?;
    let client = stdio_client(&dir.path().join("kakitsugi.sqlite3")).await?;
    let created: CreateThreadResponse = structured(
        call(
            &client,
            "create_thread",
            json!({"title": "Close over MCP", "author": "claude", "body": "start"}),
        )
        .await?,
    );
    let thread_id = created.thread.id.clone();

    let closed: UpdateThreadResponse = structured(
        call(
            &client,
            "update_thread",
            json!({"thread_id": thread_id, "actor": "claude", "status": "closed"}),
        )
        .await?,
    );
    assert!(closed.changed);
    assert_eq!(closed.thread.status, ThreadStatus::Closed);

    let rejected = tool_error(
        call(
            &client,
            "reply",
            json!({"thread_id": thread_id, "author": "codex", "body": "too late"}),
        )
        .await?,
    );
    assert_eq!(rejected.code, "thread_closed");
    assert_eq!(rejected.message, "thread is closed");

    let reopened: UpdateThreadResponse = structured(
        call(
            &client,
            "update_thread",
            json!({"thread_id": thread_id, "actor": "codex", "status": "open"}),
        )
        .await?,
    );
    assert_eq!(reopened.thread.status, ThreadStatus::Open);
    let replied: ReplyResponse = structured(
        call(
            &client,
            "reply",
            json!({"thread_id": thread_id, "author": "codex", "body": "reopened"}),
        )
        .await?,
    );
    let detail: ThreadDetail =
        structured(call(&client, "get_thread", json!({"thread_id": thread_id})).await?);
    assert_eq!(detail.posts.len(), 2);
    assert_eq!(detail.posts[1].id, replied.post.id);

    let _ = client.cancel().await;
    Ok(())
}

#[tokio::test]
async fn search_filters_by_status_and_tag_over_mcp() -> anyhow::Result<()> {
    let dir = TempDir::new()?;
    let client = stdio_client(&dir.path().join("kakitsugi.sqlite3")).await?;
    let mut ids = vec![];
    for (title, tag) in [("検索対象の調査", "research"), ("検索対象の実装", "impl")] {
        let created: CreateThreadResponse = structured(
            call(
                &client,
                "create_thread",
                json!({"title": title, "author": "claude", "body": "共通の本文", "tags": [tag]}),
            )
            .await?,
        );
        ids.push(created.thread.id);
    }
    structured::<UpdateThreadResponse>(
        call(
            &client,
            "update_thread",
            json!({"thread_id": ids[1], "actor": "claude", "status": "closed"}),
        )
        .await?,
    );

    let all: Page<ThreadRecord> =
        structured(call(&client, "search", json!({"query": "共通"})).await?);
    assert_eq!(all.items.len(), 2);
    let by_tag: Page<ThreadRecord> = structured(
        call(
            &client,
            "search",
            json!({"query": "共通", "tag": "research"}),
        )
        .await?,
    );
    assert_eq!(
        by_tag.items.iter().map(|t| &t.id).collect::<Vec<_>>(),
        vec![&ids[0]]
    );
    let closed: Page<ThreadRecord> = structured(
        call(
            &client,
            "search",
            json!({"query": "検索対象", "status": "closed"}),
        )
        .await?,
    );
    assert_eq!(
        closed.items.iter().map(|t| &t.id).collect::<Vec<_>>(),
        vec![&ids[1]]
    );
    let none: Page<ThreadRecord> =
        structured(call(&client, "search", json!({"query": "存在しない語"})).await?);
    assert!(none.items.is_empty());
    let empty = tool_error(call(&client, "search", json!({"query": "  "})).await?);
    assert_eq!(empty.code, "invalid_input");

    let _ = client.cancel().await;
    Ok(())
}
