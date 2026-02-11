//! IRC message prefix (source).
//!
//! The prefix indicates the origin of a message. It can be either
//! a server name or a user identifier (nick!user@host).

use std::fmt;

use crate::ParseError;

/// Message prefix indicating the source of a message.
///
/// # Examples
///
/// Server prefix:
/// ```text
/// :irc.example.com NOTICE * :Server restarting
/// ```
///
/// User prefix:
/// ```text
/// :nick!user@host PRIVMSG #channel :Hello
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Prefix {
    /// Server name
    Server(String),

    /// User identifier
    User {
        /// Nickname
        nick: String,
        /// Username (optional)
        user: Option<String>,
        /// Hostname (optional)
        host: Option<String>,
    },
}

impl Prefix {
    /// Parse a prefix string (without the leading `:`).
    pub fn parse(input: &str) -> Result<Self, ParseError> {
        if input.is_empty() {
            return Err(ParseError::InvalidPrefix("empty prefix".into()));
        }

        // Check for user prefix (contains ! or @)
        if let Some(bang_pos) = input.find('!') {
            // nick!user@host or nick!user
            let nick = &input[..bang_pos];
            let rest = &input[bang_pos + 1..];

            let (user, host) = if let Some(at_pos) = rest.find('@') {
                (Some(&rest[..at_pos]), Some(&rest[at_pos + 1..]))
            } else {
                (Some(rest), None)
            };

            Ok(Prefix::User {
                nick: nick.to_string(),
                user: user.map(String::from),
                host: host.map(String::from),
            })
        } else if let Some(at_pos) = input.find('@') {
            // nick@host (no user)
            let nick = &input[..at_pos];
            let host = &input[at_pos + 1..];

            Ok(Prefix::User {
                nick: nick.to_string(),
                user: None,
                host: Some(host.to_string()),
            })
        } else if input.contains('.') {
            // Likely a server name (contains dots)
            Ok(Prefix::Server(input.to_string()))
        } else {
            // Just a nick
            Ok(Prefix::User {
                nick: input.to_string(),
                user: None,
                host: None,
            })
        }
    }

    /// Get the nickname if this is a user prefix.
    pub fn nick(&self) -> Option<&str> {
        match self {
            Prefix::User { nick, .. } => Some(nick),
            Prefix::Server(_) => None,
        }
    }

    /// Get the username if this is a user prefix.
    pub fn user(&self) -> Option<&str> {
        match self {
            Prefix::User { user, .. } => user.as_deref(),
            Prefix::Server(_) => None,
        }
    }

    /// Get the hostname.
    pub fn host(&self) -> Option<&str> {
        match self {
            Prefix::User { host, .. } => host.as_deref(),
            Prefix::Server(name) => Some(name),
        }
    }

    /// Check if this is a server prefix.
    pub fn is_server(&self) -> bool {
        matches!(self, Prefix::Server(_))
    }

    /// Check if this is a user prefix.
    pub fn is_user(&self) -> bool {
        matches!(self, Prefix::User { .. })
    }

    /// Create a user prefix from just a nickname.
    pub fn from_nick(nick: impl Into<String>) -> Self {
        Prefix::User {
            nick: nick.into(),
            user: None,
            host: None,
        }
    }

    /// Create a full user prefix.
    pub fn from_user(
        nick: impl Into<String>,
        user: impl Into<String>,
        host: impl Into<String>,
    ) -> Self {
        Prefix::User {
            nick: nick.into(),
            user: Some(user.into()),
            host: Some(host.into()),
        }
    }

    /// Create a server prefix.
    pub fn from_server(name: impl Into<String>) -> Self {
        Prefix::Server(name.into())
    }
}

impl fmt::Display for Prefix {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Prefix::Server(name) => write!(f, "{}", name),
            Prefix::User { nick, user, host } => {
                write!(f, "{}", nick)?;
                if let Some(u) = user {
                    write!(f, "!{}", u)?;
                }
                if let Some(h) = host {
                    write!(f, "@{}", h)?;
                }
                Ok(())
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_full_user() {
        let prefix = Prefix::parse("nick!user@host.example.com").unwrap();
        assert_eq!(prefix.nick(), Some("nick"));
        assert_eq!(prefix.user(), Some("user"));
        assert_eq!(prefix.host(), Some("host.example.com"));
    }

    #[test]
    fn test_parse_nick_host() {
        let prefix = Prefix::parse("nick@host.example.com").unwrap();
        assert_eq!(prefix.nick(), Some("nick"));
        assert_eq!(prefix.user(), None);
        assert_eq!(prefix.host(), Some("host.example.com"));
    }

    #[test]
    fn test_parse_nick_only() {
        let prefix = Prefix::parse("nick").unwrap();
        assert_eq!(prefix.nick(), Some("nick"));
        assert_eq!(prefix.user(), None);
        assert_eq!(prefix.host(), None);
    }

    #[test]
    fn test_parse_server() {
        let prefix = Prefix::parse("irc.example.com").unwrap();
        assert!(prefix.is_server());
        assert_eq!(prefix.host(), Some("irc.example.com"));
    }

    #[test]
    fn test_display_roundtrip() {
        let prefix = Prefix::from_user("nick", "user", "host");
        assert_eq!(prefix.to_string(), "nick!user@host");

        let parsed = Prefix::parse(&prefix.to_string()).unwrap();
        assert_eq!(prefix, parsed);
    }
}
