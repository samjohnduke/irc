//! IRC message parsing and serialization.

use std::fmt;

use crate::command::Command;
use crate::error::ParseError;
use crate::prefix::Prefix;
use crate::tags::Tags;

/// A parsed IRC message.
///
/// IRC messages have the format:
/// ```text
/// [@tags] [:prefix] <command> [params] CRLF
/// ```
///
/// # Examples
///
/// ```
/// use irc_proto::Message;
///
/// let msg = Message::parse(b"PING :server\r\n").unwrap();
/// println!("{}", msg);
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Message {
    /// IRCv3 message tags
    pub tags: Option<Tags>,
    /// Message prefix (source)
    pub prefix: Option<Prefix>,
    /// The command
    pub command: Command,
}

impl Message {
    /// Create a new message with just a command.
    pub fn new(command: Command) -> Self {
        Self {
            tags: None,
            prefix: None,
            command,
        }
    }

    /// Create a new message with a prefix.
    pub fn with_prefix(prefix: Prefix, command: Command) -> Self {
        Self {
            tags: None,
            prefix: Some(prefix),
            command,
        }
    }

    /// Add tags to the message.
    pub fn with_tags(mut self, tags: Tags) -> Self {
        self.tags = Some(tags);
        self
    }

    /// Parse a message from bytes.
    pub fn parse(input: &[u8]) -> Result<Self, ParseError> {
        // Check maximum length (use IRCv3 limit which allows message-tags)
        if input.len() > crate::MAX_MESSAGE_LEN_IRCV3 {
            return Err(ParseError::MessageTooLong(input.len()));
        }

        // Convert to string (IRC is technically Latin-1, but we use UTF-8)
        let input = std::str::from_utf8(input).map_err(|_| ParseError::InvalidUtf8)?;

        // Strip CRLF if present
        let input = input.trim_end_matches("\r\n").trim_end_matches('\n');

        if input.is_empty() {
            return Err(ParseError::EmptyMessage);
        }

        Self::parse_str(input)
    }

    /// Parse a message from a string (without CRLF).
    pub fn parse_str(input: &str) -> Result<Self, ParseError> {
        let mut remaining = input;

        // Parse tags (optional, starts with @)
        let tags = if remaining.starts_with('@') {
            let space_idx = remaining.find(' ').ok_or(ParseError::EmptyCommand)?;
            let tags_str = &remaining[1..space_idx];
            remaining = remaining[space_idx..].trim_start();
            Some(Tags::parse(tags_str)?)
        } else {
            None
        };

        // Parse prefix (optional, starts with :)
        let prefix = if remaining.starts_with(':') {
            let space_idx = remaining.find(' ').ok_or(ParseError::EmptyCommand)?;
            let prefix_str = &remaining[1..space_idx];
            remaining = remaining[space_idx..].trim_start();
            Some(Prefix::parse(prefix_str)?)
        } else {
            None
        };

        // Parse command and parameters
        let command = parse_command(remaining)?;

        Ok(Self {
            tags,
            prefix,
            command,
        })
    }

    /// Serialize the message to bytes (with CRLF).
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut bytes = self.to_string().into_bytes();
        bytes.extend_from_slice(b"\r\n");
        bytes
    }

    /// Get the command.
    pub fn command(&self) -> &Command {
        &self.command
    }

    /// Get the source nickname if available.
    pub fn source_nick(&self) -> Option<&str> {
        self.prefix.as_ref().and_then(|p| p.nick())
    }
}

impl fmt::Display for Message {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if let Some(ref tags) = self.tags && !tags.is_empty() {
            write!(f, "@{} ", tags)?;
        }
        if let Some(ref prefix) = self.prefix {
            write!(f, ":{} ", prefix)?;
        }
        write!(f, "{}", self.command)
    }
}

/// Parse the command and parameters portion of a message.
fn parse_command(input: &str) -> Result<Command, ParseError> {
    if input.is_empty() {
        return Err(ParseError::EmptyCommand);
    }

    // Split into command and params
    let (command_str, params_str) = match input.find(' ') {
        Some(idx) => (&input[..idx], Some(&input[idx + 1..])),
        None => (input, None),
    };

    // Parse parameters
    let params = parse_params(params_str.unwrap_or(""));

    // Check if numeric
    if command_str.len() == 3 && command_str.chars().all(|c| c.is_ascii_digit()) {
        let code: u16 = command_str
            .parse()
            .map_err(|_| ParseError::InvalidNumeric(command_str.to_string()))?;

        let target = params.first().cloned().unwrap_or_default();
        let rest = params.into_iter().skip(1).collect();

        return Ok(Command::Numeric {
            code,
            target,
            params: rest,
        });
    }

    // Parse by command name
    let command = command_str.to_uppercase();
    parse_command_with_params(&command, params)
}

/// Parse message parameters.
///
/// Parameters are space-separated, with the trailing parameter
/// prefixed by `:` to allow spaces.
fn parse_params(input: &str) -> Vec<String> {
    if input.is_empty() {
        return Vec::new();
    }

    let mut params = Vec::new();
    let mut remaining = input;

    while !remaining.is_empty() {
        if let Some(trailing) = remaining.strip_prefix(':') {
            // Trailing parameter (rest of the line)
            params.push(trailing.to_string());
            break;
        }

        match remaining.find(' ') {
            Some(idx) => {
                let param = &remaining[..idx];
                if !param.is_empty() {
                    params.push(param.to_string());
                }
                remaining = &remaining[idx + 1..];
            }
            None => {
                if !remaining.is_empty() {
                    params.push(remaining.to_string());
                }
                break;
            }
        }
    }

    params
}

/// Parse a specific command with its parameters.
fn parse_command_with_params(command: &str, params: Vec<String>) -> Result<Command, ParseError> {
    match command {
        "PASS" => Ok(Command::Pass {
            password: params.into_iter().next().unwrap_or_default(),
        }),

        "NICK" => Ok(Command::Nick {
            nickname: params.into_iter().next().unwrap_or_default(),
        }),

        "USER" => {
            let mut iter = params.into_iter();
            Ok(Command::User {
                username: iter.next().unwrap_or_default(),
                mode: iter.next().and_then(|s| s.parse().ok()).unwrap_or(0),
                realname: iter.nth(1).unwrap_or_default(), // Skip the unused parameter
            })
        }

        "OPER" => {
            let mut iter = params.into_iter();
            Ok(Command::Oper {
                name: iter.next().unwrap_or_default(),
                password: iter.next().unwrap_or_default(),
            })
        }

        "QUIT" => Ok(Command::Quit {
            message: params.into_iter().next(),
        }),

        "JOIN" => {
            let mut iter = params.into_iter();
            let channels_str = iter.next().unwrap_or_default();
            let keys_str = iter.next();

            let channel_names: Vec<_> = channels_str.split(',').map(String::from).collect();
            let keys: Vec<String> = keys_str
                .map(|k| k.split(',').map(String::from).collect())
                .unwrap_or_default();

            let channels = channel_names
                .into_iter()
                .enumerate()
                .map(|(i, c)| {
                    let key = keys.get(i).cloned();
                    (c, key)
                })
                .collect();

            Ok(Command::Join { channels })
        }

        "PART" => {
            let mut iter = params.into_iter();
            let channels_str = iter.next().unwrap_or_default();
            let message = iter.next();

            let channels = channels_str.split(',').map(String::from).collect();

            Ok(Command::Part { channels, message })
        }

        "MODE" => {
            let mut iter = params.into_iter();
            let target = iter.next().unwrap_or_default();
            let modes = iter.next();
            let mode_params: Vec<_> = iter.collect();

            Ok(Command::Mode {
                target,
                modes,
                params: mode_params,
            })
        }

        "TOPIC" => {
            let mut iter = params.into_iter();
            Ok(Command::Topic {
                channel: iter.next().unwrap_or_default(),
                topic: iter.next(),
            })
        }

        "NAMES" => {
            let channels = params.first().map(|s| s.split(',').map(String::from).collect());
            Ok(Command::Names { channels })
        }

        "LIST" => {
            let channels = params.first().map(|s| s.split(',').map(String::from).collect());
            Ok(Command::List { channels })
        }

        "INVITE" => {
            let mut iter = params.into_iter();
            Ok(Command::Invite {
                nickname: iter.next().unwrap_or_default(),
                channel: iter.next().unwrap_or_default(),
            })
        }

        "KICK" => {
            let mut iter = params.into_iter();
            let channel = iter.next().unwrap_or_default();
            let users_str = iter.next().unwrap_or_default();
            let comment = iter.next();

            let users = users_str.split(',').map(String::from).collect();

            Ok(Command::Kick {
                channel,
                users,
                comment,
            })
        }

        "PRIVMSG" => {
            let mut iter = params.into_iter();
            Ok(Command::Privmsg {
                target: iter.next().unwrap_or_default(),
                message: iter.next().unwrap_or_default(),
            })
        }

        "NOTICE" => {
            let mut iter = params.into_iter();
            Ok(Command::Notice {
                target: iter.next().unwrap_or_default(),
                message: iter.next().unwrap_or_default(),
            })
        }

        "TAGMSG" => Ok(Command::Tagmsg {
            target: params.into_iter().next().unwrap_or_default(),
        }),

        "PING" => {
            let mut iter = params.into_iter();
            Ok(Command::Ping {
                server1: iter.next().unwrap_or_default(),
                server2: iter.next(),
            })
        }

        "PONG" => {
            let mut iter = params.into_iter();
            Ok(Command::Pong {
                server1: iter.next().unwrap_or_default(),
                server2: iter.next(),
            })
        }

        "AWAY" => Ok(Command::Away {
            message: params.into_iter().next(),
        }),

        "MOTD" => Ok(Command::Motd {
            server: params.into_iter().next(),
        }),

        "VERSION" => Ok(Command::Version {
            server: params.into_iter().next(),
        }),

        "TIME" => Ok(Command::Time {
            server: params.into_iter().next(),
        }),

        "ADMIN" => Ok(Command::Admin {
            server: params.into_iter().next(),
        }),

        "INFO" => Ok(Command::Info {
            server: params.into_iter().next(),
        }),

        "WHO" => {
            let mut iter = params.into_iter();
            let mask = iter.next().unwrap_or_else(|| "*".to_string());
            let operators_only = iter.next().map(|s| s == "o").unwrap_or(false);
            Ok(Command::Who {
                mask,
                operators_only,
            })
        }

        "WHOIS" => {
            let mut iter = params.into_iter();
            let first = iter.next().unwrap_or_default();
            let second = iter.next();

            let (server, nicks_str) = if let Some(s) = second {
                (Some(first), s)
            } else {
                (None, first)
            };

            let nicknames = nicks_str.split(',').map(String::from).collect();

            Ok(Command::Whois { server, nicknames })
        }

        "WHOWAS" => {
            let mut iter = params.into_iter();
            Ok(Command::Whowas {
                nickname: iter.next().unwrap_or_default(),
                count: iter.next().and_then(|s| s.parse().ok()),
                server: iter.next(),
            })
        }

        "USERHOST" => Ok(Command::Userhost {
            nicknames: params,
        }),

        "ISON" => Ok(Command::Ison { nicknames: params }),

        "KILL" => {
            let mut iter = params.into_iter();
            Ok(Command::Kill {
                nickname: iter.next().unwrap_or_default(),
                comment: iter.next().unwrap_or_default(),
            })
        }

        "WALLOPS" => Ok(Command::Wallops {
            message: params.into_iter().next().unwrap_or_default(),
        }),

        "REHASH" => Ok(Command::Rehash),

        "RESTART" => Ok(Command::Restart),

        "DIE" => Ok(Command::Die),

        "KLINE" => {
            // KLINE [duration] mask [:reason]
            let mut iter = params.into_iter().peekable();
            let first = iter.next().unwrap_or_default();

            // Check if first param looks like a duration (e.g., "1d", "2h", "30m")
            let (duration, mask) = if first.chars().all(|c| c.is_ascii_digit())
                || first.ends_with('d')
                || first.ends_with('h')
                || first.ends_with('m')
                || first.ends_with('s')
            {
                let mask = iter.next().unwrap_or_default();
                (Some(first), mask)
            } else {
                (None, first)
            };
            let reason = iter.next();

            Ok(Command::Kline { duration, mask, reason })
        }

        "UNKLINE" => Ok(Command::Unkline {
            mask: params.into_iter().next().unwrap_or_default(),
        }),

        "GLINE" => {
            let mut iter = params.into_iter().peekable();
            let first = iter.next().unwrap_or_default();

            let (duration, mask) = if first.chars().all(|c| c.is_ascii_digit())
                || first.ends_with('d')
                || first.ends_with('h')
                || first.ends_with('m')
                || first.ends_with('s')
            {
                let mask = iter.next().unwrap_or_default();
                (Some(first), mask)
            } else {
                (None, first)
            };
            let reason = iter.next();

            Ok(Command::Gline { duration, mask, reason })
        }

        "UNGLINE" => Ok(Command::Ungline {
            mask: params.into_iter().next().unwrap_or_default(),
        }),

        "ZLINE" => {
            let mut iter = params.into_iter().peekable();
            let first = iter.next().unwrap_or_default();

            let (duration, mask) = if first.chars().all(|c| c.is_ascii_digit())
                || first.ends_with('d')
                || first.ends_with('h')
                || first.ends_with('m')
                || first.ends_with('s')
            {
                let mask = iter.next().unwrap_or_default();
                (Some(first), mask)
            } else {
                (None, first)
            };
            let reason = iter.next();

            Ok(Command::Zline { duration, mask, reason })
        }

        "UNZLINE" => Ok(Command::Unzline {
            mask: params.into_iter().next().unwrap_or_default(),
        }),

        "HELP" => Ok(Command::Help {
            topic: params.into_iter().next(),
        }),

        "CAP" => {
            let mut iter = params.into_iter();
            Ok(Command::Cap {
                subcommand: iter.next().unwrap_or_default(),
                params: iter.collect(),
            })
        }

        "AUTHENTICATE" => Ok(Command::Authenticate {
            data: params.into_iter().next().unwrap_or_default(),
        }),

        "BATCH" => {
            let mut iter = params.into_iter();
            let reference = iter.next().unwrap_or_default();
            let batch_type = iter.next();
            let rest: Vec<_> = iter.collect();

            Ok(Command::Batch {
                reference,
                batch_type,
                params: rest,
            })
        }

        "ACCOUNT" => Ok(Command::Account {
            account: params.into_iter().next().unwrap_or_else(|| "*".to_string()),
        }),

        "CHGHOST" => {
            let mut iter = params.into_iter();
            Ok(Command::Chghost {
                user: iter.next().unwrap_or_default(),
                host: iter.next().unwrap_or_default(),
            })
        }

        "SETNAME" => Ok(Command::Setname {
            realname: params.into_iter().next().unwrap_or_default(),
        }),

        "CHATHISTORY" => {
            let mut iter = params.into_iter();
            Ok(Command::Chathistory {
                subcommand: iter.next().unwrap_or_default(),
                target: iter.next().unwrap_or_default(),
                params: iter.collect(),
            })
        }

        "MONITOR" => {
            let mut iter = params.into_iter();
            let subcommand = iter
                .next()
                .and_then(|s| s.chars().next())
                .unwrap_or('+');
            let targets = iter.next();
            Ok(Command::Monitor {
                subcommand,
                targets,
            })
        }

        "MARKREAD" => {
            let mut iter = params.into_iter();
            Ok(Command::Markread {
                target: iter.next().unwrap_or_default(),
                timestamp: iter.next(),
            })
        }

        "REGISTER" => {
            // REGISTER <account> <email> <password>
            let mut iter = params.into_iter();
            Ok(Command::Register {
                account: iter.next().unwrap_or_else(|| "*".to_string()),
                email: iter.next().unwrap_or_else(|| "*".to_string()),
                password: iter.next().unwrap_or_default(),
            })
        }

        "LUSERS" => {
            let mut iter = params.into_iter();
            Ok(Command::Lusers {
                mask: iter.next(),
                server: iter.next(),
            })
        }

        "STATS" => {
            let mut iter = params.into_iter();
            Ok(Command::Stats {
                query: iter.next().and_then(|s| s.chars().next()),
                server: iter.next(),
            })
        }

        _ => Ok(Command::Unknown {
            command: command.to_string(),
            params,
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_simple() {
        let msg = Message::parse(b"PING :server\r\n").unwrap();
        assert!(matches!(msg.command, Command::Ping { .. }));
    }

    #[test]
    fn test_parse_with_prefix() {
        let msg = Message::parse(b":nick!user@host PRIVMSG #channel :Hello world\r\n").unwrap();
        assert_eq!(msg.source_nick(), Some("nick"));
        assert!(matches!(msg.command, Command::Privmsg { .. }));
    }

    #[test]
    fn test_parse_with_tags() {
        let msg =
            Message::parse(b"@time=2024-01-15T14:32:00.000Z :nick PRIVMSG #chan :Hi\r\n").unwrap();
        assert!(msg.tags.is_some());
        assert_eq!(msg.tags.as_ref().unwrap().time(), Some("2024-01-15T14:32:00.000Z"));
    }

    #[test]
    fn test_parse_numeric() {
        let msg = Message::parse(b":server 001 nick :Welcome\r\n").unwrap();
        assert!(matches!(msg.command, Command::Numeric { code: 1, .. }));
    }

    #[test]
    fn test_roundtrip() {
        let original = ":nick!user@host PRIVMSG #channel :Hello world";
        let msg = Message::parse_str(original).unwrap();
        assert_eq!(msg.to_string(), original);
    }

    #[test]
    fn test_join_with_keys() {
        let msg = Message::parse(b"JOIN #chan1,#chan2 key1,key2\r\n").unwrap();
        if let Command::Join { channels } = msg.command {
            assert_eq!(channels.len(), 2);
            assert_eq!(channels[0], ("#chan1".to_string(), Some("key1".to_string())));
            assert_eq!(channels[1], ("#chan2".to_string(), Some("key2".to_string())));
        } else {
            panic!("Expected JOIN command");
        }
    }

    #[test]
    fn test_cap() {
        let msg = Message::parse(b"CAP LS 302\r\n").unwrap();
        if let Command::Cap { subcommand, params } = msg.command {
            assert_eq!(subcommand, "LS");
            assert_eq!(params, vec!["302"]);
        } else {
            panic!("Expected CAP command");
        }
    }
}
