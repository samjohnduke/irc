//! Server state management.
//!
//! This module contains the core state structures for the IRC server,
//! including client management and channel management.

mod client;

pub use client::{Client, ClientId, RegistrationPhase, UserModes};

use std::collections::HashSet;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, RwLock};

use chrono::{DateTime, Utc};
use dashmap::DashMap;
use irc_proto::Message;
use unicase::UniCase;

use crate::config::ServerConfig;
use crate::error::Result;
use crate::lock::RwLockExt;

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
}

impl ServerState {
    /// Create new server state with the given configuration.
    pub fn new(config: ServerConfig) -> Self {
        Self {
            config: Arc::new(config),
            clients: DashMap::new(),
            nicknames: DashMap::new(),
            channels: DashMap::new(),
            created_at: Utc::now(),
            client_counter: AtomicU64::new(1),
            motd: tokio::sync::RwLock::new(None),
        }
    }

    /// Generate the next unique client ID.
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
            if let Some(client) = self.clients.get(&client_id) {
                client.send(msg.clone());
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
            if channel.remove_member(client_id).is_some() {
                if channel.member_count() == 0 {
                    empty_channels.push(channel_name);
                }
            }
        }

        // Clean up empty channels
        for name in &empty_channels {
            self.remove_channel(name);
        }

        Ok(empty_channels)
    }
}
