//! Client configuration.

use serde::Deserialize;

/// Client configuration.
#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct ClientConfig {
    // === Connection ===
    /// Server hostname.
    pub server: String,

    /// Server port (default: 6697 for TLS, 6667 for plain).
    pub port: u16,

    /// Use TLS encryption.
    pub tls: bool,

    /// Accept invalid TLS certificates (for testing only).
    #[serde(default)]
    pub tls_accept_invalid: bool,

    // === Identity ===
    /// Nicknames to try (in order).
    pub nicknames: Vec<String>,

    /// Username (ident).
    pub username: String,

    /// Real name (GECOS).
    pub realname: String,

    /// Server password (PASS command).
    pub server_password: Option<String>,

    // === SASL Authentication ===
    /// SASL configuration.
    pub sasl: Option<SaslConfig>,

    // === IRCv3 Capabilities ===
    /// Capabilities to request from the server.
    /// If empty, uses a sensible default set.
    pub capabilities: Vec<String>,

    // === Auto-features ===
    /// Automatically request chat history when joining channels.
    pub request_chathistory: bool,

    /// Number of messages to request for chat history.
    pub chathistory_limit: usize,

    /// Channels to auto-join on connect.
    pub autojoin: Vec<String>,

    // === Reconnection ===
    /// Automatically reconnect on disconnect.
    pub reconnect: bool,

    /// Initial delay before reconnecting (seconds).
    pub reconnect_delay: u64,

    /// Maximum delay between reconnect attempts (seconds).
    pub reconnect_max_delay: u64,

    /// Maximum number of reconnect attempts (0 = unlimited).
    pub reconnect_max_attempts: u32,
}

impl Default for ClientConfig {
    fn default() -> Self {
        Self {
            server: "localhost".into(),
            port: 6697,
            tls: true,
            tls_accept_invalid: false,
            nicknames: vec!["user".into()],
            username: "user".into(),
            realname: "IRC User".into(),
            server_password: None,
            sasl: None,
            capabilities: default_capabilities(),
            request_chathistory: true,
            chathistory_limit: 100,
            autojoin: Vec::new(),
            reconnect: true,
            reconnect_delay: 5,
            reconnect_max_delay: 300,
            reconnect_max_attempts: 0,
        }
    }
}

impl ClientConfig {
    /// Create a new config with the given nickname.
    pub fn new(nick: impl Into<String>) -> Self {
        let nick = nick.into();
        Self {
            nicknames: vec![nick.clone()],
            username: nick.clone(),
            realname: nick,
            ..Default::default()
        }
    }

    /// Set the server and port.
    pub fn server(mut self, host: impl Into<String>, port: u16) -> Self {
        self.server = host.into();
        self.port = port;
        self
    }

    /// Enable or disable TLS.
    pub fn tls(mut self, enabled: bool) -> Self {
        self.tls = enabled;
        self
    }

    /// Set SASL credentials.
    pub fn sasl(mut self, username: impl Into<String>, password: impl Into<String>) -> Self {
        self.sasl = Some(SaslConfig {
            username: username.into(),
            password: password.into(),
            mechanism: SaslMechanism::Plain,
        });
        self
    }

    /// Add a channel to auto-join.
    pub fn autojoin(mut self, channel: impl Into<String>) -> Self {
        self.autojoin.push(channel.into());
        self
    }

    /// Get the effective port (accounting for TLS default).
    pub fn effective_port(&self) -> u16 {
        if self.port == 6697 && !self.tls {
            6667
        } else if self.port == 6667 && self.tls {
            6697
        } else {
            self.port
        }
    }
}

/// SASL authentication configuration.
#[derive(Debug, Clone, Deserialize)]
pub struct SaslConfig {
    /// Username (authcid).
    pub username: String,

    /// Password.
    pub password: String,

    /// SASL mechanism to use.
    #[serde(default)]
    pub mechanism: SaslMechanism,
}

/// SASL authentication mechanism.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Deserialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum SaslMechanism {
    /// PLAIN mechanism (base64 encoded credentials).
    #[default]
    Plain,
}

impl SaslMechanism {
    /// Get the mechanism name as a string.
    pub fn as_str(&self) -> &'static str {
        match self {
            SaslMechanism::Plain => "PLAIN",
        }
    }
}

/// Default capabilities to request.
fn default_capabilities() -> Vec<String> {
    vec![
        "server-time".into(),
        "message-tags".into(),
        "batch".into(),
        "echo-message".into(),
        "account-notify".into(),
        "account-tag".into(),
        "away-notify".into(),
        "extended-join".into(),
        "multi-prefix".into(),
        "draft/chathistory".into(),
        "cap-notify".into(),
    ]
}
