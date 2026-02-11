//! IRC channel and user modes.

use std::fmt;

/// Channel mode flags.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ChannelMode {
    /// `o` - Channel operator
    Operator,
    /// `v` - Voice (can speak in moderated channels)
    Voice,
    /// `i` - Invite only
    InviteOnly,
    /// `m` - Moderated (only voiced/ops can speak)
    Moderated,
    /// `n` - No external messages
    NoExternal,
    /// `t` - Only ops can change topic
    TopicLock,
    /// `s` - Secret channel
    Secret,
    /// `p` - Private channel
    Private,
    /// `k` - Channel key (password)
    Key,
    /// `l` - User limit
    Limit,
    /// `b` - Ban mask
    Ban,
    /// `e` - Ban exception mask
    Exception,
    /// `I` - Invite exception mask
    InviteException,
}

impl ChannelMode {
    /// Get the mode character.
    pub fn char(&self) -> char {
        match self {
            ChannelMode::Operator => 'o',
            ChannelMode::Voice => 'v',
            ChannelMode::InviteOnly => 'i',
            ChannelMode::Moderated => 'm',
            ChannelMode::NoExternal => 'n',
            ChannelMode::TopicLock => 't',
            ChannelMode::Secret => 's',
            ChannelMode::Private => 'p',
            ChannelMode::Key => 'k',
            ChannelMode::Limit => 'l',
            ChannelMode::Ban => 'b',
            ChannelMode::Exception => 'e',
            ChannelMode::InviteException => 'I',
        }
    }

    /// Parse a mode character.
    pub fn from_char(c: char) -> Option<Self> {
        match c {
            'o' => Some(ChannelMode::Operator),
            'v' => Some(ChannelMode::Voice),
            'i' => Some(ChannelMode::InviteOnly),
            'm' => Some(ChannelMode::Moderated),
            'n' => Some(ChannelMode::NoExternal),
            't' => Some(ChannelMode::TopicLock),
            's' => Some(ChannelMode::Secret),
            'p' => Some(ChannelMode::Private),
            'k' => Some(ChannelMode::Key),
            'l' => Some(ChannelMode::Limit),
            'b' => Some(ChannelMode::Ban),
            'e' => Some(ChannelMode::Exception),
            'I' => Some(ChannelMode::InviteException),
            _ => None,
        }
    }

    /// Check if this mode requires a parameter when adding.
    pub fn requires_param_add(&self) -> bool {
        matches!(
            self,
            ChannelMode::Operator
                | ChannelMode::Voice
                | ChannelMode::Key
                | ChannelMode::Limit
                | ChannelMode::Ban
                | ChannelMode::Exception
                | ChannelMode::InviteException
        )
    }

    /// Check if this mode requires a parameter when removing.
    pub fn requires_param_remove(&self) -> bool {
        matches!(
            self,
            ChannelMode::Operator
                | ChannelMode::Voice
                | ChannelMode::Key
                | ChannelMode::Ban
                | ChannelMode::Exception
                | ChannelMode::InviteException
        )
    }

    /// Check if this is a list mode (ban, exception, invite).
    pub fn is_list_mode(&self) -> bool {
        matches!(
            self,
            ChannelMode::Ban | ChannelMode::Exception | ChannelMode::InviteException
        )
    }

    /// Check if this is a member status mode (op, voice).
    pub fn is_status_mode(&self) -> bool {
        matches!(self, ChannelMode::Operator | ChannelMode::Voice)
    }
}

impl fmt::Display for ChannelMode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.char())
    }
}

/// User mode flags.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum UserMode {
    /// `i` - Invisible
    Invisible,
    /// `s` - Receive server notices
    ServerNotices,
    /// `w` - Receive wallops
    Wallops,
    /// `o` - IRC operator
    Operator,
}

impl UserMode {
    /// Get the mode character.
    pub fn char(&self) -> char {
        match self {
            UserMode::Invisible => 'i',
            UserMode::ServerNotices => 's',
            UserMode::Wallops => 'w',
            UserMode::Operator => 'o',
        }
    }

    /// Parse a mode character.
    pub fn from_char(c: char) -> Option<Self> {
        match c {
            'i' => Some(UserMode::Invisible),
            's' => Some(UserMode::ServerNotices),
            'w' => Some(UserMode::Wallops),
            'o' => Some(UserMode::Operator),
            _ => None,
        }
    }
}

impl fmt::Display for UserMode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.char())
    }
}

/// A single mode change.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModeChange {
    /// Whether the mode is being added (+) or removed (-)
    pub adding: bool,
    /// The mode being changed
    pub mode: char,
    /// Optional parameter (nick for +o, mask for +b, etc.)
    pub param: Option<String>,
}

impl ModeChange {
    /// Create a mode addition.
    pub fn add(mode: char, param: Option<String>) -> Self {
        Self {
            adding: true,
            mode,
            param,
        }
    }

    /// Create a mode removal.
    pub fn remove(mode: char, param: Option<String>) -> Self {
        Self {
            adding: false,
            mode,
            param,
        }
    }
}

/// A set of mode changes parsed from a MODE command.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ModeChanges {
    pub changes: Vec<ModeChange>,
}

impl ModeChanges {
    /// Create an empty mode change set.
    pub fn new() -> Self {
        Self::default()
    }

    /// Parse mode changes from mode string and parameters.
    ///
    /// # Example
    ///
    /// ```
    /// use irc_proto::ModeChanges;
    ///
    /// let changes = ModeChanges::parse("+ov-b", &["alice", "bob", "*!*@bad.host"]);
    /// assert_eq!(changes.changes.len(), 3);
    /// ```
    pub fn parse(modes: &str, params: &[&str]) -> Self {
        let mut changes = Vec::new();
        let mut adding = true;
        let mut param_idx = 0;

        for c in modes.chars() {
            match c {
                '+' => adding = true,
                '-' => adding = false,
                _ => {
                    // Check if this mode needs a parameter
                    let needs_param = if let Some(cm) = ChannelMode::from_char(c) {
                        if adding {
                            cm.requires_param_add()
                        } else {
                            cm.requires_param_remove()
                        }
                    } else {
                        false
                    };

                    let param = if needs_param && param_idx < params.len() {
                        let p = params[param_idx].to_string();
                        param_idx += 1;
                        Some(p)
                    } else {
                        None
                    };

                    changes.push(ModeChange {
                        adding,
                        mode: c,
                        param,
                    });
                }
            }
        }

        Self { changes }
    }

    /// Check if there are no changes.
    pub fn is_empty(&self) -> bool {
        self.changes.is_empty()
    }

    /// Get the number of changes.
    pub fn len(&self) -> usize {
        self.changes.len()
    }
}

impl fmt::Display for ModeChanges {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.changes.is_empty() {
            return Ok(());
        }

        let mut adding = true;
        let mut modes = String::new();
        let mut params = Vec::new();

        // Start with +
        modes.push('+');

        for change in &self.changes {
            if change.adding != adding {
                adding = change.adding;
                modes.push(if adding { '+' } else { '-' });
            }
            modes.push(change.mode);
            if let Some(ref p) = change.param {
                params.push(p.as_str());
            }
        }

        write!(f, "{}", modes)?;
        for param in params {
            write!(f, " {}", param)?;
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_modes() {
        let changes = ModeChanges::parse("+o-o+v", &["alice", "bob", "carol"]);
        assert_eq!(changes.changes.len(), 3);

        assert!(changes.changes[0].adding);
        assert_eq!(changes.changes[0].mode, 'o');
        assert_eq!(changes.changes[0].param, Some("alice".to_string()));

        assert!(!changes.changes[1].adding);
        assert_eq!(changes.changes[1].mode, 'o');
        assert_eq!(changes.changes[1].param, Some("bob".to_string()));

        assert!(changes.changes[2].adding);
        assert_eq!(changes.changes[2].mode, 'v');
        assert_eq!(changes.changes[2].param, Some("carol".to_string()));
    }

    #[test]
    fn test_parse_simple_modes() {
        let changes = ModeChanges::parse("+nt", &[]);
        assert_eq!(changes.changes.len(), 2);
        assert!(changes.changes[0].param.is_none());
    }

    #[test]
    fn test_display() {
        let changes = ModeChanges::parse("+ov", &["alice", "bob"]);
        let s = changes.to_string();
        assert!(s.contains("+ov"));
        assert!(s.contains("alice"));
        assert!(s.contains("bob"));
    }
}
