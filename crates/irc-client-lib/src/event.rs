//! Client events.
//!
//! Events are emitted by the client to notify the application of
//! IRC activity. All message events include optional IRCv3 metadata.

use chrono::{DateTime, Utc};

/// Events emitted by the client.
#[derive(Debug, Clone)]
pub enum Event {
    // === Connection Lifecycle ===
    /// Connected to server (TCP/TLS established).
    Connecting,

    /// Registration in progress (CAP/SASL/NICK/USER).
    Registering,

    /// Detailed connection progress for UI display.
    ConnectionProgress {
        /// Phase description.
        phase: String,
        /// Detail message.
        message: String,
    },

    /// Successfully registered with the server.
    Connected {
        /// Our confirmed nickname.
        nick: String,
        /// Server name.
        server: String,
        /// Welcome message.
        welcome: String,
    },

    /// Disconnected from server.
    Disconnected {
        /// Reason for disconnection.
        reason: Option<String>,
        /// Whether this was a clean disconnect.
        clean: bool,
    },

    /// Connection error.
    Error {
        /// Error message.
        message: String,
    },

    // === SASL Authentication ===
    /// SASL authentication succeeded.
    SaslSuccess {
        /// Authenticated account name.
        account: String,
    },

    /// SASL authentication failed.
    SaslFailed {
        /// Failure reason.
        reason: String,
    },

    // === Messages ===
    /// Received a private message (to user or channel).
    Privmsg {
        /// Message source (nick!user@host).
        source: String,
        /// Target (channel or our nick).
        target: String,
        /// Message text.
        message: String,
        /// IRCv3 metadata.
        meta: MessageMeta,
    },

    /// Received a notice.
    Notice {
        /// Message source.
        source: Option<String>,
        /// Target.
        target: String,
        /// Notice text.
        message: String,
        /// IRCv3 metadata.
        meta: MessageMeta,
    },

    /// Received a CTCP ACTION (/me).
    Action {
        /// Message source.
        source: String,
        /// Target (channel or our nick).
        target: String,
        /// Action text.
        action: String,
        /// IRCv3 metadata.
        meta: MessageMeta,
    },

    // === Channel Events ===
    /// User joined a channel.
    Join {
        /// User who joined.
        nick: String,
        /// User info (user@host).
        userhost: Option<String>,
        /// Channel joined.
        channel: String,
        /// Account name (extended-join).
        account: Option<String>,
        /// Real name (extended-join).
        realname: Option<String>,
    },

    /// User left a channel.
    Part {
        /// User who left.
        nick: String,
        /// Channel left.
        channel: String,
        /// Part message.
        message: Option<String>,
    },

    /// User was kicked from a channel.
    Kick {
        /// User who was kicked.
        nick: String,
        /// Channel kicked from.
        channel: String,
        /// User who kicked.
        kicker: String,
        /// Kick reason.
        reason: Option<String>,
    },

    /// User quit IRC.
    Quit {
        /// User who quit.
        nick: String,
        /// Quit message.
        message: Option<String>,
    },

    /// Channel topic changed.
    Topic {
        /// Channel.
        channel: String,
        /// New topic (None = topic cleared).
        topic: Option<String>,
        /// Who set the topic.
        setter: Option<String>,
    },

    /// Received channel names list.
    Names {
        /// Channel.
        channel: String,
        /// List of (prefix, nick) pairs.
        names: Vec<(String, String)>,
    },

    /// Channel mode changed.
    ChannelMode {
        /// Channel.
        channel: String,
        /// Who set the mode.
        setter: String,
        /// Mode string (e.g., "+o nick").
        modes: String,
    },

    // === User Events ===
    /// Our nickname changed.
    NickChange {
        /// Old nickname.
        old_nick: String,
        /// New nickname.
        new_nick: String,
    },

    /// Another user changed their nickname.
    Nick {
        /// Old nickname.
        old_nick: String,
        /// New nickname.
        new_nick: String,
    },

    /// User mode changed.
    UserMode {
        /// Mode string.
        modes: String,
    },

    /// User invited us to a channel.
    Invite {
        /// Who invited us.
        inviter: String,
        /// Channel we were invited to.
        channel: String,
    },

    // === IRCv3 Events ===
    /// User's account changed (account-notify).
    AccountChange {
        /// User whose account changed.
        nick: String,
        /// New account (None = logged out).
        account: Option<String>,
    },

    /// User's away status changed (away-notify).
    AwayChange {
        /// User whose status changed.
        nick: String,
        /// Away message (None = back).
        away: Option<String>,
    },

    /// User's host changed (chghost).
    HostChange {
        /// User whose host changed.
        nick: String,
        /// New username.
        user: String,
        /// New hostname.
        host: String,
    },

    /// User's realname changed (setname).
    RealnameChange {
        /// User whose realname changed.
        nick: String,
        /// New realname.
        realname: String,
    },

    /// Batch of messages (chathistory, etc.).
    Batch {
        /// Batch type (e.g., "chathistory").
        batch_type: String,
        /// Target (e.g., channel name).
        target: Option<String>,
        /// Messages in the batch.
        messages: Vec<Event>,
    },

    // === Server Messages ===
    /// Server MOTD line.
    Motd {
        /// MOTD line.
        line: String,
    },

    /// Server numeric reply.
    Numeric {
        /// Numeric code.
        code: u16,
        /// Parameters.
        params: Vec<String>,
    },

    /// Server error.
    ServerError {
        /// Error message.
        message: String,
    },

    /// Raw message (for debugging/unhandled messages).
    Raw {
        /// Raw message line.
        line: String,
    },

    /// Capabilities available/enabled.
    Capabilities {
        /// Available capabilities (from CAP LS).
        available: Vec<String>,
        /// Enabled capabilities.
        enabled: Vec<String>,
    },

    /// Ping from server (automatically replied to).
    Ping {
        /// Ping token.
        token: String,
    },
}

/// IRCv3 message metadata.
#[derive(Debug, Clone, Default)]
pub struct MessageMeta {
    /// Message timestamp (from @time tag).
    pub time: Option<DateTime<Utc>>,

    /// Message ID (from @msgid tag).
    pub msgid: Option<String>,

    /// Sender's account (from @account tag).
    pub account: Option<String>,

    /// Label for request correlation (from @label tag).
    pub label: Option<String>,

    /// Whether this is an echo of our own message.
    pub echo: bool,
}

impl MessageMeta {
    /// Create metadata from IRC message tags.
    pub fn from_tags(tags: Option<&irc_proto::Tags>) -> Self {
        let tags = match tags {
            Some(t) => t,
            None => return Self::default(),
        };

        Self {
            time: tags.time().and_then(|t| {
                DateTime::parse_from_rfc3339(t)
                    .ok()
                    .map(|dt| dt.with_timezone(&Utc))
            }),
            msgid: tags.msgid().map(String::from),
            account: tags.account().map(String::from),
            label: tags.label().map(String::from),
            echo: false,
        }
    }
}
