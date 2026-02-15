//! SASL authentication support.
//!
//! This module implements the SASL authentication mechanism for IRC,
//! starting with the PLAIN mechanism.

use base64::{Engine, engine::general_purpose::STANDARD as BASE64};

/// SASL authentication state machine.
#[derive(Debug, Clone)]
pub enum SaslState {
    /// Waiting for mechanism selection (AUTHENTICATE <mechanism>).
    WaitingForMechanism,
    /// Authenticating with a specific mechanism.
    Authenticating {
        mechanism: SaslMechanism,
        /// Accumulated data (for multi-chunk transfers).
        data: Vec<u8>,
    },
    /// Authentication complete (success or failure already sent).
    Complete,
}

/// Supported SASL mechanisms.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SaslMechanism {
    /// PLAIN mechanism (base64 encoded "authzid\0authcid\0password").
    Plain,
}

impl SaslMechanism {
    /// Parse a mechanism name.
    pub fn parse(name: &str) -> Option<Self> {
        match name.to_uppercase().as_str() {
            "PLAIN" => Some(SaslMechanism::Plain),
            _ => None,
        }
    }

    /// Get the mechanism name.
    pub fn name(&self) -> &'static str {
        match self {
            SaslMechanism::Plain => "PLAIN",
        }
    }
}

/// Decoded PLAIN mechanism credentials.
#[derive(Debug, Clone)]
pub struct PlainCredentials {
    /// Authorization identity (can be empty to use authcid).
    pub authzid: String,
    /// Authentication identity (username).
    pub authcid: String,
    /// Password.
    pub password: String,
}

/// Decode PLAIN mechanism data.
///
/// PLAIN format: base64(authzid NUL authcid NUL password)
/// - authzid: Authorization identity (usually empty)
/// - authcid: Authentication identity (username)
/// - password: User's password
pub fn decode_plain(data: &str) -> Result<PlainCredentials, SaslError> {
    // Decode base64
    let decoded = BASE64
        .decode(data.as_bytes())
        .map_err(|_| SaslError::InvalidBase64)?;

    // Split on NUL bytes
    let parts: Vec<&[u8]> = decoded.split(|&b| b == 0).collect();

    if parts.len() != 3 {
        return Err(SaslError::MalformedPlain);
    }

    let authzid = String::from_utf8(parts[0].to_vec()).map_err(|_| SaslError::InvalidUtf8)?;
    let authcid = String::from_utf8(parts[1].to_vec()).map_err(|_| SaslError::InvalidUtf8)?;
    let password = String::from_utf8(parts[2].to_vec()).map_err(|_| SaslError::InvalidUtf8)?;

    if authcid.is_empty() {
        return Err(SaslError::EmptyAuthcid);
    }

    Ok(PlainCredentials {
        authzid,
        authcid,
        password,
    })
}

/// Encode PLAIN mechanism data (for testing).
pub fn encode_plain(authzid: &str, authcid: &str, password: &str) -> String {
    let data = format!("{}\0{}\0{}", authzid, authcid, password);
    BASE64.encode(data.as_bytes())
}

/// SASL-related errors.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SaslError {
    /// Invalid base64 encoding.
    InvalidBase64,
    /// Malformed PLAIN data (wrong number of NUL separators).
    MalformedPlain,
    /// Invalid UTF-8 in credentials.
    InvalidUtf8,
    /// Empty authentication identity.
    EmptyAuthcid,
    /// Unknown mechanism.
    UnknownMechanism,
    /// Authentication failed (wrong password).
    AuthFailed,
    /// Data too long.
    TooLong,
    /// Authentication aborted.
    Aborted,
    /// Already authenticated.
    AlreadyAuthenticated,
}

impl std::fmt::Display for SaslError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SaslError::InvalidBase64 => write!(f, "Invalid base64 encoding"),
            SaslError::MalformedPlain => write!(f, "Malformed PLAIN data"),
            SaslError::InvalidUtf8 => write!(f, "Invalid UTF-8 in credentials"),
            SaslError::EmptyAuthcid => write!(f, "Empty authentication identity"),
            SaslError::UnknownMechanism => write!(f, "Unknown SASL mechanism"),
            SaslError::AuthFailed => write!(f, "Authentication failed"),
            SaslError::TooLong => write!(f, "SASL data too long"),
            SaslError::Aborted => write!(f, "Authentication aborted"),
            SaslError::AlreadyAuthenticated => write!(f, "Already authenticated"),
        }
    }
}

impl std::error::Error for SaslError {}

/// List of supported SASL mechanisms.
pub fn supported_mechanisms() -> &'static str {
    "PLAIN"
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_decode_plain() {
        // Test vector: authzid="", authcid="testuser", password="testpass"
        // Base64 of "\0testuser\0testpass"
        let encoded = encode_plain("", "testuser", "testpass");
        let creds = decode_plain(&encoded).unwrap();

        assert_eq!(creds.authzid, "");
        assert_eq!(creds.authcid, "testuser");
        assert_eq!(creds.password, "testpass");
    }

    #[test]
    fn test_decode_plain_with_authzid() {
        let encoded = encode_plain("admin", "testuser", "testpass");
        let creds = decode_plain(&encoded).unwrap();

        assert_eq!(creds.authzid, "admin");
        assert_eq!(creds.authcid, "testuser");
        assert_eq!(creds.password, "testpass");
    }

    #[test]
    fn test_decode_plain_invalid_base64() {
        let result = decode_plain("not valid base64!!!");
        assert!(matches!(result, Err(SaslError::InvalidBase64)));
    }

    #[test]
    fn test_decode_plain_malformed() {
        // Only one NUL - should fail
        let encoded = BASE64.encode(b"test\0user");
        let result = decode_plain(&encoded);
        assert!(matches!(result, Err(SaslError::MalformedPlain)));
    }

    #[test]
    fn test_decode_plain_empty_authcid() {
        let encoded = encode_plain("", "", "password");
        let result = decode_plain(&encoded);
        assert!(matches!(result, Err(SaslError::EmptyAuthcid)));
    }

    #[test]
    fn test_mechanism_parse() {
        assert_eq!(SaslMechanism::parse("PLAIN"), Some(SaslMechanism::Plain));
        assert_eq!(SaslMechanism::parse("plain"), Some(SaslMechanism::Plain));
        assert_eq!(SaslMechanism::parse("UNKNOWN"), None);
    }

    #[test]
    fn test_encode_decode_roundtrip() {
        let encoded = encode_plain("", "alice", "secret123");
        let decoded = decode_plain(&encoded).unwrap();

        assert_eq!(decoded.authzid, "");
        assert_eq!(decoded.authcid, "alice");
        assert_eq!(decoded.password, "secret123");
    }
}
