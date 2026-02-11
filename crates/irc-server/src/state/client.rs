//! Client state management.

use std::collections::HashSet;
use std::net::SocketAddr;
use std::sync::RwLock;

use chrono::{DateTime, Utc};
use irc_proto::{Command, Message, Prefix, Tags};
use tokio::sync::mpsc;
use unicase::UniCase;

use crate::cap::ClientCapState;
use crate::error::{Error, Result};
use crate::lock::RwLockExt;

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

    /// Channel for sending messages to this client (bounded to prevent backpressure).
    pub sender: mpsc::Sender<Message>,

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

    /// IRCv3 capability state.
    cap_state: RwLock<ClientCapState>,
}

impl Client {
    /// Create a new client.
    pub fn new(
        id: ClientId,
        addr: SocketAddr,
        sender: mpsc::Sender<Message>,
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
            cap_state: RwLock::new(ClientCapState::new()),
        }
    }

    /// Check if the client is fully registered.
    pub fn is_registered(&self) -> Result<bool> {
        Ok(self.registration.read_lock("registration")?.is_registered())
    }

    /// Get the current registration phase.
    pub fn registration_phase(&self) -> Result<RegistrationPhase> {
        Ok(*self.registration.read_lock("registration")?)
    }

    /// Get the client's nickname.
    pub fn nickname(&self) -> Result<Option<String>> {
        Ok(self.nickname.read_lock("nickname")?.clone())
    }

    /// Set the client's nickname.
    pub fn set_nickname(&self, nick: String) -> Result<()> {
        *self.nickname.write_lock("nickname")? = Some(nick);
        Ok(())
    }

    /// Mark that NICK was received.
    pub fn got_nick(&self) -> Result<()> {
        self.registration.write_lock("registration")?.got_nick();
        Ok(())
    }

    /// Get the client's username.
    pub fn username(&self) -> Result<Option<String>> {
        Ok(self.username.read_lock("username")?.clone())
    }

    /// Set the client's username and realname.
    pub fn set_user(&self, username: String, realname: String) -> Result<()> {
        *self.username.write_lock("username")? = Some(username);
        *self.realname.write_lock("realname")? = Some(realname);
        Ok(())
    }

    /// Mark that USER was received.
    pub fn got_user(&self) -> Result<()> {
        self.registration.write_lock("registration")?.got_user();
        Ok(())
    }

    /// Get the client's realname.
    pub fn realname(&self) -> Result<Option<String>> {
        Ok(self.realname.read_lock("realname")?.clone())
    }

    /// Get the client's hostname.
    pub fn hostname(&self) -> Result<String> {
        Ok(self.hostname.read_lock("hostname")?.clone())
    }

    /// Set the client's hostname.
    pub fn set_hostname(&self, hostname: String) -> Result<()> {
        *self.hostname.write_lock("hostname")? = hostname;
        Ok(())
    }

    /// Get the client's away message.
    pub fn away_message(&self) -> Result<Option<String>> {
        Ok(self.away.read_lock("away")?.clone())
    }

    /// Set the client's away message.
    pub fn set_away(&self, message: Option<String>) -> Result<()> {
        *self.away.write_lock("away")? = message;
        Ok(())
    }

    /// Check if the client is away.
    pub fn is_away(&self) -> Result<bool> {
        Ok(self.away.read_lock("away")?.is_some())
    }

    /// Set the connection password.
    pub fn set_password(&self, password: String) -> Result<()> {
        *self.password.write_lock("password")? = Some(password);
        Ok(())
    }

    /// Get the connection password.
    pub fn password(&self) -> Result<Option<String>> {
        Ok(self.password.read_lock("password")?.clone())
    }

    /// Get the client's prefix for outgoing messages.
    pub fn prefix(&self) -> Result<Prefix> {
        let nick = self.nickname()?.unwrap_or_else(|| "*".to_string());
        let user = self.username()?.unwrap_or_else(|| "unknown".to_string());
        let host = self.hostname()?;
        Ok(Prefix::from_user(nick, user, host))
    }

    /// Send a message to this client.
    ///
    /// Returns `Ok(true)` if successful, `Ok(false)` if the channel is closed,
    /// or `Err(SendBufferFull)` if the send buffer is full (client too slow).
    pub fn send(&self, message: Message) -> Result<bool> {
        match self.sender.try_send(message) {
            Ok(()) => Ok(true),
            Err(mpsc::error::TrySendError::Full(_)) => Err(Error::SendBufferFull),
            Err(mpsc::error::TrySendError::Closed(_)) => Ok(false),
        }
    }

    /// Send a numeric reply to this client.
    pub fn send_numeric(&self, server_name: &str, code: u16, params: Vec<String>) -> Result<bool> {
        let target = self.nickname()?.unwrap_or_else(|| "*".to_string());
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

    /// Add a channel to the client's channel list.
    pub fn join_channel(&self, channel_name: &str) -> Result<()> {
        let key = UniCase::new(channel_name.to_string());
        self.channels.write_lock("channels")?.insert(key);
        Ok(())
    }

    /// Remove a channel from the client's channel list.
    pub fn leave_channel(&self, channel_name: &str) -> Result<()> {
        let key = UniCase::new(channel_name.to_string());
        self.channels.write_lock("channels")?.remove(&key);
        Ok(())
    }

    /// Get the number of channels the client is in.
    pub fn channel_count(&self) -> Result<usize> {
        Ok(self.channels.read_lock("channels")?.len())
    }

    /// Check if the client is in a channel.
    pub fn is_in_channel(&self, channel_name: &str) -> Result<bool> {
        let key = UniCase::new(channel_name.to_string());
        Ok(self.channels.read_lock("channels")?.contains(&key))
    }

    /// Get the client's hostmask (nick!user@host).
    pub fn hostmask(&self) -> Result<String> {
        let nick = self.nickname()?.unwrap_or_else(|| "*".to_string());
        let user = self.username()?.unwrap_or_else(|| "unknown".to_string());
        let host = self.hostname()?;
        Ok(format!("{}!{}@{}", nick, user, host))
    }

    /// Get a copy of the channel names the client is in.
    pub fn channel_names(&self) -> Result<Vec<String>> {
        Ok(self
            .channels
            .read_lock("channels")?
            .iter()
            .map(|c| c.to_string())
            .collect())
    }

    // ========================================
    // IRCv3 Capability Methods
    // ========================================

    /// Check if a capability is enabled for this client.
    pub fn has_cap(&self, name: &str) -> Result<bool> {
        Ok(self.cap_state.read_lock("cap_state")?.has_cap(name))
    }

    /// Enable a capability for this client.
    pub fn enable_cap(&self, name: &str) -> Result<()> {
        self.cap_state.write_lock("cap_state")?.enable(name);
        Ok(())
    }

    /// Disable a capability for this client.
    pub fn disable_cap(&self, name: &str) -> Result<()> {
        self.cap_state.write_lock("cap_state")?.disable(name);
        Ok(())
    }

    /// Get the list of enabled capabilities.
    pub fn enabled_caps(&self) -> Result<Vec<String>> {
        Ok(self
            .cap_state
            .read_lock("cap_state")?
            .enabled
            .iter()
            .cloned()
            .collect())
    }

    /// Start capability negotiation.
    pub fn start_cap_negotiation(&self) -> Result<()> {
        self.cap_state.write_lock("cap_state")?.start_negotiation();
        Ok(())
    }

    /// End capability negotiation.
    pub fn end_cap_negotiation(&self) -> Result<()> {
        self.cap_state.write_lock("cap_state")?.end_negotiation();
        Ok(())
    }

    /// Check if capability negotiation is in progress.
    pub fn is_cap_negotiating(&self) -> Result<bool> {
        Ok(self.cap_state.read_lock("cap_state")?.is_negotiating())
    }

    /// Get the formatted list of enabled capabilities.
    pub fn format_cap_list(&self) -> Result<String> {
        Ok(self.cap_state.read_lock("cap_state")?.format_list())
    }

    /// Get the account name (if authenticated via SASL).
    pub fn account(&self) -> Result<Option<String>> {
        Ok(self.cap_state.read_lock("cap_state")?.account.clone())
    }

    /// Set the account name after successful SASL authentication.
    pub fn set_account(&self, account: String) -> Result<()> {
        self.cap_state.write_lock("cap_state")?.account = Some(account);
        Ok(())
    }

    /// Get the current SASL state.
    pub fn sasl_state(&self) -> Result<Option<crate::cap::sasl::SaslState>> {
        Ok(self.cap_state.read_lock("cap_state")?.sasl_state.clone())
    }

    /// Set the SASL state.
    pub fn set_sasl_state(&self, state: Option<crate::cap::sasl::SaslState>) -> Result<()> {
        self.cap_state.write_lock("cap_state")?.sasl_state = state;
        Ok(())
    }

    /// Send a message to this client, adding server-time tag if capability is enabled.
    pub fn send_with_tags(&self, mut message: Message) -> Result<bool> {
        // Add server-time tag if the client has it enabled
        if self.has_cap("server-time")? {
            let time = Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true);
            let tags = message.tags.get_or_insert_with(Tags::new);
            tags.set("time", time);
        }
        self.send(message)
    }
}
