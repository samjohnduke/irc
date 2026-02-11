//! Channel state management (Phase 2 stub).

use std::collections::HashMap;

use chrono::{DateTime, Utc};

use super::ClientId;

/// Member status flags.
#[derive(Debug, Clone, Default)]
pub struct MemberStatus {
    /// Channel operator (@).
    pub operator: bool,
    /// Voice (+).
    pub voice: bool,
}

impl MemberStatus {
    /// Get the prefix character for this member.
    pub fn prefix_char(&self) -> Option<char> {
        if self.operator {
            Some('@')
        } else if self.voice {
            Some('+')
        } else {
            None
        }
    }
}

/// Channel modes.
#[derive(Debug, Clone, Default)]
pub struct ChannelModes {
    /// Invite only (+i).
    pub invite_only: bool,
    /// Moderated (+m).
    pub moderated: bool,
    /// No external messages (+n).
    pub no_external: bool,
    /// Secret (+s).
    pub secret: bool,
    /// Topic locked (+t).
    pub topic_locked: bool,
    /// Channel key (+k).
    pub key: Option<String>,
    /// User limit (+l).
    pub limit: Option<u32>,
}

/// A ban/exception/invite mask entry.
#[derive(Debug, Clone)]
pub struct MaskEntry {
    /// The mask pattern (nick!user@host).
    pub mask: String,
    /// Who set it.
    pub set_by: String,
    /// When it was set.
    pub set_at: DateTime<Utc>,
}

/// An IRC channel (Phase 2).
pub struct Channel {
    /// Channel name (including prefix).
    pub name: String,
    /// Channel topic.
    pub topic: Option<String>,
    /// Who set the topic.
    pub topic_set_by: Option<String>,
    /// When the topic was set.
    pub topic_set_at: Option<DateTime<Utc>>,
    /// Channel modes.
    pub modes: ChannelModes,
    /// Channel members.
    pub members: HashMap<ClientId, MemberStatus>,
    /// Ban list.
    pub bans: Vec<MaskEntry>,
    /// Exception list.
    pub exceptions: Vec<MaskEntry>,
    /// Invite list.
    pub invites: Vec<MaskEntry>,
    /// Channel creation time.
    pub created_at: DateTime<Utc>,
}

impl Channel {
    /// Create a new channel.
    pub fn new(name: String) -> Self {
        Self {
            name,
            topic: None,
            topic_set_by: None,
            topic_set_at: None,
            modes: ChannelModes::default(),
            members: HashMap::new(),
            bans: Vec::new(),
            exceptions: Vec::new(),
            invites: Vec::new(),
            created_at: Utc::now(),
        }
    }

    /// Get the number of members.
    pub fn member_count(&self) -> usize {
        self.members.len()
    }

    /// Check if a client is a member.
    pub fn is_member(&self, client_id: ClientId) -> bool {
        self.members.contains_key(&client_id)
    }

    /// Check if a client is an operator.
    pub fn is_operator(&self, client_id: ClientId) -> bool {
        self.members
            .get(&client_id)
            .map(|s| s.operator)
            .unwrap_or(false)
    }

    /// Check if a client has voice.
    pub fn has_voice(&self, client_id: ClientId) -> bool {
        self.members
            .get(&client_id)
            .map(|s| s.voice)
            .unwrap_or(false)
    }
}
