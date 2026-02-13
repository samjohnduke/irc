//! IRC protocol parsing, serialization, and types.
//!
//! This crate provides the core protocol layer for IRC, implementing
//! RFC 2812 message parsing and IRCv3 extensions.
//!
//! # Example
//!
//! ```
//! use irc_proto::{Message, Command};
//!
//! // Parse a message
//! let msg = Message::parse(b":nick!user@host PRIVMSG #channel :Hello!\r\n").unwrap();
//! assert_eq!(msg.prefix.unwrap().nick(), Some("nick"));
//!
//! // Create and serialize a message
//! let msg = Message::new(Command::Privmsg {
//!     target: "#channel".into(),
//!     message: "Hello!".into(),
//! });
//! assert_eq!(msg.to_string(), "PRIVMSG #channel :Hello!");
//! ```

mod command;
mod error;
mod message;
mod mode;
mod parse;
mod prefix;
mod reply;
pub mod s2s_command;
mod tags;
mod validate;

pub use command::Command;
pub use error::{ParseError, ValidationError};
pub use message::Message;
pub use mode::{ChannelMode, ModeChange, ModeChanges, UserMode};
pub use prefix::Prefix;
pub use tags::Tags;
pub use validate::{is_channel, irc_eq, validate_channel, validate_nickname};

// Re-export S2S types
pub use s2s_command::{
    validate_sid, validate_uid, uid_to_sid, S2SCommand, S2SMessage, SjoinMember,
};

// Re-export reply modules with their original names
pub use reply::describe as describe_numeric;
pub use reply::error as errors;
pub use reply::reply as replies;
pub use reply::register_errors;

/// Codec for streaming message parsing (tokio compatible)
pub use parse::MessageCodec;

/// Maximum length of an IRC message (including CRLF) - classic RFC 1459
pub const MAX_MESSAGE_LEN: usize = 512;

/// Maximum length of message content (excluding CRLF)
pub const MAX_MESSAGE_CONTENT_LEN: usize = 510;

/// Maximum length of an IRCv3 message with tags (including CRLF)
/// IRCv3.2 allows 4096 bytes for tags + 512 for message body
pub const MAX_MESSAGE_LEN_IRCV3: usize = 8191;

/// Maximum length of a nickname (default, can be overridden by ISUPPORT)
pub const MAX_NICK_LEN: usize = 9;

/// Maximum length of a channel name (default, can be overridden by ISUPPORT)
pub const MAX_CHANNEL_LEN: usize = 50;
