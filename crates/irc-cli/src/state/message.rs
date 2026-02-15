//! Display messages for the TUI.

use chrono::{DateTime, Utc};

/// A message to display in a buffer.
#[derive(Debug, Clone)]
pub struct DisplayMessage {
    /// Message timestamp.
    pub time: DateTime<Utc>,

    /// Message type/content.
    pub kind: MessageKind,

    /// Whether this message is from us (echo).
    pub is_echo: bool,

    /// Unique message ID (for deduplication).
    pub msgid: Option<String>,
}

/// Types of displayable messages.
#[derive(Debug, Clone)]
pub enum MessageKind {
    /// Regular chat message.
    Privmsg { nick: String, text: String },

    /// CTCP ACTION (/me).
    Action { nick: String, text: String },

    /// Notice message.
    Notice {
        source: Option<String>,
        text: String,
    },

    /// User joined channel.
    Join {
        nick: String,
        userhost: Option<String>,
    },

    /// User left channel.
    Part {
        nick: String,
        message: Option<String>,
    },

    /// User quit IRC.
    Quit {
        nick: String,
        message: Option<String>,
    },

    /// Aggregated join events (multiple users joined).
    AggregatedJoin { nicks: Vec<String> },

    /// Aggregated part/quit events (multiple users left).
    AggregatedPart { nicks: Vec<String>, is_quit: bool },

    /// User was kicked.
    Kick {
        nick: String,
        kicker: String,
        reason: Option<String>,
    },

    /// Nick changed.
    Nick { old_nick: String, new_nick: String },

    /// Topic changed.
    Topic {
        setter: Option<String>,
        topic: Option<String>,
    },

    /// Channel mode changed.
    Mode { setter: String, modes: String },

    /// Server message.
    Server { text: String },

    /// Error message.
    Error { text: String },

    /// History separator.
    HistorySeparator,
}

impl DisplayMessage {
    /// Create a new message with the current time.
    pub fn new(kind: MessageKind) -> Self {
        Self {
            time: Utc::now(),
            kind,
            is_echo: false,
            msgid: None,
        }
    }

    /// Create a message with a specific time.
    pub fn with_time(time: DateTime<Utc>, kind: MessageKind) -> Self {
        Self {
            time,
            kind,
            is_echo: false,
            msgid: None,
        }
    }

    /// Mark as echo (our own message).
    pub fn echo(mut self) -> Self {
        self.is_echo = true;
        self
    }

    /// Set message ID.
    pub fn with_msgid(mut self, msgid: String) -> Self {
        self.msgid = Some(msgid);
        self
    }

    /// Create a privmsg.
    pub fn privmsg(nick: impl Into<String>, text: impl Into<String>) -> Self {
        Self::new(MessageKind::Privmsg {
            nick: nick.into(),
            text: text.into(),
        })
    }

    /// Create an action.
    pub fn action(nick: impl Into<String>, text: impl Into<String>) -> Self {
        Self::new(MessageKind::Action {
            nick: nick.into(),
            text: text.into(),
        })
    }

    /// Create a notice.
    pub fn notice(source: Option<String>, text: impl Into<String>) -> Self {
        Self::new(MessageKind::Notice {
            source,
            text: text.into(),
        })
    }

    /// Create a join message.
    pub fn join(nick: impl Into<String>, userhost: Option<String>) -> Self {
        Self::new(MessageKind::Join {
            nick: nick.into(),
            userhost,
        })
    }

    /// Create a part message.
    pub fn part(nick: impl Into<String>, message: Option<String>) -> Self {
        Self::new(MessageKind::Part {
            nick: nick.into(),
            message,
        })
    }

    /// Create a quit message.
    pub fn quit(nick: impl Into<String>, message: Option<String>) -> Self {
        Self::new(MessageKind::Quit {
            nick: nick.into(),
            message,
        })
    }

    /// Create a kick message.
    pub fn kick(
        nick: impl Into<String>,
        kicker: impl Into<String>,
        reason: Option<String>,
    ) -> Self {
        Self::new(MessageKind::Kick {
            nick: nick.into(),
            kicker: kicker.into(),
            reason,
        })
    }

    /// Create a nick change message.
    pub fn nick_change(old_nick: impl Into<String>, new_nick: impl Into<String>) -> Self {
        Self::new(MessageKind::Nick {
            old_nick: old_nick.into(),
            new_nick: new_nick.into(),
        })
    }

    /// Create a topic message.
    pub fn topic(setter: Option<String>, topic: Option<String>) -> Self {
        Self::new(MessageKind::Topic { setter, topic })
    }

    /// Create a mode message.
    pub fn mode(setter: impl Into<String>, modes: impl Into<String>) -> Self {
        Self::new(MessageKind::Mode {
            setter: setter.into(),
            modes: modes.into(),
        })
    }

    /// Create a server message.
    pub fn server(text: impl Into<String>) -> Self {
        Self::new(MessageKind::Server { text: text.into() })
    }

    /// Create an error message.
    pub fn error(text: impl Into<String>) -> Self {
        Self::new(MessageKind::Error { text: text.into() })
    }

    /// Create a history separator.
    pub fn history_separator() -> Self {
        Self::new(MessageKind::HistorySeparator)
    }

    /// Create an aggregated join message.
    pub fn aggregated_join(nicks: Vec<String>) -> Self {
        Self::new(MessageKind::AggregatedJoin { nicks })
    }

    /// Create an aggregated part/quit message.
    pub fn aggregated_part(nicks: Vec<String>, is_quit: bool) -> Self {
        Self::new(MessageKind::AggregatedPart { nicks, is_quit })
    }

    /// Check if this is a join/part/quit event that can be aggregated.
    #[allow(dead_code)]
    pub fn is_join_part_event(&self) -> bool {
        matches!(
            self.kind,
            MessageKind::Join { .. } | MessageKind::Part { .. } | MessageKind::Quit { .. }
        )
    }

    /// Get the nick from a join/part/quit event.
    pub fn join_part_nick(&self) -> Option<&str> {
        match &self.kind {
            MessageKind::Join { nick, .. }
            | MessageKind::Part { nick, .. }
            | MessageKind::Quit { nick, .. } => Some(nick),
            _ => None,
        }
    }

    /// Check if this is a join event.
    pub fn is_join(&self) -> bool {
        matches!(self.kind, MessageKind::Join { .. })
    }

    /// Check if this is a quit event (vs part).
    pub fn is_quit(&self) -> bool {
        matches!(self.kind, MessageKind::Quit { .. })
    }
}
