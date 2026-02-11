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
        }
    }
}
