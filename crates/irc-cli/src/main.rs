//! IRC terminal client.

use clap::Parser;

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

    /// Use TLS
    #[arg(long, default_value = "true")]
    tls: bool,

    /// Nickname
    #[arg(short, long)]
    nick: Option<String>,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();

    tracing_subscriber::fmt::init();

    println!("IRC CLI - Coming soon!");
    println!("Server: {:?}", args.server);

    Ok(())
}
