# irc-client-lib

A shared library providing IRC client functionality for both CLI and GUI clients. Handles connection management, protocol handling, and session state.

## Responsibilities

- Establish and maintain TCP/TLS connections to IRC servers
- Handle protocol-level concerns (encoding, rate limiting)
- Manage session state (channels joined, users seen, modes)
- Provide an async event-driven API for UI layers
- Support multiple simultaneous server connections
- Handle reconnection logic

## Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                     irc-client-lib                          │
├─────────────────────────────────────────────────────────────┤
│                                                             │
│  ┌─────────────┐      ┌─────────────┐      ┌─────────────┐  │
│  │   Client    │      │   Client    │      │   Client    │  │
│  │  (Server 1) │      │  (Server 2) │      │  (Server N) │  │
│  └──────┬──────┘      └──────┬──────┘      └──────┬──────┘  │
│         │                    │                    │         │
│         └────────────────────┼────────────────────┘         │
│                              │                              │
│                              ▼                              │
│                    ┌───────────────────┐                    │
│                    │   Event Stream    │                    │
│                    │  (mpsc channel)   │                    │
│                    └───────────────────┘                    │
│                              │                              │
│                              ▼                              │
│                    ┌───────────────────┐                    │
│                    │   UI Layer        │                    │
│                    │  (cli or gui)     │                    │
│                    └───────────────────┘                    │
└─────────────────────────────────────────────────────────────┘
```

## Core Types

### Client Handle

```rust
/// Handle to an IRC server connection
/// Clone-friendly, can be shared across tasks
#[derive(Clone)]
pub struct Client {
    inner: Arc<ClientInner>,
}

struct ClientInner {
    /// Server identifier
    server_id: ServerId,

    /// Server configuration
    config: ServerConfig,

    /// Command sender (to connection task)
    command_tx: mpsc::Sender<ClientCommand>,

    /// Current connection state
    state: watch::Receiver<ConnectionState>,

    /// Session data (channels, users, etc.)
    session: Arc<RwLock<Session>>,
}

/// Unique identifier for a server connection
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ServerId(u64);
```

### Connection State

```rust
/// Connection lifecycle state
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConnectionState {
    /// Not connected
    Disconnected,

    /// TCP connection in progress
    Connecting,

    /// Connected, registration in progress
    Registering,

    /// Fully connected and registered
    Connected {
        nick: String,
        server_name: String,
    },

    /// Connection lost, will retry
    Reconnecting {
        attempt: u32,
        next_retry: Instant,
    },

    /// Permanently disconnected (user quit or error)
    Closed {
        reason: Option<String>,
    },
}
```

### Session State

```rust
/// Session state for a single server connection
pub struct Session {
    /// Our current nickname
    pub nick: String,

    /// Our current user modes
    pub user_modes: UserModes,

    /// Server information (from 004/005)
    pub server_info: ServerInfo,

    /// Channels we're in
    pub channels: HashMap<UniCase<String>, ChannelState>,

    /// Known users (from WHOIS, WHO, etc.)
    pub users: HashMap<UniCase<String>, UserInfo>,

    /// Our away message (if set)
    pub away: Option<String>,

    /// Server's ISUPPORT values
    pub isupport: ISupport,
}

/// State of a joined channel
pub struct ChannelState {
    /// Channel name
    pub name: String,

    /// Current topic
    pub topic: Option<Topic>,

    /// Channel modes (what we know)
    pub modes: ChannelModes,

    /// Members we know about
    pub members: HashMap<UniCase<String>, MemberInfo>,

    /// Whether we have the full member list
    pub names_complete: bool,

    /// Message history (ring buffer)
    pub messages: VecDeque<ChannelMessage>,
}

/// Information about a channel member
pub struct MemberInfo {
    pub nick: String,
    pub is_op: bool,
    pub has_voice: bool,
}

/// A message in a channel
pub struct ChannelMessage {
    pub timestamp: DateTime<Utc>,
    pub sender: String,
    pub content: MessageContent,
}

pub enum MessageContent {
    Privmsg(String),
    Notice(String),
    Action(String),  // CTCP ACTION
    Join,
    Part(Option<String>),
    Quit(Option<String>),
    Kick { by: String, reason: Option<String> },
    NickChange { old: String },
    TopicChange(Option<String>),
    ModeChange(String),
}
```

### Server Configuration

```rust
/// Configuration for connecting to an IRC server
#[derive(Debug, Clone, Default)]
pub struct ServerConfig {
    /// Display name for this connection
    pub name: String,

    /// Server hostname
    pub host: String,

    /// Server port
    pub port: u16,

    /// Use TLS
    pub tls: bool,

    /// Accept invalid TLS certificates (testing only)
    pub tls_insecure: bool,

    /// Server password (PASS command)
    pub password: Option<String>,

    /// Nicknames to try (in order)
    pub nicknames: Vec<String>,

    /// Username (USER command)
    pub username: String,

    /// Real name (USER command)
    pub realname: String,

    /// Channels to auto-join on connect
    pub autojoin: Vec<AutoJoinChannel>,

    /// Auto-reconnect settings
    pub reconnect: ReconnectConfig,
}

#[derive(Debug, Clone)]
pub struct AutoJoinChannel {
    pub name: String,
    pub key: Option<String>,
}

#[derive(Debug, Clone)]
pub struct ReconnectConfig {
    /// Enable auto-reconnect
    pub enabled: bool,

    /// Initial delay (seconds)
    pub initial_delay: u64,

    /// Maximum delay (seconds)
    pub max_delay: u64,

    /// Maximum attempts (0 = infinite)
    pub max_attempts: u32,
}
```

### Events

```rust
/// Events emitted by the client
#[derive(Debug, Clone)]
pub enum Event {
    /// Connection state changed
    ConnectionStateChanged {
        server: ServerId,
        state: ConnectionState,
    },

    /// Received a message in a channel
    ChannelMessage {
        server: ServerId,
        channel: String,
        message: ChannelMessage,
    },

    /// Received a private message
    PrivateMessage {
        server: ServerId,
        from: String,
        message: String,
        is_notice: bool,
    },

    /// Channel topic changed
    TopicChanged {
        server: ServerId,
        channel: String,
        topic: Option<Topic>,
    },

    /// We joined a channel
    ChannelJoined {
        server: ServerId,
        channel: String,
    },

    /// We left/were kicked from a channel
    ChannelLeft {
        server: ServerId,
        channel: String,
        reason: ChannelLeftReason,
    },

    /// User joined a channel we're in
    UserJoined {
        server: ServerId,
        channel: String,
        nick: String,
        user: Option<String>,
        host: Option<String>,
    },

    /// User left a channel we're in
    UserLeft {
        server: ServerId,
        channel: String,
        nick: String,
        message: Option<String>,
    },

    /// User quit (affects multiple channels)
    UserQuit {
        server: ServerId,
        nick: String,
        message: Option<String>,
        channels: Vec<String>,
    },

    /// User changed nick
    NickChanged {
        server: ServerId,
        old_nick: String,
        new_nick: String,
        channels: Vec<String>,
    },

    /// Our nick changed
    OurNickChanged {
        server: ServerId,
        new_nick: String,
    },

    /// Channel member list updated
    NamesUpdated {
        server: ServerId,
        channel: String,
    },

    /// Mode change
    ModeChanged {
        server: ServerId,
        target: String,
        changes: String,
        by: Option<String>,
    },

    /// Error from server
    ServerError {
        server: ServerId,
        code: u16,
        message: String,
    },

    /// MOTD received
    Motd {
        server: ServerId,
        lines: Vec<String>,
    },

    /// WHOIS response
    WhoisResponse {
        server: ServerId,
        info: WhoisInfo,
    },

    /// Raw message (for debugging/logging)
    RawMessage {
        server: ServerId,
        direction: Direction,
        message: String,
    },
}

#[derive(Debug, Clone)]
pub enum ChannelLeftReason {
    Part(Option<String>),
    Kicked { by: String, reason: Option<String> },
    ServerDisconnected,
}

#[derive(Debug, Clone, Copy)]
pub enum Direction {
    Incoming,
    Outgoing,
}
```

## Public API

### Client Manager

```rust
/// Manages multiple IRC connections
pub struct ClientManager {
    clients: HashMap<ServerId, Client>,
    event_rx: mpsc::Receiver<Event>,
    next_id: AtomicU64,
}

impl ClientManager {
    /// Create a new client manager
    pub fn new() -> Self;

    /// Connect to a server
    pub async fn connect(&mut self, config: ServerConfig) -> Result<ServerId, ConnectError>;

    /// Disconnect from a server
    pub async fn disconnect(&mut self, server: ServerId, message: Option<&str>);

    /// Get a client handle
    pub fn get(&self, server: ServerId) -> Option<&Client>;

    /// Get event receiver (for UI event loop).
    /// Single-consumer: only one task should poll this receiver.
    /// The UI event loop owns the receiver; other components react
    /// to state changes via `Client::watch_state()` or session snapshots.
    pub fn events(&mut self) -> &mut mpsc::Receiver<Event>;

    /// List all connections
    pub fn servers(&self) -> impl Iterator<Item = (ServerId, &Client)>;
}
```

### Client Commands

```rust
impl Client {
    // === Connection ===

    /// Get current connection state
    pub fn state(&self) -> ConnectionState;

    /// Watch connection state changes
    pub fn watch_state(&self) -> watch::Receiver<ConnectionState>;

    /// Disconnect from server
    pub async fn quit(&self, message: Option<&str>) -> Result<(), SendError>;

    // === Messaging ===

    /// Send a message to a channel or user
    pub async fn privmsg(&self, target: &str, message: &str) -> Result<(), SendError>;

    /// Send a notice
    pub async fn notice(&self, target: &str, message: &str) -> Result<(), SendError>;

    /// Send a CTCP ACTION (/me)
    pub async fn action(&self, target: &str, action: &str) -> Result<(), SendError>;

    // === Channels ===

    /// Join a channel
    pub async fn join(&self, channel: &str, key: Option<&str>) -> Result<(), SendError>;

    /// Leave a channel
    pub async fn part(&self, channel: &str, message: Option<&str>) -> Result<(), SendError>;

    /// Set channel topic
    pub async fn set_topic(&self, channel: &str, topic: &str) -> Result<(), SendError>;

    /// Kick a user
    pub async fn kick(
        &self,
        channel: &str,
        nick: &str,
        reason: Option<&str>,
    ) -> Result<(), SendError>;

    /// Invite a user
    pub async fn invite(&self, nick: &str, channel: &str) -> Result<(), SendError>;

    // === Modes ===

    /// Set channel or user mode
    pub async fn mode(&self, target: &str, modes: &str) -> Result<(), SendError>;

    // === Queries ===

    /// Query user information
    pub async fn whois(&self, nick: &str) -> Result<(), SendError>;

    /// List users matching mask
    pub async fn who(&self, mask: &str) -> Result<(), SendError>;

    /// List channels
    pub async fn list(&self, pattern: Option<&str>) -> Result<(), SendError>;

    // === User State ===

    /// Change nickname
    pub async fn set_nick(&self, new_nick: &str) -> Result<(), SendError>;

    /// Set away message
    pub async fn away(&self, message: Option<&str>) -> Result<(), SendError>;

    // === Session Access ===

    /// Get current session state (read-only snapshot)
    pub fn session(&self) -> SessionSnapshot;

    /// Get current nickname
    pub fn nick(&self) -> Option<String>;

    /// Check if we're in a channel
    pub fn is_in_channel(&self, channel: &str) -> bool;

    /// Get channel state
    pub fn channel(&self, name: &str) -> Option<ChannelSnapshot>;

    // === Raw ===

    /// Send a raw IRC command
    pub async fn raw(&self, command: &str) -> Result<(), SendError>;
}
```

### Configuration Loading

```rust
/// Load client configuration from file
pub fn load_config(path: &Path) -> Result<ClientConfig, ConfigError>;

/// Client configuration (multiple servers)
#[derive(Debug, Deserialize)]
pub struct ClientConfig {
    /// Default identity
    pub identity: IdentityConfig,

    /// Server configurations
    pub servers: Vec<ServerConfig>,

    /// UI preferences (used by cli/gui)
    pub ui: UiConfig,
}

#[derive(Debug, Deserialize)]
pub struct IdentityConfig {
    pub nicknames: Vec<String>,
    pub username: String,
    pub realname: String,
}
```

## Re-exports

`irc-client-lib` re-exports commonly needed types from `irc-proto` so that
downstream crates (`irc-cli`, `irc-gui`) don't need a direct dependency:

```rust
// lib.rs
pub use irc_proto::{Command, Message, Prefix, ChannelMode, UserMode};
```

## Internal Structure

```
irc-client-lib/
├── Cargo.toml
└── src/
    ├── lib.rs              # Public re-exports (including irc-proto types)
    ├── client.rs           # Client, ClientManager
    ├── connection.rs       # TCP/TLS connection handling
    ├── session.rs          # Session, ChannelState
    ├── event.rs            # Event types
    ├── config.rs           # Configuration types and loading
    ├── handler.rs          # Incoming message handling
    └── error.rs            # Error types
```

## Dependencies

```toml
[dependencies]
irc-proto = { path = "../irc-proto" }
tokio = { version = "1", features = ["net", "sync", "time", "rt"] }
tokio-util = { version = "0.7", features = ["codec"] }
tokio-rustls = "0.26"
rustls-pemfile = "2"
webpki-roots = "0.26"
unicase = "2"
tracing = "0.1"
serde = { version = "1", features = ["derive"] }
toml = "0.8"
chrono = { version = "0.4", features = ["serde"] }
thiserror = "2"

[dev-dependencies]
tokio-test = "0.4"
```

## Error Types

```rust
#[derive(Debug, thiserror::Error)]
pub enum ConnectError {
    #[error("failed to resolve hostname: {0}")]
    DnsError(String),

    #[error("connection failed: {0}")]
    ConnectionFailed(#[from] std::io::Error),

    #[error("TLS handshake failed: {0}")]
    TlsError(String),

    #[error("registration failed: {0}")]
    RegistrationFailed(String),

    #[error("all nicknames in use")]
    NoAvailableNick,
}

#[derive(Debug, thiserror::Error)]
pub enum SendError {
    #[error("not connected")]
    NotConnected,

    #[error("connection closed")]
    ConnectionClosed,

    #[error("send buffer full")]
    BufferFull,
}

#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    #[error("failed to read config file: {0}")]
    Io(#[from] std::io::Error),

    #[error("failed to parse config: {0}")]
    Parse(#[from] toml::de::Error),

    #[error("invalid configuration: {0}")]
    Invalid(String),
}
```

## Example Usage

```rust
use irc_client_lib::{ClientManager, ServerConfig, Event};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut manager = ClientManager::new();

    let server_id = manager.connect(ServerConfig {
        name: "libera".into(),
        host: "irc.libera.chat".into(),
        port: 6697,
        tls: true,
        nicknames: vec!["mybot".into(), "mybot_".into()],
        username: "mybot".into(),
        realname: "My IRC Bot".into(),
        autojoin: vec![AutoJoinChannel { name: "#test".into(), key: None }],
        ..Default::default()
    }).await?;

    let client = manager.get(server_id).unwrap();

    loop {
        match manager.events().recv().await {
            Some(Event::ChannelMessage { channel, message, .. }) => {
                println!("<{}> {}: {}", channel, message.sender, message.content);
            }
            Some(Event::PrivateMessage { from, message, .. }) => {
                println!("[PM from {}] {}", from, message);
                client.privmsg(&from, "Got your message!").await?;
            }
            None => break,
        }
    }

    Ok(())
}
```

## Open Questions

1. **History Storage**: In-memory only or SQLite?
   - Recommendation: In-memory with configurable limit, SQLite as future option

2. **CTCP Handling**: Auto-respond to VERSION, PING, etc.?
   - Recommendation: Yes, with configurable responses

3. **DCC Support**: Include file transfer?
   - Recommendation: Defer, complex and rarely used

4. **Scripting**: Expose event hooks for user scripts?
   - Recommendation: Defer to GUI, out of scope for base lib

5. **Certificate Pinning**: Support TOFU or explicit pins?
   - Recommendation: Support both, TOFU with warning on change
