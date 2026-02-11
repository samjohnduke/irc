//! Channel state management.

use std::collections::{HashMap, HashSet};

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

/// Error returned when a user cannot join a channel.
#[derive(Debug, Clone)]
pub enum JoinError {
    /// Channel is full (+l limit reached)
    ChannelFull,
    /// User is banned (+b)
    Banned,
    /// Wrong or missing key (+k)
    BadKey,
    /// Channel is invite-only (+i) and user not invited
    InviteOnly,
}

/// An IRC channel.
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

    /// Add a member to the channel.
    pub fn add_member(&mut self, client_id: ClientId, status: MemberStatus) {
        self.members.insert(client_id, status);
    }

    /// Remove a member from the channel.
    pub fn remove_member(&mut self, client_id: ClientId) -> Option<MemberStatus> {
        self.members.remove(&client_id)
    }

    /// Set the channel topic.
    pub fn set_topic(&mut self, topic: Option<String>, set_by: String) {
        self.topic = topic;
        if self.topic.is_some() {
            self.topic_set_by = Some(set_by);
            self.topic_set_at = Some(Utc::now());
        } else {
            self.topic_set_by = None;
            self.topic_set_at = None;
        }
    }

    /// Check if a user can join the channel.
    ///
    /// Returns Ok(()) if the user can join, or a JoinError explaining why not.
    pub fn can_join(
        &self,
        hostmask: &str,
        key: Option<&str>,
        invited_clients: &HashSet<ClientId>,
        client_id: ClientId,
    ) -> Result<(), JoinError> {
        // Check limit
        if let Some(limit) = self.modes.limit {
            if self.members.len() >= limit as usize {
                return Err(JoinError::ChannelFull);
            }
        }

        // Check ban (unless has exception)
        if self.is_banned(hostmask) && !self.has_exception(hostmask) {
            return Err(JoinError::Banned);
        }

        // Check key
        if let Some(ref channel_key) = self.modes.key {
            match key {
                Some(k) if k == channel_key => {}
                _ => return Err(JoinError::BadKey),
            }
        }

        // Check invite-only
        if self.modes.invite_only {
            // Check if user has invite exception or was invited
            if !self.has_invite_exception(hostmask) && !invited_clients.contains(&client_id) {
                return Err(JoinError::InviteOnly);
            }
        }

        Ok(())
    }

    /// Check if a client can speak in the channel.
    pub fn can_speak(&self, client_id: ClientId, is_member: bool) -> bool {
        // +n: no external messages
        if self.modes.no_external && !is_member {
            return false;
        }

        // +m: moderated - only ops and voiced can speak
        if self.modes.moderated {
            if let Some(status) = self.members.get(&client_id) {
                return status.operator || status.voice;
            }
            return false;
        }

        true
    }

    /// Get the mode string (e.g., "+nt" or "+ntk secret").
    pub fn mode_string(&self) -> String {
        let mut modes = String::from("+");
        let mut params = Vec::new();

        if self.modes.invite_only {
            modes.push('i');
        }
        if self.modes.moderated {
            modes.push('m');
        }
        if self.modes.no_external {
            modes.push('n');
        }
        if self.modes.secret {
            modes.push('s');
        }
        if self.modes.topic_locked {
            modes.push('t');
        }
        if let Some(ref key) = self.modes.key {
            modes.push('k');
            params.push(key.clone());
        }
        if let Some(limit) = self.modes.limit {
            modes.push('l');
            params.push(limit.to_string());
        }

        if modes.len() == 1 {
            // Just "+"
            "+".to_string()
        } else if params.is_empty() {
            modes
        } else {
            format!("{} {}", modes, params.join(" "))
        }
    }

    /// Check if a hostmask is banned.
    pub fn is_banned(&self, hostmask: &str) -> bool {
        self.bans.iter().any(|entry| matches_mask(&entry.mask, hostmask))
    }

    /// Check if a hostmask has a ban exception.
    pub fn has_exception(&self, hostmask: &str) -> bool {
        self.exceptions.iter().any(|entry| matches_mask(&entry.mask, hostmask))
    }

    /// Check if a hostmask has an invite exception.
    pub fn has_invite_exception(&self, hostmask: &str) -> bool {
        self.invites.iter().any(|entry| matches_mask(&entry.mask, hostmask))
    }

    /// Get all member IDs.
    pub fn member_ids(&self) -> impl Iterator<Item = ClientId> + '_ {
        self.members.keys().copied()
    }

    /// Get the member status for a client.
    pub fn get_member_status(&self, client_id: ClientId) -> Option<&MemberStatus> {
        self.members.get(&client_id)
    }

    /// Get mutable member status for a client.
    pub fn get_member_status_mut(&mut self, client_id: ClientId) -> Option<&mut MemberStatus> {
        self.members.get_mut(&client_id)
    }

    /// Add a ban entry.
    pub fn add_ban(&mut self, mask: String, set_by: String) {
        if !self.bans.iter().any(|e| e.mask == mask) {
            self.bans.push(MaskEntry {
                mask,
                set_by,
                set_at: Utc::now(),
            });
        }
    }

    /// Remove a ban entry.
    pub fn remove_ban(&mut self, mask: &str) -> bool {
        let len = self.bans.len();
        self.bans.retain(|e| e.mask != mask);
        self.bans.len() < len
    }

    /// Add an exception entry.
    pub fn add_exception(&mut self, mask: String, set_by: String) {
        if !self.exceptions.iter().any(|e| e.mask == mask) {
            self.exceptions.push(MaskEntry {
                mask,
                set_by,
                set_at: Utc::now(),
            });
        }
    }

    /// Remove an exception entry.
    pub fn remove_exception(&mut self, mask: &str) -> bool {
        let len = self.exceptions.len();
        self.exceptions.retain(|e| e.mask != mask);
        self.exceptions.len() < len
    }

    /// Add an invite exception entry.
    pub fn add_invite_exception(&mut self, mask: String, set_by: String) {
        if !self.invites.iter().any(|e| e.mask == mask) {
            self.invites.push(MaskEntry {
                mask,
                set_by,
                set_at: Utc::now(),
            });
        }
    }

    /// Remove an invite exception entry.
    pub fn remove_invite_exception(&mut self, mask: &str) -> bool {
        let len = self.invites.len();
        self.invites.retain(|e| e.mask != mask);
        self.invites.len() < len
    }
}

/// Match a wildcard mask against a string (case-insensitive).
///
/// Supports `*` (matches any characters) and `?` (matches single character).
pub fn matches_mask(mask: &str, s: &str) -> bool {
    let mask = mask.to_lowercase();
    let s = s.to_lowercase();
    matches_mask_impl(mask.as_bytes(), s.as_bytes())
}

fn matches_mask_impl(mask: &[u8], s: &[u8]) -> bool {
    let mut mi = 0;
    let mut si = 0;
    let mut star_mi = None;
    let mut star_si = None;

    while si < s.len() {
        if mi < mask.len() && (mask[mi] == b'?' || mask[mi] == s[si]) {
            mi += 1;
            si += 1;
        } else if mi < mask.len() && mask[mi] == b'*' {
            star_mi = Some(mi);
            star_si = Some(si);
            mi += 1;
        } else if let (Some(smi), Some(ssi)) = (star_mi, star_si) {
            mi = smi + 1;
            si = ssi + 1;
            star_si = Some(ssi + 1);
        } else {
            return false;
        }
    }

    // Consume remaining '*' in mask
    while mi < mask.len() && mask[mi] == b'*' {
        mi += 1;
    }

    mi == mask.len()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_matches_mask_exact() {
        assert!(matches_mask("nick!user@host", "nick!user@host"));
        assert!(matches_mask("NICK!USER@HOST", "nick!user@host"));
        assert!(!matches_mask("nick!user@host", "other!user@host"));
    }

    #[test]
    fn test_matches_mask_star() {
        assert!(matches_mask("*!*@*", "nick!user@host"));
        assert!(matches_mask("*!*@host", "nick!user@host"));
        assert!(matches_mask("nick!*@*", "nick!user@host"));
        assert!(matches_mask("*!user@*", "nick!user@host"));
        assert!(!matches_mask("*!*@other", "nick!user@host"));
    }

    #[test]
    fn test_matches_mask_question() {
        assert!(matches_mask("nic?!user@host", "nick!user@host"));
        assert!(matches_mask("???k!user@host", "nick!user@host"));
        assert!(!matches_mask("ni?!user@host", "nick!user@host"));
    }

    #[test]
    fn test_matches_mask_combined() {
        assert!(matches_mask("*!*@*.example.com", "nick!user@foo.example.com"));
        assert!(matches_mask("*!*@*.example.com", "nick!user@bar.example.com"));
        assert!(!matches_mask("*!*@*.example.com", "nick!user@example.org"));
    }

    #[test]
    fn test_mode_string() {
        let mut channel = Channel::new("#test".to_string());
        assert_eq!(channel.mode_string(), "+");

        channel.modes.no_external = true;
        channel.modes.topic_locked = true;
        assert_eq!(channel.mode_string(), "+nt");

        channel.modes.key = Some("secret".to_string());
        assert!(channel.mode_string().contains("+ntk"));
        assert!(channel.mode_string().contains("secret"));

        channel.modes.limit = Some(50);
        assert!(channel.mode_string().contains("+ntkl"));
    }

    #[test]
    fn test_can_speak() {
        let mut channel = Channel::new("#test".to_string());
        let client1 = ClientId(1);
        let client2 = ClientId(2);

        channel.add_member(client1, MemberStatus::default());

        // Non-moderated: anyone can speak
        assert!(channel.can_speak(client1, true));
        assert!(channel.can_speak(client2, false));

        // +n: no external messages
        channel.modes.no_external = true;
        assert!(channel.can_speak(client1, true));
        assert!(!channel.can_speak(client2, false));

        // +m: moderated
        channel.modes.moderated = true;
        assert!(!channel.can_speak(client1, true)); // not voiced/op

        // Give voice
        channel.get_member_status_mut(client1).unwrap().voice = true;
        assert!(channel.can_speak(client1, true));
    }
}
