use std::time::Duration;

use rmcp::{
    Json, ServerHandler, ServiceExt,
    handler::server::{router::tool::ToolRouter, wrapper::Parameters},
    model::{CallToolResult, ContentBlock, ServerCapabilities, ServerInfo},
    schemars, tool, tool_handler, tool_router,
    transport::{
        StreamableHttpService,
        streamable_http_server::{StreamableHttpServerConfig, session::local::LocalSessionManager},
    },
};
use schemars::JsonSchema;
use serde::Deserialize;
use tokio_util::sync::CancellationToken;

use crate::{
    domain::{
        BoardsResponse, CreateThreadInput, CursorResponse, ErrorBody, ErrorEnvelope,
        EventsResponse, ListThreadsInput, Page, ReplyInput, SearchInput, ThreadDetail,
        ThreadRecord, ThreadStatus, UpdateThreadInput, default_limit,
    },
    service::{AppError, AppService},
};

#[derive(Debug, Clone)]
pub struct KakitsugiMcp {
    service: AppService,
    tool_router: ToolRouter<Self>,
}

impl KakitsugiMcp {
    pub fn new(service: AppService) -> Self {
        Self {
            service,
            tool_router: Self::tool_router(),
        }
    }
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct ListThreadsArgs {
    #[serde(default)]
    pub status: Option<ThreadStatus>,
    #[serde(default)]
    pub tag: Option<String>,
    #[serde(default = "default_limit")]
    pub limit: u32,
    #[serde(default)]
    pub offset: u32,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct GetThreadArgs {
    pub thread_id: String,
    #[serde(default)]
    pub after: i64,
    #[serde(default = "default_limit")]
    pub limit: u32,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct ReplyArgs {
    pub thread_id: String,
    pub author: String,
    pub body: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct SearchArgs {
    pub query: String,
    #[serde(default)]
    pub status: Option<ThreadStatus>,
    #[serde(default)]
    pub tag: Option<String>,
    #[serde(default = "default_limit")]
    pub limit: u32,
    #[serde(default)]
    pub offset: u32,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct WaitForUpdatesArgs {
    pub after: i64,
    #[serde(default = "default_timeout_ms")]
    pub timeout_ms: u64,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct UpdateThreadArgs {
    pub thread_id: String,
    pub actor: String,
    #[serde(default)]
    pub status: Option<ThreadStatus>,
    #[serde(default)]
    pub tags: Option<Vec<String>>,
}

fn default_timeout_ms() -> u64 {
    30_000
}

#[tool_router]
impl KakitsugiMcp {
    #[tool(description = "List the local bulletin boards available to agents")]
    async fn list_boards(&self) -> Result<Json<BoardsResponse>, CallToolResult> {
        self.service
            .list_boards()
            .await
            .map(|items| Json(BoardsResponse { items }))
            .map_err(mcp_error)
    }

    #[tool(description = "List threads, optionally filtered by status or tag")]
    async fn list_threads(
        &self,
        Parameters(args): Parameters<ListThreadsArgs>,
    ) -> Result<Json<Page<ThreadRecord>>, CallToolResult> {
        self.service
            .list_threads(ListThreadsInput {
                status: args.status,
                tag: args.tag,
                limit: args.limit,
                offset: args.offset,
            })
            .await
            .map(Json)
            .map_err(mcp_error)
    }

    #[tool(description = "Get one thread and an ordered page of its posts")]
    async fn get_thread(
        &self,
        Parameters(args): Parameters<GetThreadArgs>,
    ) -> Result<Json<ThreadDetail>, CallToolResult> {
        self.service
            .get_thread(&args.thread_id, args.after, args.limit)
            .await
            .map(Json)
            .map_err(mcp_error)
    }

    #[tool(description = "Create a thread with its first immutable post")]
    async fn create_thread(
        &self,
        Parameters(input): Parameters<CreateThreadInput>,
    ) -> Result<Json<crate::domain::CreateThreadResponse>, CallToolResult> {
        self.service
            .create_thread(input)
            .await
            .map(Json)
            .map_err(mcp_error)
    }

    #[tool(description = "Append an immutable reply to an open thread")]
    async fn reply(
        &self,
        Parameters(args): Parameters<ReplyArgs>,
    ) -> Result<Json<crate::domain::ReplyResponse>, CallToolResult> {
        self.service
            .reply(
                &args.thread_id,
                ReplyInput {
                    author: args.author,
                    body: args.body,
                },
            )
            .await
            .map(Json)
            .map_err(mcp_error)
    }

    #[tool(description = "Search thread titles and complete post text")]
    async fn search(
        &self,
        Parameters(args): Parameters<SearchArgs>,
    ) -> Result<Json<Page<ThreadRecord>>, CallToolResult> {
        self.service
            .search(SearchInput {
                query: args.query,
                status: args.status,
                tag: args.tag,
                limit: args.limit,
                offset: args.offset,
            })
            .await
            .map(Json)
            .map_err(mcp_error)
    }

    #[tool(description = "Get the latest durable event id before waiting for new updates")]
    async fn get_cursor(&self) -> Result<Json<CursorResponse>, CallToolResult> {
        self.service
            .latest_event_id()
            .await
            .map(|latest_event_id| Json(CursorResponse { latest_event_id }))
            .map_err(mcp_error)
    }

    #[tool(description = "Wait for durable updates after an event cursor")]
    async fn wait_for_updates(
        &self,
        Parameters(args): Parameters<WaitForUpdatesArgs>,
    ) -> Result<Json<EventsResponse>, CallToolResult> {
        self.service
            .wait_for_updates(args.after, Duration::from_millis(args.timeout_ms))
            .await
            .map(|items| Json(EventsResponse { items }))
            .map_err(mcp_error)
    }

    #[tool(description = "Change a thread status or replace its tags, recording the actor")]
    async fn update_thread(
        &self,
        Parameters(args): Parameters<UpdateThreadArgs>,
    ) -> Result<Json<crate::domain::UpdateThreadResponse>, CallToolResult> {
        self.service
            .update_thread(
                &args.thread_id,
                UpdateThreadInput {
                    actor: args.actor,
                    status: args.status,
                    tags: args.tags,
                },
            )
            .await
            .map(Json)
            .map_err(mcp_error)
    }
}

#[tool_handler(router = self.tool_router)]
impl ServerHandler for KakitsugiMcp {
    fn get_info(&self) -> ServerInfo {
        ServerInfo::new(ServerCapabilities::builder().enable_tools().build()).with_instructions(
            "Use Kakitsugi for durable coordination between trusted local AI agents. Search or list threads before creating a new topic, and reply to an existing thread when it already covers the work. Use a stable author name. create_thread and reply are non-idempotent, so do not retry them when the outcome is unknown. Before waiting for only new activity, call get_cursor once and pass latest_event_id to wait_for_updates; advance the cursor to the greatest returned event id after processing each batch. Close completed threads with update_thread. Kakitsugi has no authentication and must remain local-only.",
        )
    }
}

pub type HttpMcpService = StreamableHttpService<KakitsugiMcp, LocalSessionManager>;

pub fn http_service(service: AppService, cancellation: CancellationToken) -> HttpMcpService {
    StreamableHttpService::new(
        move || Ok(KakitsugiMcp::new(service.clone())),
        Default::default(),
        StreamableHttpServerConfig::default()
            .with_allowed_hosts(["localhost", "127.0.0.1", "::1"])
            .with_cancellation_token(cancellation),
    )
}

pub async fn serve_stdio(service: AppService) -> anyhow::Result<()> {
    let running = KakitsugiMcp::new(service)
        .serve(rmcp::transport::stdio())
        .await?;
    running.waiting().await?;
    Ok(())
}

/// 業務エラーを `isError: true` のツール結果へ変換する。
///
/// 本文は REST と同じ `{"error":{"code","message"}}` の JSON テキストとする。
/// 成功時の outputSchema と形が異なるため、`structuredContent` は付けない。
fn mcp_error(error: AppError) -> CallToolResult {
    let (code, message) = match error {
        AppError::InvalidInput(message) => ("invalid_input", message),
        AppError::NotFound => ("not_found", "resource not found".into()),
        AppError::ThreadClosed => ("thread_closed", "thread is closed".into()),
        AppError::Internal(message) => {
            tracing::error!(error = %message, "MCP tool failed");
            ("internal_error", "internal server error".into())
        }
    };
    let envelope = ErrorEnvelope {
        error: ErrorBody {
            code: code.to_owned(),
            message,
        },
    };
    let text = serde_json::to_string(&envelope).expect("serializing an error cannot fail");
    CallToolResult::error(vec![ContentBlock::text(text)])
}
