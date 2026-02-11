# irc-proto

The core protocol library shared by all other crates. Handles parsing, serialization, and type definitions for the IRC protocol.

## Responsibilities

- Parse raw IRC messages into structured types
- Serialize structured types back to wire format
- Define all IRC commands, replies, and error codes
- Define channel and user mode types
- Provide validation for nicknames, channels, hostmasks

## Public API

### Core Types

```rust
/// A parsed IRC message
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Message {
    /// Optional message tags (IRCv3)
    pub tags: Option<Tags>,

    /// Optional prefix (source of message)
    pub prefix: Option<Prefix>,

    /// The command or numeric reply
    pub command: Command,
}

/// Message prefix indicating the source
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Prefix {
    /// Server name
    Server(String),

    /// User with nick, optional user, optional host
    User {
        nick: String,
        user: Option<String>,
        host: Option<String>,
    },
}

/// All IRC commands (client and server)
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Command {
    // Connection Registration
    Pass { password: String },
    Nick { nickname: String },
    User { username: String, mode: u8, realname: String }, // mode bitmask: 0 = none, 8 = +i (invisible)
    Oper { name: String, password: String },
    Quit { message: Option<String> },

    // Channel Operations
    Join { channels: Vec<(String, Option<String>)> }, // (channel, key)
    Part { channels: Vec<String>, message: Option<String> },
    Mode { target: String, changes: Option<ModeChanges> },
    Topic { channel: String, topic: Option<String> },
    Names { channels: Option<Vec<String>> },
    List { channels: Option<Vec<String>> },
    Invite { nickname: String, channel: String },
    Kick { channel: String, users: Vec<String>, comment: Option<String> },

    // Messaging
    Privmsg { targets: Vec<String>, message: String },
    Notice { targets: Vec<String>, message: String },

    // Server Queries
    Motd { server: Option<String> },
    Lusers { mask: Option<String>, server: Option<String> },
    Version { server: Option<String> },
    Stats { query: Option<char>, server: Option<String> },
    Links { mask: Option<String>, server: Option<String> },
    Time { server: Option<String> },
    Admin { server: Option<String> },
    Info { server: Option<String> },

    // User Queries
    Who { mask: String, operators_only: bool },
    Whois { server: Option<String>, nicknames: Vec<String> },
    Whowas { nickname: String, count: Option<u32>, server: Option<String> },

    // Miscellaneous
    Ping { server1: String, server2: Option<String> },
    Pong { server1: String, server2: Option<String> },
    Away { message: Option<String> },
    Userhost { nicknames: Vec<String> },
    Ison { nicknames: Vec<String> },

    // Operator Commands
    Kill { nickname: String, comment: String },
    Wallops { message: String },

    // Numeric Reply (server -> client)
    Numeric { code: u16, params: Vec<String> },

    // Unknown/Raw command
    Unknown { command: String, params: Vec<String> },
}
```

### Mode Types

```rust
/// Channel mode flags
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChannelMode {
    // Member status
    Operator,      // o - channel operator
    Voice,         // v - voice privilege

    // Channel flags
    InviteOnly,    // i
    Moderated,     // m
    NoExternal,    // n - no external messages
    TopicLock,     // t - only ops can change topic
    Secret,        // s
    Private,       // p
    Key,           // k - requires key to join
    Limit,         // l - user limit

    // Masks
    Ban,           // b
    Exception,     // e
    InviteException, // I
}

/// User mode flags
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UserMode {
    Invisible,     // i
    ServerNotices, // s
    Wallops,       // w
    Operator,      // o
}

/// A single mode change (adding or removing)
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ModeChange {
    /// Channel mode change
    Channel { adding: bool, mode: ChannelMode, param: Option<String> },
    /// User mode change
    User { adding: bool, mode: UserMode },
}

/// A set of mode changes parsed from a MODE command
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModeChanges {
    pub changes: Vec<ModeChange>,
}
```

### Numeric Replies

```rust
/// Numeric reply codes (RFC 2812 Section 5)
pub mod reply {
    // Connection registration
    pub const RPL_WELCOME: u16 = 001;
    pub const RPL_YOURHOST: u16 = 002;
    pub const RPL_CREATED: u16 = 003;
    pub const RPL_MYINFO: u16 = 004;
    pub const RPL_ISUPPORT: u16 = 005;

    // Command responses
    pub const RPL_UMODEIS: u16 = 221;
    pub const RPL_LUSERCLIENT: u16 = 251;
    pub const RPL_LUSEROP: u16 = 252;
    pub const RPL_LUSERUNKNOWN: u16 = 253;
    pub const RPL_LUSERCHANNELS: u16 = 254;
    pub const RPL_LUSERME: u16 = 255;

    // AWAY
    pub const RPL_AWAY: u16 = 301;
    pub const RPL_UNAWAY: u16 = 305;
    pub const RPL_NOWAWAY: u16 = 306;

    // WHOIS
    pub const RPL_WHOISUSER: u16 = 311;
    pub const RPL_WHOISSERVER: u16 = 312;
    pub const RPL_WHOISOPERATOR: u16 = 313;
    pub const RPL_WHOISIDLE: u16 = 317;
    pub const RPL_ENDOFWHOIS: u16 = 318;
    pub const RPL_WHOISCHANNELS: u16 = 319;

    // LIST
    pub const RPL_LISTSTART: u16 = 321;
    pub const RPL_LIST: u16 = 322;
    pub const RPL_LISTEND: u16 = 323;

    // TOPIC
    pub const RPL_NOTOPIC: u16 = 331;
    pub const RPL_TOPIC: u16 = 332;
    pub const RPL_TOPICWHOTIME: u16 = 333;

    // INVITE
    pub const RPL_INVITING: u16 = 341;

    // WHO
    pub const RPL_WHOREPLY: u16 = 352;
    pub const RPL_ENDOFWHO: u16 = 315;

    // NAMES
    pub const RPL_NAMREPLY: u16 = 353;
    pub const RPL_ENDOFNAMES: u16 = 366;

    // MOTD
    pub const RPL_MOTDSTART: u16 = 375;
    pub const RPL_MOTD: u16 = 372;
    pub const RPL_ENDOFMOTD: u16 = 376;

    // ... (complete list in implementation)
}

pub mod error {
    pub const ERR_NOSUCHNICK: u16 = 401;
    pub const ERR_NOSUCHSERVER: u16 = 402;
    pub const ERR_NOSUCHCHANNEL: u16 = 403;
    pub const ERR_CANNOTSENDTOCHAN: u16 = 404;
    pub const ERR_TOOMANYCHANNELS: u16 = 405;
    pub const ERR_TOOMANYTARGETS: u16 = 407;
    pub const ERR_NOORIGIN: u16 = 409;
    pub const ERR_NORECIPIENT: u16 = 411;
    pub const ERR_NOTEXTTOSEND: u16 = 412;
    pub const ERR_UNKNOWNCOMMAND: u16 = 421;
    pub const ERR_NOMOTD: u16 = 422;
    pub const ERR_NONICKNAMEGIVEN: u16 = 431;
    pub const ERR_ERRONEUSNICKNAME: u16 = 432;
    pub const ERR_NICKNAMEINUSE: u16 = 433;
    pub const ERR_NICKCOLLISION: u16 = 436;
    pub const ERR_USERNOTINCHANNEL: u16 = 441;
    pub const ERR_NOTONCHANNEL: u16 = 442;
    pub const ERR_USERONCHANNEL: u16 = 443;
    pub const ERR_NOTREGISTERED: u16 = 451;
    pub const ERR_NEEDMOREPARAMS: u16 = 461;
    pub const ERR_ALREADYREGISTERED: u16 = 462;
    pub const ERR_PASSWDMISMATCH: u16 = 464;
    pub const ERR_CHANNELISFULL: u16 = 471;
    pub const ERR_UNKNOWNMODE: u16 = 472;
    pub const ERR_INVITEONLYCHAN: u16 = 473;
    pub const ERR_BANNEDFROMCHAN: u16 = 474;
    pub const ERR_BADCHANNELKEY: u16 = 475;
    pub const ERR_NOPRIVILEGES: u16 = 481;
    pub const ERR_CHANOPRIVSNEEDED: u16 = 482;

    // ... (complete list in implementation)
}
```

### Validation Functions

```rust
/// Validates an IRC nickname
/// - Max 9 characters (configurable via ISUPPORT)
/// - Must start with letter or special char
/// - Allowed: a-z, A-Z, 0-9, []\`^{|}-_
pub fn validate_nickname(nick: &str) -> Result<(), ValidationError>;

/// Validates a channel name
/// - Max 50 characters (configurable)
/// - Must start with #, &, +, or !
/// - Cannot contain space, comma, or ^G
pub fn validate_channel(channel: &str) -> Result<(), ValidationError>;

/// Validates and parses a hostmask (nick!user@host)
pub fn parse_hostmask(mask: &str) -> Result<Hostmask, ValidationError>;

/// Checks if a hostmask matches a user
pub fn hostmask_matches(mask: &Hostmask, user: &UserInfo) -> bool;
```

### Parsing API

```rust
/// Parse a single IRC message from bytes
pub fn parse_message(input: &[u8]) -> Result<Message, ParseError>;

/// Parse a single IRC message from a string
pub fn parse_message_str(input: &str) -> Result<Message, ParseError>;

/// Streaming parser for use with tokio codecs
pub struct MessageCodec {
    max_length: usize,
}

impl Decoder for MessageCodec {
    type Item = Message;
    type Error = ParseError;

    fn decode(&mut self, src: &mut BytesMut) -> Result<Option<Message>, ParseError>;
}

impl Encoder<Message> for MessageCodec {
    type Error = std::io::Error;

    fn encode(&mut self, msg: Message, dst: &mut BytesMut) -> Result<(), Self::Error>;
}
```

### Serialization

```rust
impl Message {
    /// Serialize to IRC wire format
    pub fn to_bytes(&self) -> Vec<u8>;

    /// Serialize to string (without CRLF)
    pub fn to_string(&self) -> String;
}

impl std::fmt::Display for Message {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result;
}
```

## Error Types

```rust
#[derive(Debug, thiserror::Error)]
pub enum ParseError {
    #[error("message too long (max 512 bytes)")]
    MessageTooLong,

    #[error("message missing CRLF terminator")]
    MissingTerminator,

    #[error("invalid prefix format")]
    InvalidPrefix,

    #[error("empty command")]
    EmptyCommand,

    #[error("invalid numeric code: {0}")]
    InvalidNumeric(String),

    #[error("invalid UTF-8 in message")]
    InvalidUtf8,
}

#[derive(Debug, thiserror::Error)]
pub enum ValidationError {
    #[error("nickname too long (max {max} chars)")]
    NicknameTooLong { max: usize },

    #[error("nickname contains invalid character: {0}")]
    InvalidNicknameChar(char),

    #[error("channel name too long (max {max} chars)")]
    ChannelTooLong { max: usize },

    #[error("invalid channel prefix: {0}")]
    InvalidChannelPrefix(char),

    #[error("channel contains invalid character")]
    InvalidChannelChar,
}
```

## Internal Structure

```
irc-proto/
├── Cargo.toml
└── src/
    ├── lib.rs           # Public re-exports
    ├── message.rs       # Message, Prefix types
    ├── command.rs       # Command enum and parsing
    ├── mode.rs          # ChannelMode, UserMode, ModeChanges
    ├── reply.rs         # Numeric reply constants
    ├── error.rs         # Error codes constants
    ├── parse.rs         # Parsing logic
    ├── codec.rs         # tokio_util codec impl
    ├── validate.rs      # Validation functions
    └── hostmask.rs      # Hostmask parsing/matching
```

## Dependencies

```toml
[dependencies]
thiserror = "2"
bytes = "1"
tokio-util = { version = "0.7", features = ["codec"] }

[dev-dependencies]
criterion = "0.5"
proptest = "1"
```

## Testing Strategy

1. **Unit tests** for each command parse/serialize round-trip
2. **Property tests** with proptest for fuzzing
3. **Conformance tests** against captured real IRC traffic
4. **Benchmarks** for parsing performance

## Example Usage

```rust
use irc_proto::{parse_message_str, Command, Message, Prefix};

// Parse an incoming message
let msg = parse_message_str(":nick!user@host PRIVMSG #channel :Hello, world!\r\n")?;
assert_eq!(msg.prefix, Some(Prefix::User {
    nick: "nick".into(),
    user: Some("user".into()),
    host: Some("host".into()),
}));
assert!(matches!(msg.command, Command::Privmsg { .. }));

// Construct and serialize a message
let msg = Message {
    tags: None,
    prefix: None,
    command: Command::Join {
        channels: vec![("#rust".into(), None)],
    },
};
assert_eq!(msg.to_string(), "JOIN #rust");
```

## Design Decisions

1. **IRCv3 Tags**: The `Message` struct includes `tags: Option<Tags>` to reserve the field, but tag parsing is deferred to a later phase. This avoids breaking API changes when IRCv3 support is added.

## Open Questions

1. **ISUPPORT Parsing**: Parse ISUPPORT values into typed struct?
   - Recommendation: Yes, common values like NICKLEN, CHANMODES

2. **Case Mapping**: Handle IRC case mapping (rfc1459 vs ascii)?
   - Recommendation: Provide utility functions, use rfc1459 by default
