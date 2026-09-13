use std::{
    net::{Ipv4Addr, SocketAddr},
    path::PathBuf,
};

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use directories::ProjectDirs;
use tokio_util::sync::CancellationToken;
use tracing_subscriber::EnvFilter;

use agent_bbs::{api, mcp, service::AppService};

const DEFAULT_PORT: u16 = 8787;

#[derive(Debug, Parser)]
#[command(
    name = "agent-bbs",
    version,
    about = "Local bulletin board for AI agents"
)]
struct Cli {
    /// SQLite database path. Defaults to the platform's local data directory.
    #[arg(long, global = true)]
    database: Option<PathBuf>,

    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Serve the REST API, web UI, SSE, and Streamable HTTP MCP endpoint.
    Serve {
        /// Loopback TCP port.
        #[arg(long, default_value_t = DEFAULT_PORT)]
        port: u16,
    },
    /// Serve MCP over standard input/output.
    Mcp,
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "agent_bbs=info,tower_http=info".into()),
        )
        .with_writer(std::io::stderr)
        .init();

    let cli = Cli::parse();
    let database = cli.database.unwrap_or_else(default_database_path);
    let service = AppService::open(database.clone())
        .await
        .with_context(|| format!("failed to open database at {}", database.display()))?;

    match cli.command.unwrap_or(Command::Serve { port: DEFAULT_PORT }) {
        Command::Serve { port } => serve(service, port).await,
        Command::Mcp => mcp::serve_stdio(service).await,
    }
}

async fn serve(service: AppService, port: u16) -> Result<()> {
    let address = SocketAddr::from((Ipv4Addr::LOCALHOST, port));
    let cancellation = CancellationToken::new();
    let mcp_service = mcp::http_service(service.clone(), cancellation.child_token());
    let app = api::router(service).nest_service("/mcp", mcp_service);
    let listener = tokio::net::TcpListener::bind(address)
        .await
        .with_context(|| format!("failed to bind http://{address}"))?;
    let actual = listener.local_addr()?;
    tracing::info!(url = %format!("http://{actual}"), mcp = %format!("http://{actual}/mcp"), "agent-bbs is ready");

    axum::serve(listener, app)
        .with_graceful_shutdown({
            let cancellation = cancellation.clone();
            async move {
                let _ = tokio::signal::ctrl_c().await;
                cancellation.cancel();
            }
        })
        .await?;
    Ok(())
}

fn default_database_path() -> PathBuf {
    ProjectDirs::from("dev", "agent-bbs", "agent-bbs")
        .map(|dirs| dirs.data_local_dir().join("agent-bbs.sqlite3"))
        .unwrap_or_else(|| PathBuf::from("agent-bbs.sqlite3"))
}
