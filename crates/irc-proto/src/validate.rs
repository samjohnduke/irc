//! Validation functions for IRC identifiers.

use crate::error::ValidationError;
use crate::{MAX_CHANNEL_LEN, MAX_NICK_LEN};

/// Validate an IRC nickname.
///
/// Rules:
/// - Must not be empty
/// - Max 9 characters (default, can be extended via ISUPPORT NICKLEN)
/// - Must start with a letter or special character
/// - Allowed characters: a-z, A-Z, 0-9, and special: `[]\`^{|}-_`
///
/// # Examples
///
/// ```
/// use irc_proto::validate_nickname;
///
/// assert!(validate_nickname("nick").is_ok());
/// assert!(validate_nickname("nick123").is_ok());
/// assert!(validate_nickname("[nick]").is_ok());
/// assert!(validate_nickname("123nick").is_err()); // Can't start with digit
/// assert!(validate_nickname("").is_err()); // Empty
/// ```
pub fn validate_nickname(nick: &str) -> Result<(), ValidationError> {
    validate_nickname_with_max(nick, MAX_NICK_LEN)
}

/// Validate a nickname with a custom max length.
pub fn validate_nickname_with_max(nick: &str, max_len: usize) -> Result<(), ValidationError> {
    if nick.is_empty() {
        return Err(ValidationError::EmptyNickname);
    }

    if nick.len() > max_len {
        return Err(ValidationError::NicknameTooLong {
            max: max_len,
            got: nick.len(),
        });
    }

    let mut chars = nick.chars();

    // First character must be letter or special
    let first = chars.next().unwrap();
    if !is_nick_start_char(first) {
        return Err(ValidationError::InvalidNicknameStart(first));
    }

    // Rest can include digits
    for c in chars {
        if !is_nick_char(c) {
            return Err(ValidationError::InvalidNicknameChar(c));
        }
    }

    Ok(())
}

/// Check if a character is valid at the start of a nickname.
fn is_nick_start_char(c: char) -> bool {
    c.is_ascii_alphabetic() || is_special_char(c)
}

/// Check if a character is valid in a nickname.
fn is_nick_char(c: char) -> bool {
    c.is_ascii_alphanumeric() || is_special_char(c) || c == '-'
}

/// Check if a character is a "special" IRC character.
///
/// Special characters are: `[]\`^{|}_`
/// These are equivalent to letters in IRC's case mapping.
fn is_special_char(c: char) -> bool {
    matches!(c, '[' | ']' | '\\' | '`' | '^' | '{' | '|' | '}' | '_')
}

/// Validate an IRC channel name.
///
/// Rules:
/// - Must not be empty
/// - Max 50 characters (default, can be extended via ISUPPORT CHANNELLEN)
/// - Must start with #, &, +, or !
/// - Cannot contain space, comma, or control characters (ASCII 0-31, 127)
/// - Cannot contain colon (would break parsing)
///
/// # Examples
///
/// ```
/// use irc_proto::validate_channel;
///
/// assert!(validate_channel("#channel").is_ok());
/// assert!(validate_channel("&local").is_ok());
/// assert!(validate_channel("#chan-name").is_ok());
/// assert!(validate_channel("channel").is_err()); // Missing prefix
/// assert!(validate_channel("#chan nel").is_err()); // Contains space
/// ```
pub fn validate_channel(channel: &str) -> Result<(), ValidationError> {
    validate_channel_with_max(channel, MAX_CHANNEL_LEN)
}

/// Validate a channel name with a custom max length.
pub fn validate_channel_with_max(channel: &str, max_len: usize) -> Result<(), ValidationError> {
    if channel.is_empty() {
        return Err(ValidationError::EmptyChannel);
    }

    if channel.len() > max_len {
        return Err(ValidationError::ChannelTooLong {
            max: max_len,
            got: channel.len(),
        });
    }

    let mut chars = channel.chars();

    // First character must be a valid prefix
    let prefix = chars.next().unwrap();
    if !is_channel_prefix(prefix) {
        return Err(ValidationError::InvalidChannelPrefix(prefix));
    }

    // Rest cannot contain space, comma, colon, or control characters
    for c in chars {
        if !is_channel_char(c) {
            return Err(ValidationError::InvalidChannelChar(c));
        }
    }

    Ok(())
}

/// Check if a character is a valid channel prefix.
fn is_channel_prefix(c: char) -> bool {
    matches!(c, '#' | '&' | '+' | '!')
}

/// Check if a character is valid in a channel name (after the prefix).
fn is_channel_char(c: char) -> bool {
    // Disallow: space, comma, colon, NUL, BEL, CR, LF
    !matches!(c, ' ' | ',' | ':' | '\0' | '\x07' | '\r' | '\n')
        && !c.is_control()
}

/// Check if a string looks like a channel name.
#[inline]
pub fn is_channel(s: &str) -> bool {
    s.starts_with('#') || s.starts_with('&') || s.starts_with('+') || s.starts_with('!')
}

/// Compare two strings using IRC case mapping (RFC 1459).
///
/// In IRC, the following characters are considered equivalent:
/// - `{` and `[`
/// - `}` and `]`
/// - `|` and `\`
/// - `~` and `^`
pub fn irc_eq(a: &str, b: &str) -> bool {
    if a.len() != b.len() {
        return false;
    }

    a.chars()
        .zip(b.chars())
        .all(|(ca, cb)| irc_to_upper(ca) == irc_to_upper(cb))
}

/// Convert a character to IRC uppercase.
fn irc_to_upper(c: char) -> char {
    match c {
        'a'..='z' => (c as u8 - 32) as char,
        '{' => '[',
        '}' => ']',
        '|' => '\\',
        '~' => '^',
        _ => c,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_valid_nicknames() {
        assert!(validate_nickname("nick").is_ok());
        assert!(validate_nickname("Nick123").is_ok());
        assert!(validate_nickname("[nick]").is_ok());
        assert!(validate_nickname("nick_name").is_ok());
        assert!(validate_nickname("nick-name").is_ok());
    }

    #[test]
    fn test_invalid_nicknames() {
        assert!(validate_nickname("").is_err());
        assert!(validate_nickname("123nick").is_err());
        assert!(validate_nickname("nick name").is_err());
        assert!(validate_nickname("nick@host").is_err());
        assert!(validate_nickname("verylongnickname").is_err()); // > 9 chars
    }

    #[test]
    fn test_valid_channels() {
        assert!(validate_channel("#channel").is_ok());
        assert!(validate_channel("&local").is_ok());
        assert!(validate_channel("+modeless").is_ok());
        assert!(validate_channel("!12345safe").is_ok());
        assert!(validate_channel("#chan-name").is_ok());
        assert!(validate_channel("#CHANNEL").is_ok());
    }

    #[test]
    fn test_invalid_channels() {
        assert!(validate_channel("").is_err());
        assert!(validate_channel("channel").is_err()); // No prefix
        // Note: "#" alone is technically valid per RFC (prefix + empty name)
        assert!(validate_channel("#chan nel").is_err()); // Space
        assert!(validate_channel("#chan,nel").is_err()); // Comma
    }

    #[test]
    fn test_irc_case_eq() {
        assert!(irc_eq("nick", "NICK"));
        assert!(irc_eq("nick", "Nick"));
        assert!(irc_eq("[nick]", "{NICK}"));
        assert!(irc_eq("nick|away", "NICK\\AWAY"));
        assert!(!irc_eq("nick", "other"));
    }
}
