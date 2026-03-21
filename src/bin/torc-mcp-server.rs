//! Torc MCP Server binary.
//!
//! This binary provides an MCP (Model Context Protocol) server that exposes
//! Torc's workflow and job management capabilities as tools for AI assistants.
//!
//! # Usage
//!
//! ```bash
//! # Run with default settings (connects to localhost:8080)
//! torc-mcp-server
//!
//! # Run with custom API URL
//! TORC_API_URL=http://server:8080/torc-service/v1 torc-mcp-server
//!
//! # Run with custom output directory for logs
//! torc-mcp-server --output-dir /path/to/torc_output
//! ```

use anyhow::Result;
use clap::Parser;
use rmcp::{ServiceExt, transport::io::stdio};
use std::path::PathBuf;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

use torc::mcp_server::server::TorcMcpServer;

/// MCP server for Torc workflow orchestration.
///
/// This server exposes Torc's workflow and job management capabilities
/// as tools for AI assistants via the Model Context Protocol (MCP).
#[derive(Parser, Debug)]
#[command(name = "torc-mcp-server")]
#[command(version, about, long_about = None)]
struct Args {
    /// Torc API URL
    #[arg(
        long,
        env = "TORC_API_URL",
        default_value = "http://localhost:8080/torc-service/v1"
    )]
    api_url: String,

    /// Output directory for job logs
    #[arg(long, env = "TORC_OUTPUT_DIR", default_value = "torc_output")]
    output_dir: PathBuf,

    /// Password for API authentication (uses USER env var as username)
    #[arg(long, env = "TORC_PASSWORD")]
    password: Option<String>,

    /// Path to a PEM-encoded CA certificate to trust for TLS connections
    #[arg(long, env = "TORC_TLS_CA_CERT")]
    tls_ca_cert: Option<String>,

    /// Skip TLS certificate verification (for testing only)
    #[arg(long, env = "TORC_TLS_INSECURE")]
    tls_insecure: bool,

    /// Directory containing Torc documentation (docs/src/)
    #[arg(long, env = "TORC_DOCS_DIR")]
    docs_dir: Option<PathBuf>,

    /// Directory containing example workflow specifications
    #[arg(long, env = "TORC_EXAMPLES_DIR")]
    examples_dir: Option<PathBuf>,
}

fn main() -> Result<()> {
    // Parse CLI arguments (handles -h/--help before async runtime)
    let args = Args::parse();

    // Initialize logging to stderr (stdout is used for MCP protocol)
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "torc_mcp_server=info".into()),
        )
        .with(tracing_subscriber::fmt::layer().with_writer(std::io::stderr))
        .init();

    tracing::info!("Starting Torc MCP Server");
    tracing::info!("API URL: {}", args.api_url);
    tracing::info!("Output directory: {}", args.output_dir.display());

    // Build TLS configuration
    let tls = torc::client::apis::configuration::TlsConfig {
        ca_cert_path: args.tls_ca_cert.as_ref().map(std::path::PathBuf::from),
        insecure: args.tls_insecure,
    };

    // Create the server BEFORE entering the async runtime.
    // This is important because TorcMcpServer::new() creates a reqwest::blocking::Client
    // which spawns its own tokio runtime. Creating it inside block_on would cause
    // nested runtime issues.
    let server = if args.password.is_some() {
        let username = torc::get_username();
        TorcMcpServer::with_auth_and_tls(
            args.api_url,
            args.output_dir,
            Some(username),
            args.password,
            tls,
        )
    } else {
        TorcMcpServer::new_with_tls(args.api_url, args.output_dir, tls)
    }
    .with_docs_dir(args.docs_dir)
    .with_examples_dir(args.examples_dir);

    // Build runtime and run the async portion
    // Use multi-threaded runtime to properly support spawn_blocking for the
    // blocking reqwest client calls
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .enable_all()
        .build()?;

    runtime.block_on(async_main(server))
}

async fn async_main(server: TorcMcpServer) -> Result<()> {
    // Serve over stdio transport
    let service = server.serve(stdio()).await?;

    tracing::info!("MCP server running");

    // Wait for the service to complete
    service.waiting().await?;

    Ok(())
}
