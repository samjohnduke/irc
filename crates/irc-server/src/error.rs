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

    /// Channel is full
    #[error("Channel is full: {0}")]
    ChannelFull(String),

    /// Banned from channel
    #[error("Banned from channel: {0}")]
    BannedFromChannel(String),

    /// Bad channel key
    #[error("Bad channel key: {0}")]
    BadChannelKey(String),

    /// Invite only channel
    #[error("Invite only channel: {0}")]
    InviteOnlyChannel(String),

    /// Not channel operator
    #[error("Not channel operator: {0}")]
    NotChannelOperator(String),

    /// User not in channel
    #[error("User not in channel: {0}")]
    UserNotInChannel(String, String),

    /// User already on channel
    #[error("User already on channel: {0}")]
    UserOnChannel(String, String),

    /// Invalid channel name
    #[error("Invalid channel name: {0}")]
    InvalidChannel(String),

    /// Lock was poisoned (another thread panicked while holding it)
    #[error("Internal state lock poisoned: {0}")]
    LockPoisoned(String),

    /// Client send buffer is full (client too slow)
    #[error("Client send buffer full")]
    SendBufferFull,

    /// No operator privileges
    #[error("Permission Denied- You're not an IRC operator")]
    NoPrivileges,

    /// Password mismatch (OPER command)
    #[error("Password incorrect")]
    PasswordMismatch,

    /// No operator host (host mask doesn't match)
    #[error("No O-lines for your host")]
    NoOperHost,

    /// Database error
    #[error("Database error: {0}")]
    Database(String),

    /// Account already exists
    #[error("Account already exists: {0}")]
    AccountExists(String),

    /// Nickname already registered
    #[error("Nickname already registered: {0}")]
    NickRegistered(String),

    /// Channel already registered
    #[error("Channel already registered: {0}")]
    ChannelRegistered(String),

    /// Services unavailable (no database)
    #[error("Services unavailable")]
    ServicesUnavailable,

    /// Not logged in
    #[error("Not logged in")]
    NotLoggedIn,

    /// Password hashing error
    #[error("Password hashing error: {0}")]
    PasswordHash(String),

    /// Rate limited
    #[error("Rate limited")]
    RateLimited,

    /// Banned from server
    #[error("Banned: {0}")]
    Banned(String),

    /// S2S protocol error
    #[error("Protocol error: {0}")]
    Protocol(String),
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
            Error::ChannelFull(_) => Some(ERR_CHANNELISFULL),
            Error::BannedFromChannel(_) => Some(ERR_BANNEDFROMCHAN),
            Error::BadChannelKey(_) => Some(ERR_BADCHANNELKEY),
            Error::InviteOnlyChannel(_) => Some(ERR_INVITEONLYCHAN),
            Error::NotChannelOperator(_) => Some(ERR_CHANOPRIVSNEEDED),
            Error::UserNotInChannel(_, _) => Some(ERR_USERNOTINCHANNEL),
            Error::UserOnChannel(_, _) => Some(ERR_USERONCHANNEL),
            Error::InvalidChannel(_) => Some(ERR_NOSUCHCHANNEL),
            Error::NoPrivileges => Some(ERR_NOPRIVILEGES),
            Error::PasswordMismatch => Some(ERR_PASSWDMISMATCH),
            Error::NoOperHost => Some(ERR_NOOPERHOST),
            _ => None,
        }
    }
}

/// Result type for IRC server operations.
pub type Result<T> = std::result::Result<T, Error>;
