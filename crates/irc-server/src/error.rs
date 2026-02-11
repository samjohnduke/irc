//! Error types for the IRC server.

use std::io;

use thiserror::Error;

/// Server error type.
#[derive(Debug, Error)]
pub enum Error {
    /// I/O error
    #[error("I/O error: {0}")]
    Io(#[from] io::Error),

    /// Protocol parsing error
    #[error("Parse error: {0}")]
    Parse(#[from] irc_proto::ParseError),

    /// TLS error
    #[error("TLS error: {0}")]
    Tls(#[from] rustls::Error),

    /// Configuration error
    #[error("Configuration error: {0}")]
    Config(String),

    /// Client not registered
    #[error("Client not registered")]
    NotRegistered,

    /// Nickname already in use
    #[error("Nickname already in use: {0}")]
    NicknameInUse(String),

    /// Invalid nickname
    #[error("Invalid nickname: {0}")]
    InvalidNickname(String),

    /// No such nick
    #[error("No such nick: {0}")]
    NoSuchNick(String),

    /// No such channel
    #[error("No such channel: {0}")]
    NoSuchChannel(String),

    /// Already registered
    #[error("Already registered")]
    AlreadyRegistered,

    /// Not on channel
    #[error("Not on channel: {0}")]
    NotOnChannel(String),

    /// Cannot send to channel
    #[error("Cannot send to channel: {0}")]
    CannotSendToChannel(String),

    /// Need more parameters
    #[error("Need more parameters for {0}")]
    NeedMoreParams(String),

    /// Unknown command
    #[error("Unknown command: {0}")]
    UnknownCommand(String),

    /// Client disconnected
    #[error("Client disconnected")]
    Disconnected,

    /// Channel send error (client disconnected)
    #[error("Failed to send message to client")]
    SendError,
}

impl Error {
    /// Get the IRC numeric error code for this error, if applicable.
    pub fn numeric_code(&self) -> Option<u16> {
        use irc_proto::errors::*;

        match self {
            Error::NotRegistered => Some(ERR_NOTREGISTERED),
            Error::NicknameInUse(_) => Some(ERR_NICKNAMEINUSE),
            Error::InvalidNickname(_) => Some(ERR_ERRONEUSNICKNAME),
            Error::NoSuchNick(_) => Some(ERR_NOSUCHNICK),
            Error::NoSuchChannel(_) => Some(ERR_NOSUCHCHANNEL),
            Error::AlreadyRegistered => Some(ERR_ALREADYREGISTERED),
            Error::NotOnChannel(_) => Some(ERR_NOTONCHANNEL),
            Error::CannotSendToChannel(_) => Some(ERR_CANNOTSENDTOCHAN),
            Error::NeedMoreParams(_) => Some(ERR_NEEDMOREPARAMS),
            Error::UnknownCommand(_) => Some(ERR_UNKNOWNCOMMAND),
            _ => None,
        }
    }
}

/// Result type for IRC server operations.
pub type Result<T> = std::result::Result<T, Error>;
