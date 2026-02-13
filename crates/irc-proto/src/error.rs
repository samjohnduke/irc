//! Error types for protocol parsing and validation.

use thiserror::Error;

/// Errors that can occur when parsing IRC messages.
#[derive(Debug, Error)]
pub enum ParseError {
    /// Message exceeds maximum length
    #[error("message too long ({0} bytes)")]
    MessageTooLong(usize),

    /// Message is empty
    #[error("empty message")]
    EmptyMessage,

    /// Message missing CRLF terminator
    #[error("message missing CRLF terminator")]
    MissingTerminator,

    /// Invalid prefix format
    #[error("invalid prefix format: {0}")]
    InvalidPrefix(String),

    /// Empty command
    #[error("empty command")]
    EmptyCommand,

    /// Invalid numeric reply code
    #[error("invalid numeric code: {0}")]
    InvalidNumeric(String),

    /// Invalid UTF-8 in message
    #[error("invalid UTF-8 in message")]
    InvalidUtf8,

    /// Invalid tag format
    #[error("invalid tag format: {0}")]
    InvalidTag(String),

    /// Invalid mode string
    #[error("invalid mode string: {0}")]
    InvalidMode(String),
}

impl From<std::io::Error> for ParseError {
    fn from(_err: std::io::Error) -> Self {
        ParseError::InvalidUtf8 // Map I/O errors generically for now
    }
}

/// Errors that can occur when validating IRC identifiers.
#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum ValidationError {
    /// Nickname is empty
    #[error("nickname cannot be empty")]
    EmptyNickname,

    /// Nickname is too long
    #[error("nickname too long (max {max} chars, got {got})")]
    NicknameTooLong { max: usize, got: usize },

    /// Nickname starts with invalid character
    #[error("nickname must start with a letter or special character, got '{0}'")]
    InvalidNicknameStart(char),

    /// Nickname contains invalid character
    #[error("nickname contains invalid character: '{0}'")]
    InvalidNicknameChar(char),

    /// Channel name is empty
    #[error("channel name cannot be empty")]
    EmptyChannel,

    /// Channel name is too long
    #[error("channel name too long (max {max} chars, got {got})")]
    ChannelTooLong { max: usize, got: usize },

    /// Channel has invalid prefix
    #[error("channel must start with #, &, +, or !, got '{0}'")]
    InvalidChannelPrefix(char),

    /// Channel contains invalid character
    #[error("channel contains invalid character: '{0}'")]
    InvalidChannelChar(char),
}
