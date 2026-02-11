//! Server state management.
//!
//! This module contains the core state structures for the IRC server,
//! including client management and channel management.

mod client;

pub use client::{Client, ClientId, RegistrationPhase, UserModes};

use std::collections::{HashSet, VecDeque};
use std::net::IpAddr;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, RwLock};

use chrono::{DateTime, Utc};
use dashmap::DashMap;
use irc_proto::Message;
use tokio::sync::watch;
use unicase::UniCase;

use crate::cap::CapabilityRegistry;
use crate::config::ServerConfig;
use crate::db::Database;
use crate::db::bans::ServerBan;
use crate::error::Result;
use crate::lock::RwLockExt;

/// Shutdown signal for server admin commands.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ShutdownSignal {
    /// Server is running normally.
    #[default]
    Running,
    /// Server should reload configuration.
    Rehash,
    /// Server should restart.
    Restart,
    /// Server should shut down.
    Shutdown,
}

/// Entry in the WHOWAS history buffer.
#[derive(Debug, Clone)]
pub struct WhowasEntry {
    /// The nickname
    pub nickname: String,
    /// The username
    pub username: String,
    /// The hostname
    pub hostname: String,
    /// The realname
    pub realname: String,
    /// When the user quit
    pub quit_time: DateTime<Utc>,
    /// The server name
    pub server: String,
}

/// Maximum number of WHOWAS entries to keep.
const WHOWAS_MAX_ENTRIES: usize = 1000;

mod channel;
pub use channel::{Channel, ChannelModes, JoinError, MaskEntry, MemberStatus, matches_mask};

/// Central server state.
///
/// This struct holds all shared state for the server, using concurrent
/// data structures for safe access from multiple connection handlers.
pub struct ServerState {
    /// Server configuration.
    pub config: Arc<ServerConfig>,

    /// All connected clients, indexed by their unique ID.
    pub clients: DashMap<ClientId, Arc<Client>>,

    /// Nickname to client ID mapping (case-insensitive).
    pub nicknames: DashMap<UniCase<String>, ClientId>,

    /// Channels.
    pub channels: DashMap<UniCase<String>, Arc<RwLock<Channel>>>,

    /// Server creation time.
    pub created_at: DateTime<Utc>,

    /// Counter for generating unique client IDs.
    client_counter: AtomicU64,

    /// Message of the day (loaded from config).
    pub motd: tokio::sync::RwLock<Option<Vec<String>>>,

    /// WHOWAS history buffer.
    pub whowas_history: RwLock<VecDeque<WhowasEntry>>,

    /// IRCv3 capability registry.
    pub capabilities: CapabilityRegistry,

    /// Services database (optional).
    pub db: Option<Arc<Database>>,

    /// Shutdown signal sender.
    pub shutdown_tx: watch::Sender<ShutdownSignal>,

    /// Shutdown signal receiver (cloneable for connection handlers).
    pub shutdown_rx: watch::Receiver<ShutdownSignal>,

    /// K-line cache (user@host patterns).
    pub klines: DashMap<String, ServerBan>,

    /// Z-line cache (IP patterns).
    pub zlines: DashMap<String, ServerBan>,

    /// Connection count per IP address.
    pub connections_per_ip: DashMap<IpAddr, usize>,
}

impl ServerState {
    /// Create new server state with the given configuration.
    pub fn new(config: ServerConfig) -> Self {
        // Initialize database if configured
        let db = config
            .services
            .database_path
            .as_ref()
            .and_then(|path| {
                match Database::new(path) {
                    Ok(db) => {
                        tracing::info!(path = ?path, "Services database initialized");
                        Some(Arc::new(db))
                    }
                    Err(e) => {
                        tracing::warn!(path = ?path, error = %e, "Failed to initialize services database, services will be disabled");
                        None
                    }
                }
            });

        let (shutdown_tx, shutdown_rx) = watch::channel(ShutdownSignal::Running);

        Self {
            config: Arc::new(config),
            clients: DashMap::new(),
            nicknames: DashMap::new(),
            channels: DashMap::new(),
            created_at: Utc::now(),
            client_counter: AtomicU64::new(1),
            motd: tokio::sync::RwLock::new(None),
            whowas_history: RwLock::new(VecDeque::new()),
            capabilities: CapabilityRegistry::new(),
            db,
            shutdown_tx,
            shutdown_rx,
            klines: DashMap::new(),
            zlines: DashMap::new(),
            connections_per_ip: DashMap::new(),
        }
    }

    /// Create new server state with an in-memory database (for testing).
    pub fn with_memory_db(config: ServerConfig) -> Result<Self> {
        let db = Database::in_memory()?;
        let (shutdown_tx, shutdown_rx) = watch::channel(ShutdownSignal::Running);

        Ok(Self {
            config: Arc::new(config),
            clients: DashMap::new(),
            nicknames: DashMap::new(),
            channels: DashMap::new(),
            created_at: Utc::now(),
            client_counter: AtomicU64::new(1),
            motd: tokio::sync::RwLock::new(None),
            whowas_history: RwLock::new(VecDeque::new()),
            capabilities: CapabilityRegistry::new(),
            db: Some(Arc::new(db)),
            shutdown_tx,
            shutdown_rx,
            klines: DashMap::new(),
            zlines: DashMap::new(),
            connections_per_ip: DashMap::new(),
        })
    }

    /// Generate the next unique client ID.
    ///
    /// Uses wrapping arithmetic - will wrap to 0 after 2^64 connections.
    /// In practice this is unreachable (would take millions of years).
    pub fn next_client_id(&self) -> ClientId {
        ClientId(self.client_counter.fetch_add(1, Ordering::Relaxed))
    }

    /// Find a client by their nickname (case-insensitive).
    pub fn find_client_by_nick(&self, nick: &str) -> Option<Arc<Client>> {
        let key = UniCase::new(nick.to_string());
        self.nicknames
            .get(&key)
            .and_then(|id| self.clients.get(&id).map(|c| Arc::clone(&c)))
    }

    /// Register a nickname for a client.
    ///
    /// Returns `true` if successful, `false` if the nickname is already taken.
    pub fn register_nickname(&self, nick: &str, client_id: ClientId) -> bool {
        let key = UniCase::new(nick.to_string());

        // Use entry API to atomically check and insert
        use dashmap::mapref::entry::Entry;
        match self.nicknames.entry(key) {
            Entry::Occupied(_) => false,
            Entry::Vacant(entry) => {
                entry.insert(client_id);
                true
            }
        }
    }

    /// Unregister a nickname.
    pub fn unregister_nickname(&self, nick: &str) {
        let key = UniCase::new(nick.to_string());
        self.nicknames.remove(&key);
    }

    /// Add a client to the server.
    pub fn add_client(&self, client: Arc<Client>) {
        self.clients.insert(client.id, client);
    }

    /// Remove a client from the server.
    ///
    /// This also removes their nickname registration.
    pub fn remove_client(&self, client_id: ClientId) -> Result<Option<Arc<Client>>> {
        if let Some((_, client)) = self.clients.remove(&client_id) {
            // Remove nickname if registered
            if let Some(nick) = client.nickname()? {
                self.unregister_nickname(&nick);
            }
            Ok(Some(client))
        } else {
            Ok(None)
        }
    }

    /// Get the number of connected clients.
    pub fn client_count(&self) -> usize {
        self.clients.len()
    }

    /// Get the number of registered (fully connected) clients.
    pub fn registered_count(&self) -> Result<usize> {
        let mut count = 0;
        for c in self.clients.iter() {
            if c.is_registered()? {
                count += 1;
            }
        }
        Ok(count)
    }

    /// Get the number of invisible users.
    pub fn invisible_count(&self) -> Result<usize> {
        let mut count = 0;
        for c in self.clients.iter() {
            if c.is_registered()? && c.modes.read_lock("modes")?.invisible {
                count += 1;
            }
        }
        Ok(count)
    }

    /// Get the number of operators.
    pub fn operator_count(&self) -> Result<usize> {
        let mut count = 0;
        for c in self.clients.iter() {
            if c.is_registered()? && c.modes.read_lock("modes")?.operator {
                count += 1;
            }
        }
        Ok(count)
    }

    /// Get the number of channels.
    pub fn channel_count(&self) -> usize {
        self.channels.len()
    }

    /// Get or create a channel.
    ///
    /// Returns the channel and a boolean indicating if it was newly created.
    pub fn get_or_create_channel(&self, name: &str) -> (Arc<RwLock<Channel>>, bool) {
        let key = UniCase::new(name.to_string());

        use dashmap::mapref::entry::Entry;
        match self.channels.entry(key) {
            Entry::Occupied(entry) => (Arc::clone(entry.get()), false),
            Entry::Vacant(entry) => {
                let channel = Arc::new(RwLock::new(Channel::new(name.to_string())));
                entry.insert(Arc::clone(&channel));
                (channel, true)
            }
        }
    }

    /// Get a channel by name.
    pub fn get_channel(&self, name: &str) -> Option<Arc<RwLock<Channel>>> {
        let key = UniCase::new(name.to_string());
        self.channels.get(&key).map(|c| Arc::clone(&c))
    }

    /// Remove a channel.
    pub fn remove_channel(&self, name: &str) {
        let key = UniCase::new(name.to_string());
        self.channels.remove(&key);
    }

    /// Broadcast a message to all members of a channel.
    ///
    /// Optionally skip a specific client (usually the sender).
    /// Logs but continues if individual sends fail.
    pub fn broadcast_to_channel(
        &self,
        channel: &Channel,
        msg: Message,
        skip: Option<ClientId>,
    ) {
        for client_id in channel.member_ids() {
            if Some(client_id) == skip {
                continue;
            }
            if let Some(client) = self.clients.get(&client_id)
                && let Err(e) = client.send(msg.clone())
            {
                tracing::debug!(
                    client_id = %client_id,
                    error = %e,
                    "Failed to send broadcast message"
                );
            }
        }
    }

    /// Get all channels a client is in.
    pub fn get_client_channels(&self, client_id: ClientId) -> Result<Vec<Arc<RwLock<Channel>>>> {
        let mut result = Vec::new();
        for entry in self.channels.iter() {
            let channel = entry.value();
            if channel.read_lock("channel")?.is_member(client_id) {
                result.push(Arc::clone(channel));
            }
        }
        Ok(result)
    }

    /// Get members that share channels with a client (for QUIT broadcasting).
    pub fn get_common_channel_members(&self, client_id: ClientId) -> Result<HashSet<ClientId>> {
        let mut members = HashSet::new();
        for entry in self.channels.iter() {
            let channel = entry.value().read_lock("channel")?;
            if channel.is_member(client_id) {
                for member_id in channel.member_ids() {
                    if member_id != client_id {
                        members.insert(member_id);
                    }
                }
            }
        }
        Ok(members)
    }

    /// Remove a client from all channels and clean up empty ones.
    pub fn remove_client_from_all_channels(&self, client_id: ClientId) -> Result<Vec<String>> {
        let mut empty_channels = Vec::new();

        for entry in self.channels.iter() {
            let channel_name = entry.key().to_string();
            let mut channel = entry.value().write_lock("channel")?;
            if channel.remove_member(client_id).is_some()
                && channel.member_count() == 0
            {
                empty_channels.push(channel_name);
            }
        }

        // Clean up empty channels
        for name in &empty_channels {
            self.remove_channel(name);
        }

        Ok(empty_channels)
    }

    /// Record a WHOWAS entry for a disconnecting client.
    pub fn record_whowas(&self, entry: WhowasEntry) -> Result<()> {
        let mut history = self.whowas_history.write_lock("whowas_history")?;

        // Remove oldest entries if we're at capacity
        while history.len() >= WHOWAS_MAX_ENTRIES {
            history.pop_front();
        }

        history.push_back(entry);
        Ok(())
    }

    /// Look up WHOWAS entries for a nickname.
    ///
    /// Returns up to `count` entries (or all if None), most recent first.
    pub fn lookup_whowas(&self, nickname: &str, count: Option<u32>) -> Result<Vec<WhowasEntry>> {
        let history = self.whowas_history.read_lock("whowas_history")?;
        let nick_lower = nickname.to_lowercase();

        let mut results: Vec<_> = history
            .iter()
            .filter(|e| e.nickname.to_lowercase() == nick_lower)
            .cloned()
            .collect();

        // Most recent first
        results.reverse();

        // Limit results if count specified
        if let Some(n) = count {
            results.truncate(n as usize);
        }

        Ok(results)
    }

    // ========================================
    // Shutdown Signaling
    // ========================================

    /// Request a server rehash (config reload).
    pub fn request_rehash(&self) {
        let _ = self.shutdown_tx.send(ShutdownSignal::Rehash);
    }

    /// Request a server restart.
    pub fn request_restart(&self) {
        let _ = self.shutdown_tx.send(ShutdownSignal::Restart);
    }

    /// Request a server shutdown.
    pub fn request_shutdown(&self) {
        let _ = self.shutdown_tx.send(ShutdownSignal::Shutdown);
    }

    // ========================================
    // Server Bans (K-line/Z-line)
    // ========================================

    /// Check if a hostmask matches any K-line.
    pub fn is_klined(&self, hostmask: &str) -> Option<ServerBan> {
        for entry in self.klines.iter() {
            if matches_mask(entry.key(), hostmask) {
                return Some(entry.value().clone());
            }
        }
        None
    }

    /// Check if an IP matches any Z-line.
    pub fn is_zlined(&self, ip: &str) -> Option<ServerBan> {
        for entry in self.zlines.iter() {
            if matches_mask(entry.key(), ip) {
                return Some(entry.value().clone());
            }
        }
        None
    }

    /// Add a K-line.
    pub fn add_kline(&self, ban: ServerBan) {
        self.klines.insert(ban.mask.clone(), ban);
    }

    /// Remove a K-line.
    pub fn remove_kline(&self, mask: &str) -> bool {
        self.klines.remove(mask).is_some()
    }

    /// Add a Z-line.
    pub fn add_zline(&self, ban: ServerBan) {
        self.zlines.insert(ban.mask.clone(), ban);
    }

    /// Remove a Z-line.
    pub fn remove_zline(&self, mask: &str) -> bool {
        self.zlines.remove(mask).is_some()
    }

    /// Load bans from database into cache.
    pub fn load_bans_from_db(&self) -> Result<()> {
        use crate::db::bans::{self, BanType};

        if let Some(ref db) = self.db {
            let conn = db.connection()?;

            // Load K-lines
            for ban in bans::list_bans(&conn, BanType::Kline)? {
                self.klines.insert(ban.mask.clone(), ban);
            }

            // Load Z-lines
            for ban in bans::list_bans(&conn, BanType::Zline)? {
                self.zlines.insert(ban.mask.clone(), ban);
            }
        }
        Ok(())
    }

    // ========================================
    // Connection Tracking
    // ========================================

    /// Check if adding another connection from this IP would exceed the limit.
    pub fn check_connection_limit(&self, ip: IpAddr) -> bool {
        let count = self.connections_per_ip.get(&ip).map(|r| *r).unwrap_or(0);
        count < self.config.limits.max_connections_per_ip
    }

    /// Track a new connection from an IP.
    pub fn track_connection(&self, ip: IpAddr) {
        self.connections_per_ip
            .entry(ip)
            .and_modify(|c| *c += 1)
            .or_insert(1);
    }

    /// Untrack a connection from an IP.
    pub fn untrack_connection(&self, ip: IpAddr) {
        if let Some(mut count) = self.connections_per_ip.get_mut(&ip) {
            *count = count.saturating_sub(1);
            if *count == 0 {
                drop(count);
                self.connections_per_ip.remove(&ip);
            }
        }
    }

    // ========================================
    // MONITOR Support
    // ========================================

    /// Get clients monitoring a specific nickname.
    pub fn get_monitors_for_nick(&self, nick: &str) -> Result<Vec<Arc<Client>>> {
        let mut monitors = Vec::new();
        for entry in self.clients.iter() {
            let client = entry.value();
            if client.is_monitoring(nick)? {
                monitors.push(Arc::clone(client));
            }
        }
        Ok(monitors)
    }
}
