//! Client error types.

use std::io;
use thiserror::Error;

/// Main client error type.
#[derive(Debug, Error)]
pub enum Error {
    /// Connection error.
    #[error("connection error: {0}")]
    Connection(#[from] ConnectionError),

    /// Registration error.
    #[error("registration error: {0}")]
    Registration(#[from] RegistrationError),

    /// Protocol error.
    #[error("protocol error: {0}")]
    Protocol(#[from] irc_proto::ParseError),

    /// I/O error.
    #[error("I/O error: {0}")]
    Io(#[from] io::Error),

    /// Client is disconnected.
    #[error("client is disconnected")]
    Disconnected,

    /// Channel send error.
    #[error("send error: channel closed")]
    SendError,
}

/// Connection-related errors.
#[derive(Debug, Error)]
pub enum ConnectionError {
    /// DNS resolution failed.
    #[error("DNS resolution failed for {host}: {source}")]
    DnsResolution {
        host: String,
        #[source]
        source: io::Error,
    },

    /// TCP connection failed.
    #[error("TCP connection to {addr} failed: {source}")]
    TcpConnect {
        addr: String,
        #[source]
        source: io::Error,
    },

    /// TLS handshake failed.
    #[error("TLS handshake failed: {0}")]
    TlsHandshake(#[from] TlsError),

    /// Connection timeout.
    #[error("connection timed out after {0:?}")]
    Timeout(std::time::Duration),

    /// Connection closed by server.
    #[error("connection closed by server")]
    ClosedByServer,

    /// Invalid server name for TLS.
    #[error("invalid server name for TLS: {0}")]
    InvalidServerName(String),
}

/// TLS-specific errors.
#[derive(Debug, Error)]
pub enum TlsError {
    /// TLS configuration error.
    #[error("TLS configuration error: {0}")]
    Config(String),

    /// Certificate error.
    #[error("certificate error: {0}")]
    Certificate(String),

    /// Handshake error.
    #[error("handshake error: {0}")]
    Handshake(String),
}

/// Registration errors.
#[derive(Debug, Error)]
pub enum RegistrationError {
    /// SASL authentication failed.
    #[error("SASL authentication failed: {0}")]
    SaslFailed(#[from] SaslError),

    /// Nickname rejected.
    #[error("nickname {nick} rejected: {reason}")]
    NickRejected { nick: String, reason: String },

    /// All nicknames exhausted.
    #[error("all nicknames rejected")]
    NoValidNick,

    /// Registration timeout.
    #[error("registration timed out")]
    Timeout,

    /// Server rejected connection.
    #[error("server rejected connection: {0}")]
    Rejected(String),

    /// Unexpected server response.
    #[error("unexpected server response: {0}")]
    UnexpectedResponse(String),

    /// Banned from server.
    #[error("banned from server: {0}")]
    Banned(String),
}

/// SASL authentication errors.
#[derive(Debug, Error)]
pub enum SaslError {
    /// Mechanism not supported by server.
    #[error("mechanism {0} not supported by server")]
    MechanismNotSupported(String),

    /// Invalid credentials.
    #[error("invalid credentials")]
    InvalidCredentials,

    /// Authentication aborted by server.
    #[error("authentication aborted")]
    Aborted,

    /// Base64 encoding/decoding error.
    #[error("base64 error: {0}")]
    Base64(String),

    /// Server does not support SASL.
    #[error("server does not support SASL")]
    NotSupported,

    /// Authentication timeout.
    #[error("SASL authentication timed out")]
    Timeout,
}

impl From<base64::DecodeError> for SaslError {
    fn from(e: base64::DecodeError) -> Self {
        SaslError::Base64(e.to_string())
    }
}
