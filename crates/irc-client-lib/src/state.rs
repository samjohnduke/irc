//! Client session state.
//!
//! Tracks the current state of the IRC session including our nickname,
//! joined channels, and their members.

use std::collections::{HashMap, HashSet};

use unicase::UniCase;

use crate::cap::CapabilityState;

/// Current session state.
#[derive(Debug)]
pub struct SessionState {
    /// Our current nickname.
    nick: String,

    /// Server name.
    server_name: Option<String>,

    /// Channels we're in (case-insensitive keys).
    channels: HashMap<UniCase<String>, ChannelState>,

    /// Users we know about (for queries/PMs).
    users: HashMap<UniCase<String>, UserInfo>,

    /// Our user modes.
    user_modes: HashSet<char>,

    /// Capability state.
    caps: CapabilityState,

    /// Whether we're fully registered.
    registered: bool,

    /// Our authenticated account name (if SASL succeeded).
    account: Option<String>,

    /// ISUPPORT parameters from the server.
    isupport: HashMap<String, Option<String>>,
}

/// State of a joined channel.
#[derive(Debug, Clone)]
pub struct ChannelState {
    /// Channel name (original case).
    pub name: String,

    /// Channel topic.
    pub topic: Option<TopicInfo>,

    /// Channel members (nick -> prefixes).
    pub members: HashMap<UniCase<String>, MemberInfo>,

    /// Channel modes (simple modes only, not lists).
    pub modes: HashSet<char>,

    /// Channel key (if set).
    pub key: Option<String>,

    /// Channel limit (if set).
    pub limit: Option<u32>,
}

/// Channel topic information.
#[derive(Debug, Clone)]
pub struct TopicInfo {
    /// The topic text.
    pub text: String,

    /// Who set the topic.
    pub setter: Option<String>,

    /// When the topic was set (Unix timestamp).
    pub set_at: Option<i64>,
}

/// Information about a channel member.
#[derive(Debug, Clone, Default)]
pub struct MemberInfo {
    /// Prefix characters (e.g., "@", "+", "@+").
    pub prefixes: String,

    /// Full user@host if known.
    pub userhost: Option<String>,

    /// Account name if known.
    pub account: Option<String>,

    /// Away message if known and away.
    pub away: Option<String>,
}

impl MemberInfo {
    /// Check if user has operator status.
    pub fn is_op(&self) -> bool {
        self.prefixes.contains('@')
    }

    /// Check if user has voice.
    pub fn is_voice(&self) -> bool {
        self.prefixes.contains('+')
    }

    /// Check if user has halfop.
    pub fn is_halfop(&self) -> bool {
        self.prefixes.contains('%')
    }
}

/// Information about a user (for queries).
#[derive(Debug, Clone)]
pub struct UserInfo {
    /// Nickname.
    pub nick: String,

    /// User@host.
    pub userhost: Option<String>,

    /// Account name.
    pub account: Option<String>,

    /// Real name.
    pub realname: Option<String>,

    /// Away message (None = not away).
    pub away: Option<String>,
}

impl Default for SessionState {
    fn default() -> Self {
        Self::new()
    }
}

impl SessionState {
    /// Create a new session state.
    pub fn new() -> Self {
        Self {
            nick: String::new(),
            server_name: None,
            channels: HashMap::new(),
            users: HashMap::new(),
            user_modes: HashSet::new(),
            caps: CapabilityState::new(),
            registered: false,
            account: None,
            isupport: HashMap::new(),
        }
    }

    /// Get our current nickname.
    pub fn nick(&self) -> &str {
        &self.nick
    }

    /// Set our nickname.
    pub fn set_nick(&mut self, nick: impl Into<String>) {
        self.nick = nick.into();
    }

    /// Get the server name.
    pub fn server_name(&self) -> Option<&str> {
        self.server_name.as_deref()
    }

    /// Set the server name.
    pub fn set_server_name(&mut self, name: impl Into<String>) {
        self.server_name = Some(name.into());
    }

    /// Check if we're registered.
    pub fn is_registered(&self) -> bool {
        self.registered
    }

    /// Mark as registered.
    pub fn set_registered(&mut self, registered: bool) {
        self.registered = registered;
    }

    /// Get our account name.
    pub fn account(&self) -> Option<&str> {
        self.account.as_deref()
    }

    /// Set our account name.
    pub fn set_account(&mut self, account: Option<String>) {
        self.account = account;
    }

    /// Get capability state.
    pub fn caps(&self) -> &CapabilityState {
        &self.caps
    }

    /// Get mutable capability state.
    pub fn caps_mut(&mut self) -> &mut CapabilityState {
        &mut self.caps
    }

    // === Channel Operations ===

    /// Add a channel we've joined.
    pub fn add_channel(&mut self, name: &str) {
        let key = UniCase::new(name.to_string());
        if !self.channels.contains_key(&key) {
            self.channels.insert(
                key,
                ChannelState {
                    name: name.to_string(),
                    topic: None,
                    members: HashMap::new(),
                    modes: HashSet::new(),
                    key: None,
                    limit: None,
                },
            );
        }
    }

    /// Remove a channel we've left.
    pub fn remove_channel(&mut self, name: &str) {
        let key = UniCase::new(name.to_string());
        self.channels.remove(&key);
    }

    /// Get a channel by name.
    pub fn channel(&self, name: &str) -> Option<&ChannelState> {
        let key = UniCase::new(name.to_string());
        self.channels.get(&key)
    }

    /// Get a mutable channel by name.
    pub fn channel_mut(&mut self, name: &str) -> Option<&mut ChannelState> {
        let key = UniCase::new(name.to_string());
        self.channels.get_mut(&key)
    }

    /// Get all channel names.
    pub fn channel_names(&self) -> impl Iterator<Item = &str> {
        self.channels.values().map(|c| c.name.as_str())
    }

    /// Check if we're in a channel.
    pub fn is_in_channel(&self, name: &str) -> bool {
        let key = UniCase::new(name.to_string());
        self.channels.contains_key(&key)
    }

    /// Set channel topic.
    pub fn set_topic(&mut self, channel: &str, topic: Option<TopicInfo>) {
        if let Some(chan) = self.channel_mut(channel) {
            chan.topic = topic;
        }
    }

    /// Add a member to a channel.
    pub fn add_member(&mut self, channel: &str, nick: &str, info: MemberInfo) {
        if let Some(chan) = self.channel_mut(channel) {
            let key = UniCase::new(nick.to_string());
            chan.members.insert(key, info);
        }
    }

    /// Remove a member from a channel.
    pub fn remove_member(&mut self, channel: &str, nick: &str) {
        if let Some(chan) = self.channel_mut(channel) {
            let key = UniCase::new(nick.to_string());
            chan.members.remove(&key);
        }
    }

    /// Remove a user from all channels (on QUIT).
    pub fn remove_user_from_all_channels(&mut self, nick: &str) {
        let key = UniCase::new(nick.to_string());
        for chan in self.channels.values_mut() {
            chan.members.remove(&key);
        }
    }

    /// Rename a user in all channels (on NICK change).
    pub fn rename_user(&mut self, old_nick: &str, new_nick: &str) {
        let old_key = UniCase::new(old_nick.to_string());
        let new_key = UniCase::new(new_nick.to_string());

        for chan in self.channels.values_mut() {
            if let Some(info) = chan.members.remove(&old_key) {
                chan.members.insert(new_key.clone(), info);
            }
        }

        // Also update in users map
        if let Some(user) = self.users.remove(&old_key) {
            let mut user = user;
            user.nick = new_nick.to_string();
            self.users.insert(new_key, user);
        }
    }

    // === User Operations ===

    /// Add or update user info.
    pub fn update_user(&mut self, nick: &str, f: impl FnOnce(&mut UserInfo)) {
        let key = UniCase::new(nick.to_string());
        let user = self.users.entry(key).or_insert_with(|| UserInfo {
            nick: nick.to_string(),
            userhost: None,
            account: None,
            realname: None,
            away: None,
        });
        f(user);
    }

    /// Get user info.
    pub fn user(&self, nick: &str) -> Option<&UserInfo> {
        let key = UniCase::new(nick.to_string());
        self.users.get(&key)
    }

    // === ISUPPORT ===

    /// Set an ISUPPORT parameter.
    pub fn set_isupport(&mut self, key: &str, value: Option<String>) {
        self.isupport.insert(key.to_string(), value);
    }

    /// Get an ISUPPORT parameter.
    pub fn isupport(&self, key: &str) -> Option<Option<&str>> {
        self.isupport.get(key).map(|v| v.as_deref())
    }

    /// Get channel prefixes (from ISUPPORT PREFIX).
    pub fn prefix_chars(&self) -> &str {
        // Default IRC prefixes
        self.isupport
            .get("PREFIX")
            .and_then(|v| v.as_ref())
            .and_then(|v| v.split(')').nth(1))
            .unwrap_or("@+")
    }

    // === User Modes ===

    /// Add a user mode.
    pub fn add_user_mode(&mut self, mode: char) {
        self.user_modes.insert(mode);
    }

    /// Remove a user mode.
    pub fn remove_user_mode(&mut self, mode: char) {
        self.user_modes.remove(&mode);
    }

    /// Check if we have a user mode.
    pub fn has_user_mode(&self, mode: char) -> bool {
        self.user_modes.contains(&mode)
    }

    /// Get all user modes.
    pub fn user_modes(&self) -> impl Iterator<Item = char> + '_ {
        self.user_modes.iter().copied()
    }
}

impl ChannelState {
    /// Get a member by nick.
    pub fn member(&self, nick: &str) -> Option<&MemberInfo> {
        let key = UniCase::new(nick.to_string());
        self.members.get(&key)
    }

    /// Get a mutable member by nick.
    pub fn member_mut(&mut self, nick: &str) -> Option<&mut MemberInfo> {
        let key = UniCase::new(nick.to_string());
        self.members.get_mut(&key)
    }

    /// Get all member nicks.
    pub fn member_nicks(&self) -> impl Iterator<Item = &str> {
        self.members.keys().map(|k| k.as_str())
    }

    /// Get member count.
    pub fn member_count(&self) -> usize {
        self.members.len()
    }
}
