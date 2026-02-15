//! Client state management.

use std::collections::HashSet;
use std::net::SocketAddr;
use std::sync::RwLock;
use std::time::Instant;

use chrono::{DateTime, Utc};
use irc_proto::{Command, Message, Prefix, Tags};
use tokio::sync::mpsc;
use unicase::UniCase;

use crate::cap::ClientCapState;
use crate::config::LimitsConfig;
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
    /// Bot mode (+B) - marks the user as a bot.
    pub bot: bool,
}

impl std::fmt::Display for UserModes {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut modes = String::from("+");
        if self.bot {
            modes.push('B');
        }
        if self.invisible {
            modes.push('i');
        }
        if self.operator {
            modes.push('o');
        }
        if self.registered {
            modes.push('r');
        }
        if self.wallops {
            modes.push('w');
        }
        if modes.len() == 1 {
            write!(f, "")
        } else {
            write!(f, "{}", modes)
        }
    }
}

/// Rate limiting state using token bucket algorithm.
struct RateLimitState {
    /// Available tokens.
    tokens: f64,
    /// Last time tokens were updated.
    last_update: Instant,
}

impl Default for RateLimitState {
    fn default() -> Self {
        Self {
            tokens: 20.0, // Start with burst capacity
            last_update: Instant::now(),
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

    /// UID for S2S protocol (SID + 6 chars).
    uid: RwLock<Option<String>>,

    /// Nick timestamp for collision resolution (Unix epoch).
    nick_ts: RwLock<i64>,

    /// Whether this is a local client (vs. remote via S2S).
    is_local: bool,

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

    /// Rate limiting state.
    rate_limit: RwLock<RateLimitState>,

    /// Nicknames this client is monitoring (MONITOR command).
    monitor_list: RwLock<HashSet<UniCase<String>>>,
}

impl Client {
    /// Create a new local client.
    pub fn new(id: ClientId, addr: SocketAddr, sender: mpsc::Sender<Message>, tls: bool) -> Self {
        let hostname = addr.ip().to_string();
        let now = Utc::now();

        Self {
            id,
            addr,
            sender,
            registration: RwLock::new(RegistrationPhase::Unregistered),
            nickname: RwLock::new(None),
            uid: RwLock::new(None),
            nick_ts: RwLock::new(now.timestamp()),
            is_local: true,
            username: RwLock::new(None),
            realname: RwLock::new(None),
            hostname: RwLock::new(hostname),
            modes: RwLock::new(UserModes::default()),
            channels: RwLock::new(HashSet::new()),
            away: RwLock::new(None),
            connected_at: now,
            tls,
            password: RwLock::new(None),
            cap_state: RwLock::new(ClientCapState::new()),
            rate_limit: RwLock::new(RateLimitState::default()),
            monitor_list: RwLock::new(HashSet::new()),
        }
    }

    /// Create a new remote client (from S2S link).
    #[allow(clippy::too_many_arguments)]
    pub fn new_remote(
        id: ClientId,
        uid: String,
        nickname: String,
        nick_ts: i64,
        username: String,
        hostname: String,
        realname: String,
        sender: mpsc::Sender<Message>,
    ) -> Self {
        use chrono::TimeZone;

        Self {
            id,
            addr: "0.0.0.0:0".parse().unwrap(),
            sender,
            registration: RwLock::new(RegistrationPhase::Registered),
            nickname: RwLock::new(Some(nickname)),
            uid: RwLock::new(Some(uid)),
            nick_ts: RwLock::new(nick_ts),
            is_local: false,
            username: RwLock::new(Some(username)),
            realname: RwLock::new(Some(realname)),
            hostname: RwLock::new(hostname),
            modes: RwLock::new(UserModes::default()),
            channels: RwLock::new(HashSet::new()),
            away: RwLock::new(None),
            connected_at: Utc
                .timestamp_opt(nick_ts, 0)
                .single()
                .unwrap_or_else(Utc::now),
            tls: false,
            password: RwLock::new(None),
            cap_state: RwLock::new(ClientCapState::new()),
            rate_limit: RwLock::new(RateLimitState::default()),
            monitor_list: RwLock::new(HashSet::new()),
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
        // Update nick timestamp
        *self.nick_ts.write_lock("nick_ts")? = Utc::now().timestamp();
        Ok(())
    }

    /// Get the client's UID.
    pub fn uid(&self) -> Result<Option<String>> {
        Ok(self.uid.read_lock("uid")?.clone())
    }

    /// Set the client's UID.
    pub fn set_uid(&self, uid: String) -> Result<()> {
        *self.uid.write_lock("uid")? = Some(uid);
        Ok(())
    }

    /// Get the nick timestamp.
    pub fn nick_ts(&self) -> Result<i64> {
        Ok(*self.nick_ts.read_lock("nick_ts")?)
    }

    /// Set the nick timestamp.
    pub fn set_nick_ts(&self, ts: i64) -> Result<()> {
        *self.nick_ts.write_lock("nick_ts")? = ts;
        Ok(())
    }

    /// Check if this is a local client.
    pub fn is_local(&self) -> bool {
        self.is_local
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

    /// Clear the account name (for logout).
    pub fn clear_account(&self) -> Result<()> {
        self.cap_state.write_lock("cap_state")?.account = None;
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

    /// Set the CAP LS version.
    pub fn set_cap_version(&self, version: u32) -> Result<()> {
        self.cap_state.write_lock("cap_state")?.cap_version = Some(version);
        Ok(())
    }

    /// Get the CAP LS version.
    pub fn cap_version(&self) -> Result<Option<u32>> {
        Ok(self.cap_state.read_lock("cap_state")?.cap_version)
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

    // ========================================
    // Rate Limiting
    // ========================================

    /// Check if the client can send a command (token bucket algorithm).
    /// Returns Ok(()) if allowed, Err(RateLimited) if not.
    pub fn check_rate_limit(&self, config: &LimitsConfig) -> Result<()> {
        let mut state = self.rate_limit.write_lock("rate_limit")?;
        let now = Instant::now();
        let elapsed = now.duration_since(state.last_update).as_secs_f64();

        // Refill tokens based on time elapsed
        state.tokens = (state.tokens + elapsed * config.command_rate_limit as f64)
            .min(config.command_burst as f64);
        state.last_update = now;

        // Check if we have a token
        if state.tokens >= 1.0 {
            state.tokens -= 1.0;
            Ok(())
        } else {
            Err(Error::RateLimited)
        }
    }

    // ========================================
    // MONITOR Support
    // ========================================

    /// Add nicknames to the monitor list.
    /// Returns the list of nicknames that were actually added.
    pub fn monitor_add(&self, nicks: &[&str]) -> Result<Vec<String>> {
        let mut list = self.monitor_list.write_lock("monitor_list")?;
        let mut added = Vec::new();

        for nick in nicks {
            if nick.is_empty() {
                continue;
            }
            let key = UniCase::new(nick.to_string());
            if list.insert(key) {
                added.push(nick.to_string());
            }
        }

        Ok(added)
    }

    /// Remove nicknames from the monitor list.
    pub fn monitor_remove(&self, nicks: &[&str]) -> Result<()> {
        let mut list = self.monitor_list.write_lock("monitor_list")?;

        for nick in nicks {
            let key = UniCase::new(nick.to_string());
            list.remove(&key);
        }

        Ok(())
    }

    /// Clear the monitor list.
    pub fn monitor_clear(&self) -> Result<()> {
        let mut list = self.monitor_list.write_lock("monitor_list")?;
        list.clear();
        Ok(())
    }

    /// Get the monitor list.
    pub fn monitor_list(&self) -> Result<Vec<String>> {
        let list = self.monitor_list.read_lock("monitor_list")?;
        Ok(list.iter().map(|k| k.to_string()).collect())
    }

    /// Check if this client is monitoring a nickname.
    pub fn is_monitoring(&self, nick: &str) -> Result<bool> {
        let list = self.monitor_list.read_lock("monitor_list")?;
        let key = UniCase::new(nick.to_string());
        Ok(list.contains(&key))
    }

    /// Get the number of monitored nicknames.
    pub fn monitor_count(&self) -> Result<usize> {
        Ok(self.monitor_list.read_lock("monitor_list")?.len())
    }
}
