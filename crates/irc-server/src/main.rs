//! IRC server CLI entry point.

use std::path::PathBuf;

use clap::Parser;
use tracing_subscriber::{EnvFilter, fmt, prelude::*};

use irc_server::{Server, ServerConfig};

mod update;

/// IRC server daemon.
#[derive(Parser, Debug)]
#[command(name = "irc-server")]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Path to configuration file
    #[arg(short, long, value_name = "FILE")]
    config: Option<PathBuf>,

    /// Bind address (overrides config)
    #[arg(short, long, value_name = "ADDR")]
    bind: Option<String>,

    /// Run in foreground (don't daemonize)
    #[arg(short, long)]
    foreground: bool,

    /// Enable verbose logging
    #[arg(short, long, action = clap::ArgAction::Count)]
    verbose: u8,

    /// Update to latest version (keeps previous as .backup)
    #[arg(long)]
    update: bool,

    /// Check for available updates
    #[arg(long)]
    check_update: bool,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let args = Args::parse();

    // Handle --check-update
    if args.check_update {
        match update::check_update().await {
            Ok(Some(version)) => {
                println!("New version available: {} -> {}", update::VERSION, version);
                println!("Run 'irc-server --update' to install.");
            }
            Ok(None) => {
                println!("Already up to date ({})", update::VERSION);
            }
            Err(e) => {
                eprintln!("Error checking for updates: {}", e);
                std::process::exit(1);
            }
        }
        return Ok(());
    }

    // Handle --update
    if args.update {
        match update::update().await {
            update::UpdateResult::Updated { from, to, backup } => {
                println!("Updated successfully: {} -> {}", from, to);
                println!("Previous version saved to: {}", backup.display());
            }
            update::UpdateResult::UpToDate { version } => {
                println!("Already up to date ({})", version);
            }
            update::UpdateResult::Error(e) => {
                eprintln!("Update failed: {}", e);
                std::process::exit(1);
            }
        }
        return Ok(());
    }

    // Initialize logging
    let filter = match args.verbose {
        0 => "info",
        1 => "debug",
        _ => "trace",
    };

    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new(filter));

    tracing_subscriber::registry()
        .with(fmt::layer())
        .with(filter)
        .init();

    // Load configuration
    let mut config = if let Some(ref path) = args.config {
        let content = std::fs::read_to_string(path)
            .map_err(|e| anyhow::anyhow!("Failed to read config file {:?}: {}", path, e))?;
        toml::from_str(&content)
            .map_err(|e| anyhow::anyhow!("Failed to parse config file {:?}: {}", path, e))?
    } else {
        ServerConfig::default()
    };

    // Override bind address if specified
    if let Some(ref bind) = args.bind {
        let addr = bind
            .parse()
            .map_err(|e| anyhow::anyhow!("Invalid bind address: {}", e))?;
        config.listen = vec![irc_server::config::ListenConfig {
            address: addr,
            tls: None,
        }];
    }

    // Run the server
    let server = Server::new(config);
    server.run().await?;

    Ok(())
}
