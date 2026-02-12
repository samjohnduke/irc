//! Server configuration.

use serde::Deserialize;
use std::net::SocketAddr;
use std::path::PathBuf;

/// Server configuration.
#[derive(Debug, Clone, Deserialize)]
pub struct ServerConfig {
    /// Server name (e.g., "irc.example.com")
    pub server_name: String,

    /// Network name (e.g., "ExampleNet")
    pub network_name: String,

    /// Listen addresses
    #[serde(default)]
    pub listen: Vec<ListenConfig>,

    /// Path to MOTD file
    #[serde(default)]
    pub motd_file: Option<PathBuf>,

    /// Server limits
    #[serde(default)]
    pub limits: LimitsConfig,

    /// Operator accounts
    #[serde(default)]
    pub operators: Vec<OperConfig>,

    /// Admin information
    #[serde(default)]
    pub admin: AdminConfig,

    /// SASL accounts for authentication
    #[serde(default)]
    pub accounts: Vec<AccountConfig>,

    /// Services configuration (NickServ, ChanServ)
    #[serde(default)]
    pub services: ServicesConfig,

    /// Server-to-Server (S2S) configuration
    #[serde(default)]
    pub s2s: Option<S2SConfig>,
}

/// Listener configuration.
#[derive(Debug, Clone, Deserialize)]
pub struct ListenConfig {
    /// Address to bind
    pub address: SocketAddr,

    /// TLS configuration (if enabled)
    pub tls: Option<TlsConfig>,
}

/// TLS configuration.
#[derive(Debug, Clone, Deserialize)]
pub struct TlsConfig {
    /// Path to certificate file
    pub cert_file: PathBuf,

    /// Path to private key file
    pub key_file: PathBuf,
}

/// Server limits configuration.
#[derive(Debug, Clone, Deserialize)]
pub struct LimitsConfig {
    /// Maximum number of clients
    #[serde(default = "default_max_clients")]
    pub max_clients: usize,

    /// Ping timeout in seconds
    #[serde(default = "default_ping_timeout")]
    pub ping_timeout: u64,

    /// Registration timeout in seconds
    #[serde(default = "default_registration_timeout")]
    pub registration_timeout: u64,

    /// Maximum nickname length
    #[serde(default = "default_max_nick_length")]
    pub max_nick_length: usize,

    /// Maximum channel name length
    #[serde(default = "default_max_channel_length")]
    pub max_channel_length: usize,

    /// Maximum topic length
    #[serde(default = "default_max_topic_length")]
    pub max_topic_length: usize,

    /// Maximum kick message length
    #[serde(default = "default_max_kick_length")]
    pub max_kick_length: usize,

    /// Maximum away message length
    #[serde(default = "default_max_away_length")]
    pub max_away_length: usize,

    /// Maximum channels a user can join
    #[serde(default = "default_max_channels")]
    pub max_channels: usize,

    /// Size of per-client send buffer (messages)
    #[serde(default = "default_send_buffer_size")]
    pub send_buffer_size: usize,

    /// Maximum connections per IP address
    #[serde(default = "default_max_connections_per_ip")]
    pub max_connections_per_ip: usize,

    /// Maximum commands per second per client
    #[serde(default = "default_command_rate_limit")]
    pub command_rate_limit: usize,

    /// Burst allowance (commands before rate limiting kicks in)
    #[serde(default = "default_command_burst")]
    pub command_burst: usize,

    /// Maximum MONITOR entries per client
    #[serde(default = "default_max_monitor")]
    pub max_monitor: usize,

    /// Maximum CHATHISTORY messages per request
    #[serde(default = "default_max_history")]
    pub max_history: usize,

    /// Message history retention in days (0 = unlimited)
    #[serde(default = "default_history_retention_days")]
    pub history_retention_days: u32,
}

fn default_max_clients() -> usize {
    1024
}
fn default_ping_timeout() -> u64 {
    180
}
fn default_registration_timeout() -> u64 {
    60
}
fn default_max_nick_length() -> usize {
    30
}
fn default_max_channel_length() -> usize {
    50
}
fn default_max_topic_length() -> usize {
    390
}
fn default_max_kick_length() -> usize {
    255
}
fn default_max_away_length() -> usize {
    255
}
fn default_max_channels() -> usize {
    25
}

fn default_send_buffer_size() -> usize {
    512
}

fn default_max_connections_per_ip() -> usize {
    10
}

fn default_command_rate_limit() -> usize {
    10
}

fn default_command_burst() -> usize {
    20
}

fn default_max_monitor() -> usize {
    100
}

fn default_max_history() -> usize {
    100
}

fn default_history_retention_days() -> u32 {
    7
}

impl Default for LimitsConfig {
    fn default() -> Self {
        Self {
            max_clients: default_max_clients(),
            ping_timeout: default_ping_timeout(),
            registration_timeout: default_registration_timeout(),
            max_nick_length: default_max_nick_length(),
            max_channel_length: default_max_channel_length(),
            max_topic_length: default_max_topic_length(),
            max_kick_length: default_max_kick_length(),
            max_away_length: default_max_away_length(),
            max_channels: default_max_channels(),
            send_buffer_size: default_send_buffer_size(),
            max_connections_per_ip: default_max_connections_per_ip(),
            command_rate_limit: default_command_rate_limit(),
            command_burst: default_command_burst(),
            max_monitor: default_max_monitor(),
            max_history: default_max_history(),
            history_retention_days: default_history_retention_days(),
        }
    }
}

/// Operator account configuration.
#[derive(Debug, Clone, Deserialize)]
pub struct OperConfig {
    /// Operator name
    pub name: String,

    /// Password hash (argon2)
    pub password_hash: String,

    /// Host mask (e.g., "*@*.example.com")
    #[serde(default)]
    pub host_mask: Option<String>,
}

/// SASL account configuration.
#[derive(Debug, Clone, Deserialize)]
pub struct AccountConfig {
    /// Account name (username for authentication)
    pub name: String,

    /// Password hash (argon2)
    pub password_hash: String,
}

/// Services configuration.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct ServicesConfig {
    /// Path to the SQLite database file.
    /// If not specified, services (NickServ, ChanServ) will be disabled.
    pub database_path: Option<PathBuf>,
}

/// Admin information configuration.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct AdminConfig {
    /// Location line 1 (e.g., organization name)
    pub location1: Option<String>,

    /// Location line 2 (e.g., city, country)
    pub location2: Option<String>,

    /// Admin email address
    pub email: Option<String>,
}

/// Server-to-Server (S2S) configuration.
#[derive(Debug, Clone, Deserialize)]
pub struct S2SConfig {
    /// This server's unique 3-character ID (e.g., "00A").
    /// Must be alphanumeric and unique across the network.
    pub sid: String,

    /// Address to listen for incoming S2S connections.
    /// If not specified, the server won't accept incoming S2S connections.
    pub listen_address: Option<String>,

    /// Port to listen for incoming S2S connections.
    pub listen_port: Option<u16>,

    /// TLS configuration for S2S connections.
    pub tls: Option<TlsConfig>,

    /// Linked server configurations.
    #[serde(default)]
    pub links: Vec<LinkConfig>,
}

/// Configuration for a linked server.
#[derive(Debug, Clone, Deserialize)]
pub struct LinkConfig {
    /// Name of the remote server (e.g., "irc.server-b.local").
    pub name: String,

    /// Address of the remote server.
    pub address: String,

    /// Port of the remote server's S2S listener.
    pub port: u16,

    /// Password to send when connecting to the remote server.
    pub send_password: String,

    /// Password expected from the remote server when it connects to us.
    pub receive_password: String,

    /// Whether to automatically connect to this server on startup.
    #[serde(default)]
    pub auto_connect: bool,

    /// Whether to accept incoming connections from this server.
    #[serde(default = "default_true")]
    pub accept_incoming: bool,

    /// TLS configuration for connecting to this server (optional).
    pub tls: Option<TlsConfig>,
}

fn default_true() -> bool {
    true
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            server_name: "localhost".into(),
            network_name: "LocalNet".into(),
            listen: vec![ListenConfig {
                address: "127.0.0.1:6667"
                    .parse()
                    .expect("hardcoded address is valid"),
                tls: None,
            }],
            motd_file: None,
            limits: LimitsConfig::default(),
            operators: Vec::new(),
            admin: AdminConfig::default(),
            accounts: Vec::new(),
            services: ServicesConfig::default(),
            s2s: None,
        }
    }
}
