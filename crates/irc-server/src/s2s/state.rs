//! Server link state management.

use std::collections::HashSet;
use std::sync::RwLock;
use std::sync::atomic::{AtomicU64, Ordering};

use chrono::{DateTime, Utc};
use irc_proto::S2SMessage;
use tokio::sync::mpsc;

use crate::error::Result;
use crate::lock::RwLockExt;

/// State of a server link.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LinkState {
    /// Connection established, awaiting authentication.
    Connecting,
    /// Authentication in progress.
    Authenticating,
    /// Authenticated, awaiting BURST.
    Authenticated,
    /// Receiving BURST data.
    Bursting,
    /// Fully synchronized and operational.
    Ready,
    /// Link is being terminated.
    Disconnecting,
}

/// A link to another server in the network.
pub struct ServerLink {
    /// Remote server's unique ID (SID).
    pub sid: String,

    /// Remote server's name (e.g., "irc.server-b.local").
    pub name: String,

    /// Remote server's description.
    pub description: String,

    /// Hop count (1 = directly connected).
    pub hopcount: u32,

    /// The SID of the server we route through to reach this server.
    /// For directly connected servers, this is the same as `sid`.
    pub uplink_sid: String,

    /// Current link state.
    state: RwLock<LinkState>,

    /// Channel for sending messages to this link.
    pub sender: mpsc::Sender<S2SMessage>,

    /// When the link was established.
    pub connected_at: DateTime<Utc>,

    /// UIDs of users on this server (directly or via routing).
    pub user_uids: RwLock<HashSet<String>>,

    /// Servers behind this link (their SIDs).
    pub child_sids: RwLock<HashSet<String>>,

    /// TS version reported by remote.
    pub ts_version: u8,

    /// Capabilities advertised by remote.
    pub capabilities: RwLock<HashSet<String>>,
}

impl ServerLink {
    /// Create a new server link.
    pub fn new(sid: String, name: String, sender: mpsc::Sender<S2SMessage>) -> Self {
        Self {
            uplink_sid: sid.clone(),
            sid,
            name,
            description: String::new(),
            hopcount: 1,
            state: RwLock::new(LinkState::Connecting),
            sender,
            connected_at: Utc::now(),
            user_uids: RwLock::new(HashSet::new()),
            child_sids: RwLock::new(HashSet::new()),
            ts_version: 6,
            capabilities: RwLock::new(HashSet::new()),
        }
    }

    /// Create a new server link for a remote server (not directly connected).
    pub fn new_remote(
        sid: String,
        name: String,
        hopcount: u32,
        uplink_sid: String,
        sender: mpsc::Sender<S2SMessage>,
    ) -> Self {
        Self {
            sid,
            name,
            description: String::new(),
            hopcount,
            uplink_sid,
            state: RwLock::new(LinkState::Ready),
            sender,
            connected_at: Utc::now(),
            user_uids: RwLock::new(HashSet::new()),
            child_sids: RwLock::new(HashSet::new()),
            ts_version: 6,
            capabilities: RwLock::new(HashSet::new()),
        }
    }

    /// Get the current link state.
    pub fn state(&self) -> Result<LinkState> {
        Ok(*self.state.read_lock("state")?)
    }

    /// Set the link state.
    pub fn set_state(&self, new_state: LinkState) -> Result<()> {
        *self.state.write_lock("state")? = new_state;
        Ok(())
    }

    /// Check if the link is ready for normal operation.
    pub fn is_ready(&self) -> Result<bool> {
        Ok(*self.state.read_lock("state")? == LinkState::Ready)
    }

    /// Check if this is a directly connected server.
    pub fn is_direct(&self) -> bool {
        self.hopcount == 1
    }

    /// Set the server description.
    pub fn set_description(&self, _desc: String) {
        // Note: description is not behind RwLock, should only be set during setup
    }

    /// Add a capability.
    pub fn add_capability(&self, cap: &str) -> Result<()> {
        self.capabilities
            .write_lock("capabilities")?
            .insert(cap.to_string());
        Ok(())
    }

    /// Check if a capability is advertised.
    pub fn has_capability(&self, cap: &str) -> Result<bool> {
        Ok(self.capabilities.read_lock("capabilities")?.contains(cap))
    }

    /// Add a user UID to this server's user list.
    pub fn add_user(&self, uid: &str) -> Result<()> {
        self.user_uids
            .write_lock("user_uids")?
            .insert(uid.to_string());
        Ok(())
    }

    /// Remove a user UID from this server's user list.
    pub fn remove_user(&self, uid: &str) -> Result<bool> {
        Ok(self.user_uids.write_lock("user_uids")?.remove(uid))
    }

    /// Get the count of users on this server.
    pub fn user_count(&self) -> Result<usize> {
        Ok(self.user_uids.read_lock("user_uids")?.len())
    }

    /// Add a child server SID.
    pub fn add_child(&self, sid: &str) -> Result<()> {
        self.child_sids
            .write_lock("child_sids")?
            .insert(sid.to_string());
        Ok(())
    }

    /// Remove a child server SID.
    pub fn remove_child(&self, sid: &str) -> Result<bool> {
        Ok(self.child_sids.write_lock("child_sids")?.remove(sid))
    }

    /// Send a message through this link.
    pub fn send(&self, msg: S2SMessage) -> Result<bool> {
        match self.sender.try_send(msg) {
            Ok(()) => Ok(true),
            Err(mpsc::error::TrySendError::Full(_)) => {
                tracing::warn!(sid = %self.sid, "S2S send buffer full");
                Ok(false)
            }
            Err(mpsc::error::TrySendError::Closed(_)) => Ok(false),
        }
    }
}

/// Manager for server link connections.
///
/// Tracks all servers in the network and handles UID generation.
pub struct LinkManager {
    /// Our server's SID.
    pub our_sid: String,

    /// Counter for generating UIDs.
    uid_counter: AtomicU64,
}

impl LinkManager {
    /// Create a new link manager.
    pub fn new(our_sid: String) -> Self {
        Self {
            our_sid,
            uid_counter: AtomicU64::new(0),
        }
    }

    /// Generate the next UID for a local user.
    pub fn next_uid(&self) -> String {
        let counter = self.uid_counter.fetch_add(1, Ordering::Relaxed);
        super::generate_uid(&self.our_sid, counter)
    }
}
