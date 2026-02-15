//! Configuration file support.
//!
//! Loads settings from ~/.config/irc/config.toml

use std::collections::HashMap;
use std::path::PathBuf;

use serde::Deserialize;

use irc_client_lib::ClientConfig;

/// Application configuration loaded from file.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct AppConfig {
    /// Default settings applied to all servers.
    #[serde(default)]
    pub defaults: DefaultSettings,

    /// Server profiles.
    #[serde(default)]
    pub servers: HashMap<String, ServerProfile>,

    /// UI settings.
    #[serde(default)]
    #[allow(dead_code)]
    pub ui: UiConfig,
}

/// UI configuration settings.
#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct UiConfig {
    /// Enable vim-style keybindings.
    pub vim_mode: bool,

    /// Message grouping window in seconds (default 300 = 5 min).
    pub message_group_window: u64,

    /// Join/part collapse window in seconds (default 30).
    pub joinpart_collapse_window: u64,

    /// Hide join/part/quit messages entirely.
    pub hide_joinpart: bool,
}

impl Default for UiConfig {
    fn default() -> Self {
        Self {
            vim_mode: false,
            message_group_window: 300,
            joinpart_collapse_window: 30,
            hide_joinpart: false,
        }
    }
}

/// Default settings for all connections.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct DefaultSettings {
    /// Default nickname.
    pub nick: Option<String>,

    /// Alternative nicknames.
    pub alt_nicks: Vec<String>,

    /// Default username.
    pub username: Option<String>,

    /// Default real name.
    pub realname: Option<String>,

    /// Default quit message.
    pub quit_message: Option<String>,

    /// Use TLS by default.
    pub tls: Option<bool>,

    /// Auto-reconnect by default.
    pub reconnect: Option<bool>,

    /// Reconnect delay (seconds).
    pub reconnect_delay: Option<u64>,

    /// Request chat history by default.
    pub chathistory: Option<bool>,
}

/// Server profile configuration.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct ServerProfile {
    /// Server hostname.
    pub host: String,

    /// Server port (default: 6697 for TLS, 6667 for plain).
    pub port: Option<u16>,

    /// Use TLS.
    pub tls: Option<bool>,

    /// Accept invalid TLS certificates.
    pub tls_accept_invalid: Option<bool>,

    /// Nickname for this server.
    pub nick: Option<String>,

    /// Alternative nicknames.
    pub alt_nicks: Vec<String>,

    /// Username.
    pub username: Option<String>,

    /// Real name.
    pub realname: Option<String>,

    /// Server password.
    pub password: Option<String>,

    /// SASL username.
    pub sasl_user: Option<String>,

    /// SASL password.
    pub sasl_pass: Option<String>,

    /// Channels to auto-join.
    pub channels: Vec<String>,

    /// Auto-reconnect.
    pub reconnect: Option<bool>,

    /// Reconnect delay (seconds).
    pub reconnect_delay: Option<u64>,

    /// Request chat history.
    pub chathistory: Option<bool>,
}

impl AppConfig {
    /// Load configuration from the default path.
    pub fn load() -> Result<Self, ConfigError> {
        let path = config_path();

        if !path.exists() {
            return Ok(Self::default());
        }

        let content =
            std::fs::read_to_string(&path).map_err(|e| ConfigError::Read(path.clone(), e))?;

        toml::from_str(&content).map_err(|e| ConfigError::Parse(path, e))
    }

    /// Load configuration from a specific path.
    pub fn load_from(path: &std::path::Path) -> Result<Self, ConfigError> {
        let content =
            std::fs::read_to_string(path).map_err(|e| ConfigError::Read(path.to_path_buf(), e))?;

        toml::from_str(&content).map_err(|e| ConfigError::Parse(path.to_path_buf(), e))
    }

    /// Get a server profile by name.
    #[allow(dead_code)]
    pub fn server(&self, name: &str) -> Option<&ServerProfile> {
        self.servers.get(name)
    }

    /// List all server profile names.
    pub fn server_names(&self) -> impl Iterator<Item = &str> {
        self.servers.keys().map(String::as_str)
    }

    /// Build a ClientConfig from a server profile.
    pub fn build_client_config(&self, profile_name: &str) -> Option<ClientConfig> {
        let profile = self.servers.get(profile_name)?;
        Some(self.build_config_from_profile(profile))
    }

    /// Build a ClientConfig from a profile with defaults applied.
    fn build_config_from_profile(&self, profile: &ServerProfile) -> ClientConfig {
        let defaults = &self.defaults;

        // Build nicknames list
        let nick = profile
            .nick
            .clone()
            .or_else(|| defaults.nick.clone())
            .unwrap_or_else(whoami::username);

        let mut nicknames = vec![nick.clone()];
        if !profile.alt_nicks.is_empty() {
            nicknames.extend(profile.alt_nicks.clone());
        } else if !defaults.alt_nicks.is_empty() {
            nicknames.extend(defaults.alt_nicks.clone());
        }

        // Determine TLS and port
        let tls = profile.tls.or(defaults.tls).unwrap_or(true);

        let port = profile.port.unwrap_or(if tls { 6697 } else { 6667 });

        let mut config = ClientConfig {
            server: profile.host.clone(),
            port,
            tls,
            tls_accept_invalid: profile.tls_accept_invalid.unwrap_or(false),
            nicknames,
            username: profile
                .username
                .clone()
                .or_else(|| defaults.username.clone())
                .unwrap_or_else(|| nick.clone()),
            realname: profile
                .realname
                .clone()
                .or_else(|| defaults.realname.clone())
                .unwrap_or_else(|| nick.clone()),
            server_password: profile.password.clone(),
            sasl: None,
            autojoin: profile.channels.clone(),
            reconnect: profile.reconnect.or(defaults.reconnect).unwrap_or(true),
            reconnect_delay: profile
                .reconnect_delay
                .or(defaults.reconnect_delay)
                .unwrap_or(5),
            request_chathistory: profile.chathistory.or(defaults.chathistory).unwrap_or(true),
            ..ClientConfig::default()
        };

        // Set SASL if configured
        if let (Some(user), Some(pass)) = (&profile.sasl_user, &profile.sasl_pass) {
            config = config.sasl(user.clone(), pass.clone());
        }

        config
    }
}

/// Get the default config file path.
pub fn config_path() -> PathBuf {
    dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("irc")
        .join("config.toml")
}

/// Get the config directory path.
pub fn config_dir() -> PathBuf {
    dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("irc")
}

/// Configuration errors.
#[derive(Debug)]
pub enum ConfigError {
    /// Error reading config file.
    Read(PathBuf, std::io::Error),
    /// Error parsing config file.
    Parse(PathBuf, toml::de::Error),
}

impl std::fmt::Display for ConfigError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ConfigError::Read(path, e) => {
                write!(f, "Failed to read config file {:?}: {}", path, e)
            }
            ConfigError::Parse(path, e) => {
                write!(f, "Failed to parse config file {:?}: {}", path, e)
            }
        }
    }
}

impl std::error::Error for ConfigError {}

/// Generate an example configuration file.
pub fn example_config() -> &'static str {
    r##"# IRC Client Configuration
#
# Place this file at ~/.config/irc/config.toml

[defaults]
nick = "mynick"
alt_nicks = ["mynick_", "mynick__"]
username = "myuser"
realname = "My Real Name"
tls = true
reconnect = true
reconnect_delay = 5
chathistory = true

# Server profiles - use with: irc --profile libera
[servers.libera]
host = "irc.libera.chat"
port = 6697
tls = true
channels = ["#channel1", "#channel2"]
# sasl_user = "myaccount"
# sasl_pass = "mypassword"

[servers.local]
host = "localhost"
port = 6667
tls = false
channels = ["#test"]
"##
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_config() {
        let config_str = r##"
[defaults]
nick = "testnick"
tls = true

[servers.test]
host = "irc.example.com"
port = 6697
channels = ["#test"]
"##;

        let config: AppConfig = toml::from_str(config_str).unwrap();
        assert_eq!(config.defaults.nick, Some("testnick".to_string()));
        assert!(config.servers.contains_key("test"));

        let client_config = config.build_client_config("test").unwrap();
        assert_eq!(client_config.server, "irc.example.com");
        assert_eq!(client_config.nicknames[0], "testnick");
    }
}
