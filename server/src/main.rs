mod config;
mod git;
mod index;
mod mcp;
mod ops;
mod review;
mod server;
mod symbols;

use std::path::PathBuf;

use clap::Parser;
use tracing::info;

use rmcp::ServiceExt;
use server::state::AppState;

#[derive(Parser)]
#[command(name = "coderlm", about = "CoderLM REPL server for code-aware agent sessions")]
struct Cli {
    /// Subcommand
    #[command(subcommand)]
    command: Commands,
}

#[derive(clap::Subcommand)]
enum Commands {
    /// Start the REPL server against a codebase
    Serve {
        /// Optional initial project directory to pre-index
        path: Option<PathBuf>,

        /// Port to listen on
        #[arg(short, long, default_value = "3000")]
        port: u16,

        /// Bind address
        #[arg(short, long, default_value = "127.0.0.1")]
        bind: String,

        /// Maximum file size in bytes to index
        #[arg(long, default_value_t = config::DEFAULT_MAX_FILE_SIZE)]
        max_file_size: u64,

        /// Maximum number of concurrent indexed projects
        #[arg(long, default_value = "5")]
        max_projects: usize,

        /// Verbose logging: -v debug, -vv trace (overridden by RUST_LOG)
        #[arg(short, long, action = clap::ArgAction::Count)]
        verbose: u8,
    },

    /// Start an MCP server over stdio for a single project directory
    Mcp {
        /// Project directory to index
        path: PathBuf,

        /// Maximum file size in bytes to index
        #[arg(long, default_value_t = config::DEFAULT_MAX_FILE_SIZE)]
        max_file_size: u64,

        /// Verbose logging: -v debug, -vv trace (overridden by RUST_LOG)
        #[arg(short, long, action = clap::ArgAction::Count)]
        verbose: u8,
    },
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Serve { path, port, bind, max_file_size, max_projects, verbose } => {
            let default_filter = match verbose {
                0 => "info",
                1 => "coderlm_server=debug,tower_http=debug",
                _ => "coderlm_server=trace,tower_http=debug",
            };
            tracing_subscriber::fmt()
                .with_env_filter(
                    tracing_subscriber::EnvFilter::try_from_default_env()
                        .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new(default_filter)),
                )
                .init();

            info!("coderlm v{}", env!("CARGO_PKG_VERSION"));
            run_server(path, port, bind, max_file_size, max_projects).await?;
        }

        Commands::Mcp { path, max_file_size, verbose } => {
            // Log to stderr — stdout is reserved for MCP JSON-RPC.
            let default_filter = match verbose {
                0 => "info",
                1 => "coderlm_server=debug",
                _ => "coderlm_server=trace",
            };
            tracing_subscriber::fmt()
                .with_writer(std::io::stderr)
                .with_ansi(false)
                .with_env_filter(
                    tracing_subscriber::EnvFilter::try_from_default_env()
                        .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new(default_filter)),
                )
                .init();

            info!("coderlm-mcp v{} indexing {}", env!("CARGO_PKG_VERSION"), path.display());
            let server = mcp::server::McpServer::new(path, max_file_size).await?;
            let service = server.serve(rmcp::transport::stdio()).await
                .map_err(|e| anyhow::anyhow!("MCP transport error: {e}"))?;
            service.waiting().await
                .map_err(|e| anyhow::anyhow!("MCP service error: {e}"))?;
        }
    }

    Ok(())
}

async fn run_server(
    path: Option<PathBuf>,
    port: u16,
    bind: String,
    max_file_size: u64,
    max_projects: usize,
) -> anyhow::Result<()> {
    // Create shared state
    let state = AppState::new(max_projects, max_file_size);

    // If an initial path was provided, pre-index it and restore any persisted reviews.
    if let Some(ref p) = path {
        info!("Pre-indexing project: {}", p.display());
        state.get_or_create_project(p).map_err(|e| {
            anyhow::anyhow!("Failed to index '{}': {}", p.display(), e)
        })?;
        state.restore_reviews(p).await;
    }

    // Build router
    let app = server::build_router(state);

    let addr = format!("{}:{}", bind, port);
    let listener = tokio::net::TcpListener::bind(&addr).await?;

    if let Some(ref p) = path {
        info!("coderlm serving {} on http://{}", p.display(), addr);
    } else {
        info!("coderlm server listening on http://{} (no project pre-indexed)", addr);
    }

    axum::serve(listener, app).await?;

    Ok(())
}
