//! IRC terminal client.

use std::io;

use clap::Parser;
use crossterm::{
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::backend::CrosstermBackend;
use ratatui::Terminal;
use tracing_subscriber::{fmt, prelude::*, EnvFilter};

use irc_client_lib::ClientConfig;

mod app;
mod completion;
mod config;
mod handler;
mod state;
mod style;
mod ui;
mod update;

use app::App;
use config::AppConfig;

#[derive(Parser)]
#[command(name = "irc")]
#[command(about = "Terminal IRC client")]
struct Args {
    /// Server profile to use from config file
    #[arg(long, value_name = "NAME")]
    profile: Option<String>,

    /// Config file path (default: ~/.config/irc/config.toml)
    #[arg(long, value_name = "PATH")]
    config: Option<std::path::PathBuf>,

    /// Generate example config file and exit
    #[arg(long)]
    gen_config: bool,

    /// Server to connect to
    #[arg(short, long)]
    server: Option<String>,

    /// Port
    #[arg(short, long)]
    port: Option<u16>,

    /// Disable TLS
    #[arg(long)]
    no_tls: bool,

    /// Accept invalid TLS certificates
    #[arg(long)]
    tls_accept_invalid: bool,

    /// Nickname
    #[arg(short, long)]
    nick: Option<String>,

    /// Username
    #[arg(short, long)]
    user: Option<String>,

    /// Real name
    #[arg(short, long)]
    realname: Option<String>,

    /// SASL username
    #[arg(long)]
    sasl_user: Option<String>,

    /// SASL password
    #[arg(long)]
    sasl_pass: Option<String>,

    /// Channel to auto-join (can be specified multiple times)
    #[arg(short = 'c', long = "channel")]
    channels: Vec<String>,

    /// Enable debug logging to ~/.local/share/irc/debug.log
    #[arg(long)]
    debug: bool,

    /// Verbose debug output (trace level)
    #[arg(short, long)]
    verbose: bool,

    /// Update to latest version (keeps previous as .backup)
    #[arg(long)]
    update: bool,

    /// Check for available updates
    #[arg(long)]
    check_update: bool,

    /// Print version
    #[arg(short = 'V', long)]
    version: bool,
}

fn setup_terminal() -> io::Result<Terminal<CrosstermBackend<io::Stdout>>> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    Terminal::new(backend)
}

/// Setup logging to file.
fn setup_logging(debug: bool, verbose: bool) -> Option<tracing_appender::non_blocking::WorkerGuard> {
    if !debug {
        return None;
    }

    // Create log directory
    let log_dir = dirs::data_local_dir()
        .unwrap_or_else(|| std::path::PathBuf::from("."))
        .join("irc");

    if let Err(e) = std::fs::create_dir_all(&log_dir) {
        eprintln!("Warning: Could not create log directory: {}", e);
        return None;
    }

    let log_path = log_dir.join("debug.log");

    // Open log file (append mode)
    let log_file = match std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&log_path)
    {
        Ok(f) => f,
        Err(e) => {
            eprintln!("Warning: Could not open log file: {}", e);
            return None;
        }
    };

    let (non_blocking, guard) = tracing_appender::non_blocking(log_file);

    let level = if verbose { "trace" } else { "debug" };
    let filter = EnvFilter::try_new(format!("irc_client_lib={},irc_cli={}", level, level))
        .unwrap_or_else(|_| EnvFilter::new("debug"));

    tracing_subscriber::registry()
        .with(
            fmt::layer()
                .with_writer(non_blocking)
                .with_ansi(false)
                .with_target(true)
                .with_thread_ids(false)
                .with_file(true)
                .with_line_number(true)
        )
        .with(filter)
        .init();

    eprintln!("Logging to: {}", log_path.display());

    Some(guard)
}

fn restore_terminal(terminal: &mut Terminal<CrosstermBackend<io::Stdout>>) -> io::Result<()> {
    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;
    Ok(())
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();

    // Handle --version
    if args.version {
        println!("irc {}", update::VERSION);
        return Ok(());
    }

    // Handle --check-update
    if args.check_update {
        match update::check_update().await {
            Ok(Some(version)) => {
                println!("New version available: {} -> {}", update::VERSION, version);
                println!("Run 'irc --update' to install.");
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

    // Setup logging if requested
    let _log_guard = setup_logging(args.debug, args.verbose);

    // Handle --gen-config
    if args.gen_config {
        let config_path = config::config_path();
        let config_dir = config::config_dir();

        // Create config directory if needed
        if !config_dir.exists() {
            std::fs::create_dir_all(&config_dir)?;
        }

        if config_path.exists() {
            eprintln!("Config file already exists: {}", config_path.display());
            eprintln!("Remove it first if you want to regenerate.");
            std::process::exit(1);
        }

        std::fs::write(&config_path, config::example_config())?;
        println!("Generated config file: {}", config_path.display());
        return Ok(());
    }

    // Load config file
    let app_config = if let Some(ref path) = args.config {
        AppConfig::load_from(path).map_err(|e| {
            eprintln!("Error loading config: {}", e);
            e
        })?
    } else {
        AppConfig::load().unwrap_or_else(|e| {
            eprintln!("Warning: Could not load config: {}", e);
            AppConfig::default()
        })
    };

    // Build client config: start from profile or defaults
    let mut config = if let Some(ref profile_name) = args.profile {
        match app_config.build_client_config(profile_name) {
            Some(c) => c,
            None => {
                eprintln!("Unknown server profile: {}", profile_name);
                eprintln!("Available profiles:");
                for name in app_config.server_names() {
                    eprintln!("  - {}", name);
                }
                std::process::exit(1);
            }
        }
    } else {
        ClientConfig::default()
    };

    // Track if server was specified on CLI
    let server_specified = args.server.is_some();

    // Override with CLI args
    if let Some(server) = args.server {
        config.server = server;
    }

    if let Some(port) = args.port {
        config.port = port;
    }

    if args.no_tls {
        config.tls = false;
    }

    if args.tls_accept_invalid {
        config.tls_accept_invalid = true;
    }

    if let Some(nick) = args.nick {
        config.nicknames = vec![nick.clone()];
        config.username = nick.clone();
        config.realname = nick;
    }

    if let Some(user) = args.user {
        config.username = user;
    }

    if let Some(realname) = args.realname {
        config.realname = realname;
    }

    if let (Some(user), Some(pass)) = (args.sasl_user, args.sasl_pass) {
        config = config.sasl(user, pass);
    }

    for channel in args.channels {
        config = config.autojoin(channel);
    }

    // Validate we have a server to connect to
    if config.server == "localhost" && args.profile.is_none() && !server_specified {
        eprintln!("No server specified. Use --server or --profile.");
        eprintln!();
        eprintln!("Examples:");
        eprintln!("  irc --server irc.libera.chat");
        eprintln!("  irc --profile libera  (after configuring ~/.config/irc/config.toml)");
        eprintln!();
        eprintln!("Generate a config file with: irc --gen-config");
        std::process::exit(1);
    }

    // Setup terminal
    let mut terminal = setup_terminal()?;

    // Create and run app
    let mut app = App::new(config);
    let result = app.run(&mut terminal).await;

    // Restore terminal
    restore_terminal(&mut terminal)?;

    result?;

    Ok(())
}
