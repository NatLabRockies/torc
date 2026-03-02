use anyhow::Result;
use clap::Parser;

use crate::client::apis::configuration::BasicAuth;

#[derive(Parser, Debug)]
#[command(about = "Interactive terminal UI for managing workflows", long_about = None)]
pub struct Args {
    /// Start in standalone mode: automatically start a torc-server
    #[arg(long)]
    pub standalone: bool,

    /// Port for the server in standalone mode (default: 8080)
    #[arg(long, default_value = "8080")]
    pub port: u16,

    /// Database path for standalone mode
    #[arg(long)]
    pub database: Option<String>,

    /// Path to a PEM-encoded CA certificate to trust for TLS connections
    #[arg(long, env = "TORC_TLS_CA_CERT")]
    pub tls_ca_cert: Option<String>,

    /// Skip TLS certificate verification (for testing only)
    #[arg(long, env = "TORC_TLS_INSECURE")]
    pub tls_insecure: bool,
}

pub fn run(args: &Args, basic_auth: Option<BasicAuth>) -> Result<()> {
    // Initialize the TUI
    // The TUI code will be in the optional 'tui' module
    #[cfg(feature = "tui")]
    {
        crate::tui::run(
            args.standalone,
            args.port,
            args.database.clone(),
            args.tls_ca_cert.clone(),
            args.tls_insecure,
            basic_auth,
        )
    }

    #[cfg(not(feature = "tui"))]
    {
        let _ = args; // Suppress unused warning
        let _ = basic_auth;
        eprintln!("Error: TUI support was not compiled into this binary");
        eprintln!("Please rebuild with --features tui or use the standalone torc-tui binary");
        std::process::exit(1);
    }
}
