//! Client state management.

use std::collections::HashSet;
use std::net::SocketAddr;
use std::sync::RwLock;

use chrono::{DateTime, Utc};
use irc_proto::{Command, Message, Prefix};
use tokio::sync::mpsc;
use unicase::UniCase;

/// Unique identifier for a connected client.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ClientId(pub u64);

impl std::fmt::Display for ClientId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Client#{}", self.0)
    }
}

/// Client registration state machine.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RegistrationPhase {
    /// Initial state - nothing received yet.
    Unregistered,
    /// Received NICK command.
    GotNick,
    /// Received USER command.
    GotUser,
    /// Received both NICK and USER - fully registered.
    Registered,
}

impl RegistrationPhase {
    /// Check if the client is fully registered.
    pub fn is_registered(&self) -> bool {
        matches!(self, RegistrationPhase::Registered)
    }

    /// Advance state after receiving NICK.
    pub fn got_nick(&mut self) {
        *self = match *self {
            RegistrationPhase::Unregistered => RegistrationPhase::GotNick,
            RegistrationPhase::GotUser => RegistrationPhase::Registered,
            other => other,
        };
    }

    /// Advance state after receiving USER.
    pub fn got_user(&mut self) {
        *self = match *self {
            RegistrationPhase::Unregistered => RegistrationPhase::GotUser,
            RegistrationPhase::GotNick => RegistrationPhase::Registered,
            other => other,
        };
    }
}

/// User modes.
#[derive(Debug, Clone, Default)]
pub struct UserModes {
    /// Invisible mode (+i).
    pub invisible: bool,
    /// Operator mode (+o).
    pub operator: bool,
    /// Wallops mode (+w) - receive wallops messages.
    pub wallops: bool,
    /// Registered with services (+r).
    pub registered: bool,
}

impl UserModes {
    /// Get the mode string (e.g., "+iw").
    pub fn to_string(&self) -> String {
        let mut modes = String::from("+");
        if self.invisible {
            modes.push('i');
        }
        if self.operator {
            modes.push('o');
        }
        if self.wallops {
            modes.push('w');
        }
        if self.registered {
            modes.push('r');
        }
        if modes.len() == 1 {
            String::new()
        } else {
            modes
        }
    }
}

/// A connected client.
pub struct Client {
    /// Unique client ID.
    pub id: ClientId,

    /// Client's remote address.
    pub addr: SocketAddr,

    /// Channel for sending messages to this client.
    pub sender: mpsc::UnboundedSender<Message>,

    /// Registration phase.
    registration: RwLock<RegistrationPhase>,

    /// Client's nickname (once set).
    nickname: RwLock<Option<String>>,

    /// Client's username (from USER command).
    username: RwLock<Option<String>>,

    /// Client's realname (from USER command).
    realname: RwLock<Option<String>>,

    /// Client's hostname (resolved or from address).
    hostname: RwLock<String>,

    /// User modes.
    pub modes: RwLock<UserModes>,

    /// Channels the client is in (Phase 2).
    pub channels: RwLock<HashSet<UniCase<String>>>,

    /// Away message (if set).
    away: RwLock<Option<String>>,

    /// Connection timestamp.
    pub connected_at: DateTime<Utc>,

    /// Whether the connection uses TLS.
    pub tls: bool,

    /// Password sent with PASS (before registration).
    password: RwLock<Option<String>>,
}

impl Client {
    /// Create a new client.
    pub fn new(
        id: ClientId,
        addr: SocketAddr,
        sender: mpsc::UnboundedSender<Message>,
        tls: bool,
    ) -> Self {
        let hostname = addr.ip().to_string();

        Self {
            id,
            addr,
            sender,
            registration: RwLock::new(RegistrationPhase::Unregistered),
            nickname: RwLock::new(None),
            username: RwLock::new(None),
            realname: RwLock::new(None),
            hostname: RwLock::new(hostname),
            modes: RwLock::new(UserModes::default()),
            channels: RwLock::new(HashSet::new()),
            away: RwLock::new(None),
            connected_at: Utc::now(),
            tls,
            password: RwLock::new(None),
        }
    }

    /// Check if the client is fully registered.
    pub fn is_registered(&self) -> bool {
        self.registration.read().unwrap().is_registered()
    }

    /// Get the current registration phase.
    pub fn registration_phase(&self) -> RegistrationPhase {
        *self.registration.read().unwrap()
    }

    /// Get the client's nickname.
    pub fn nickname(&self) -> Option<String> {
        self.nickname.read().unwrap().clone()
    }

    /// Set the client's nickname.
    pub fn set_nickname(&self, nick: String) {
        *self.nickname.write().unwrap() = Some(nick);
    }

    /// Mark that NICK was received.
    pub fn got_nick(&self) {
        self.registration.write().unwrap().got_nick();
    }

    /// Get the client's username.
    pub fn username(&self) -> Option<String> {
        self.username.read().unwrap().clone()
    }

    /// Set the client's username and realname.
    pub fn set_user(&self, username: String, realname: String) {
        *self.username.write().unwrap() = Some(username);
        *self.realname.write().unwrap() = Some(realname);
    }

    /// Mark that USER was received.
    pub fn got_user(&self) {
        self.registration.write().unwrap().got_user();
    }

    /// Get the client's realname.
    pub fn realname(&self) -> Option<String> {
        self.realname.read().unwrap().clone()
    }

    /// Get the client's hostname.
    pub fn hostname(&self) -> String {
        self.hostname.read().unwrap().clone()
    }

    /// Set the client's hostname.
    pub fn set_hostname(&self, hostname: String) {
        *self.hostname.write().unwrap() = hostname;
    }

    /// Get the client's away message.
    pub fn away_message(&self) -> Option<String> {
        self.away.read().unwrap().clone()
    }

    /// Set the client's away message.
    pub fn set_away(&self, message: Option<String>) {
        *self.away.write().unwrap() = message;
    }

    /// Check if the client is away.
    pub fn is_away(&self) -> bool {
        self.away.read().unwrap().is_some()
    }

    /// Set the connection password.
    pub fn set_password(&self, password: String) {
        *self.password.write().unwrap() = Some(password);
    }

    /// Get the connection password.
    pub fn password(&self) -> Option<String> {
        self.password.read().unwrap().clone()
    }

    /// Get the client's prefix for outgoing messages.
    pub fn prefix(&self) -> Prefix {
        let nick = self.nickname().unwrap_or_else(|| "*".to_string());
        let user = self.username().unwrap_or_else(|| "unknown".to_string());
        let host = self.hostname();
        Prefix::from_user(nick, user, host)
    }

    /// Send a message to this client.
    ///
    /// Returns `true` if successful, `false` if the channel is closed.
    pub fn send(&self, message: Message) -> bool {
        self.sender.send(message).is_ok()
    }

    /// Send a numeric reply to this client.
    pub fn send_numeric(&self, server_name: &str, code: u16, params: Vec<String>) -> bool {
        let target = self.nickname().unwrap_or_else(|| "*".to_string());
        let msg = Message::with_prefix(
            Prefix::from_server(server_name),
            Command::Numeric {
                code,
                target,
                params,
            },
        );
        self.send(msg)
    }
}
