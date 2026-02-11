//! Server state management.
//!
//! This module contains the core state structures for the IRC server,
//! including client management and (in future phases) channel management.

mod client;

pub use client::{Client, ClientId, RegistrationPhase, UserModes};

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use chrono::{DateTime, Utc};
use dashmap::DashMap;
use tokio::sync::RwLock;
use unicase::UniCase;

use crate::config::ServerConfig;

// Phase 2 stub
mod channel;
pub use channel::Channel;

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

    /// Channels (Phase 2).
    pub channels: DashMap<UniCase<String>, Arc<RwLock<Channel>>>,

    /// Server creation time.
    pub created_at: DateTime<Utc>,

    /// Counter for generating unique client IDs.
    client_counter: AtomicU64,

    /// Message of the day (loaded from config).
    pub motd: RwLock<Option<Vec<String>>>,
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
            motd: RwLock::new(None),
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
    pub fn remove_client(&self, client_id: ClientId) -> Option<Arc<Client>> {
        if let Some((_, client)) = self.clients.remove(&client_id) {
            // Remove nickname if registered
            if let Some(nick) = client.nickname() {
                self.unregister_nickname(&nick);
            }
            Some(client)
        } else {
            None
        }
    }

    /// Get the number of connected clients.
    pub fn client_count(&self) -> usize {
        self.clients.len()
    }

    /// Get the number of registered (fully connected) clients.
    pub fn registered_count(&self) -> usize {
        self.clients
            .iter()
            .filter(|c| c.is_registered())
            .count()
    }

    /// Get the number of invisible users.
    pub fn invisible_count(&self) -> usize {
        self.clients
            .iter()
            .filter(|c| c.is_registered() && c.modes.read().unwrap().invisible)
            .count()
    }

    /// Get the number of operators.
    pub fn operator_count(&self) -> usize {
        self.clients
            .iter()
            .filter(|c| c.is_registered() && c.modes.read().unwrap().operator)
            .count()
    }

    /// Get the number of channels.
    pub fn channel_count(&self) -> usize {
        self.channels.len()
    }
}
