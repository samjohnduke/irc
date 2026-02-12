//! IRC terminal client.

use std::io;

use clap::Parser;
use crossterm::{
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::backend::CrosstermBackend;
use ratatui::Terminal;

use irc_client_lib::ClientConfig;

mod app;
mod handler;
mod state;
mod style;
mod ui;

use app::App;

#[derive(Parser)]
#[command(name = "irc")]
#[command(about = "Terminal IRC client")]
struct Args {
    /// Server to connect to
    #[arg(short, long)]
    server: Option<String>,

    /// Port
    #[arg(short, long, default_value = "6697")]
    port: u16,

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
}

fn setup_terminal() -> io::Result<Terminal<CrosstermBackend<io::Stdout>>> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    Terminal::new(backend)
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

    // Build client config from args
    let mut config = ClientConfig::default();

    if let Some(server) = args.server {
        config.server = server;
    }

    config.port = args.port;
    config.tls = !args.no_tls;
    config.tls_accept_invalid = args.tls_accept_invalid;

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
