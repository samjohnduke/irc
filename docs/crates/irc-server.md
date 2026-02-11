# irc-server

The IRC daemon (ircd) implementation. A fully-featured IRC server supporting single-server operation with optional server-to-server linking.

## Responsibilities

- Accept and manage client TCP connections
- Handle client registration (NICK/USER handshake)
- Manage channels: creation, membership, modes
- Route messages between clients
- Enforce channel and user modes
- Implement operator commands
- Provide server queries (MOTD, LUSERS, etc.)
- FILEHOST: HTTP file upload endpoint (see [06-filehost.md](../06-filehost.md))
- Optional: Server-to-server linking (RFC 2813)

## Architecture Overview

```
┌─────────────────────────────────────────────────────────────────────┐
│                            irc-server                               │
├─────────────────────────────────────────────────────────────────────┤
│  ┌─────────────┐  ┌─────────────┐  ┌────────────┐  ┌─────────────┐  │
│  │   Listener  │  │   Listener  │  │  Listener  │  │    HTTP     │  │
│  │  (TCP:6667) │  │ (TLS:6697)  │  │ (WS:8080)  │  │  (FILEHOST) │  │
│  └──────┬──────┘  └──────┬──────┘  └─────┬──────┘  └──────┬──────┘  │
│         │                │               │                │         │
│         └────────────────┼───────────────┘                │         │
│                          ▼                                ▼         │
│              ┌───────────────────────┐         ┌──────────────────┐ │
│              │   Connection Manager  │         │  Upload Handler  │ │
│              │   (per-client tasks)  │         │  + File Storage  │ │
│              └───────────┬───────────┘         └──────────────────┘ │
│                          │                                          │
│         ┌────────────────┼────────────────┐                         │
│         ▼                ▼                ▼                         │
│  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐                  │
│  │   Client    │  │   Client    │  │   Client    │                  │
│  │   Handler   │  │   Handler   │  │   Handler   │                  │
│  └──────┬──────┘  └──────┬──────┘  └──────┬──────┘                  │
│         │                │                │                         │
│         └────────────────┼────────────────┘                         │
│                          ▼                                          │
│              ┌───────────────────────┐                              │
│              │     Server State      │                              │
│              │  (channels, users)    │                              │
│              └───────────────────────┘                              │
└─────────────────────────────────────────────────────────────────────┘
```

## Core Data Structures

### Server State

```rust
/// Central server state, shared via Arc
pub struct ServerState {
    /// Server configuration
    pub config: ServerConfig,

    /// All connected clients by unique ID
    clients: DashMap<ClientId, Arc<Client>>,

    /// Nickname -> ClientId mapping (case-insensitive)
    nick_to_client: DashMap<UniCase<String>, ClientId>,

    /// All channels by name (case-insensitive).
    /// Wrapped in Arc<RwLock<_>> so channel references can be held across
    /// await points without keeping the DashMap shard locked.
    channels: DashMap<UniCase<String>, Arc<RwLock<Channel>>>,

    /// Server operators (by nickname pattern)
    opers: Vec<OperConfig>,

    /// WHOWAS history (bounded ring buffer per nick)
    whowas: RwLock<HashMap<UniCase<String>, VecDeque<WhowasEntry>>>,

    /// Server statistics
    stats: ServerStats,

    /// Message of the day
    motd: Option<Vec<String>>,
}

/// Atomic server statistics
pub struct ServerStats {
    pub connections_total: AtomicU64,
    pub connections_current: AtomicU32,
    pub messages_received: AtomicU64,
    pub messages_sent: AtomicU64,
    pub channels_current: AtomicU32,
    pub server_start: Instant,
}

/// Historical nick record for WHOWAS queries
pub struct WhowasEntry {
    pub nick: String,
    pub user: String,
    pub host: String,
    pub realname: String,
    pub seen_at: SystemTime,
}
```

### Client State

```rust
/// Unique client identifier
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ClientId(u64);

/// A connected client
pub struct Client {
    pub id: ClientId,

    /// Registration state
    pub state: RwLock<ClientState>,

    /// Channel for sending messages to this client
    pub sender: mpsc::Sender<Message>,

    /// Client's socket address
    pub addr: SocketAddr,

    /// Connection time
    pub connected_at: Instant,

    /// Last activity time (for ping timeout), stored as epoch millis
    pub last_active: AtomicU64,
}

/// Client registration and identity state
pub struct ClientState {
    /// Registration phase
    pub phase: RegistrationPhase,

    /// Nickname (after registration)
    pub nick: Option<String>,

    /// Username
    pub user: Option<String>,

    /// Real name
    pub realname: Option<String>,

    /// Hostname (resolved or cloaked)
    pub host: String,

    /// User modes
    pub modes: UserModes,

    /// Channels this client is in
    pub channels: HashSet<UniCase<String>>,

    /// Away message (if set)
    pub away: Option<String>,

    /// Is IRC operator
    pub is_oper: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RegistrationPhase {
    /// Just connected, no commands received
    New,
    /// Received NICK but not USER
    NickReceived,
    /// Received USER but not NICK
    UserReceived,
    /// Fully registered
    Registered,
}
```

### Channel State

```rust
/// An IRC channel
pub struct Channel {
    /// Channel name (with prefix)
    pub name: String,

    /// Channel topic
    pub topic: Option<Topic>,

    /// Channel modes
    pub modes: ChannelModes,

    /// Members: ClientId -> membership info
    pub members: HashMap<ClientId, Membership>,

    /// Ban list
    pub bans: Vec<BanEntry>,

    /// Ban exceptions
    pub exceptions: Vec<MaskEntry>,

    /// Invite exceptions
    pub invites: Vec<MaskEntry>,

    /// Pending invitations (for +i channels)
    pub invited: HashSet<UniCase<String>>,

    /// Creation timestamp
    pub created_at: SystemTime,
}

pub struct Topic {
    pub text: String,
    pub set_by: String,
    pub set_at: SystemTime,
}

pub struct Membership {
    pub joined_at: Instant,
    pub is_op: bool,
    pub has_voice: bool,
}

pub struct ChannelModes {
    pub invite_only: bool,      // +i
    pub moderated: bool,        // +m
    pub no_external: bool,      // +n
    pub topic_lock: bool,       // +t
    pub secret: bool,           // +s
    pub private: bool,          // +p
    pub key: Option<String>,    // +k
    pub limit: Option<u32>,     // +l
}

pub struct BanEntry {
    pub mask: Hostmask,
    pub set_by: String,
    pub set_at: SystemTime,
}
```

## Command Handlers

Each IRC command has a dedicated handler function:

```rust
/// Handler result type
type HandlerResult = Result<(), CommandError>;

/// Command handler trait
#[async_trait]
trait CommandHandler {
    async fn handle(
        &self,
        state: &ServerState,
        client: &Client,
        command: &Command,
    ) -> HandlerResult;
}

// Example handlers module structure
mod handlers {
    mod registration;  // PASS, NICK, USER, QUIT
    mod channel;       // JOIN, PART, TOPIC, NAMES, LIST, KICK, INVITE
    mod messaging;     // PRIVMSG, NOTICE
    mod modes;         // MODE (user and channel)
    mod queries;       // WHO, WHOIS, WHOWAS
    mod server;        // MOTD, LUSERS, VERSION, TIME, ADMIN, INFO
    mod oper;          // OPER, KILL, WALLOPS
    mod misc;          // PING, PONG, AWAY, USERHOST, ISON
}
```

## Message Routing

```rust
impl ServerState {
    /// Send a message to a single client
    pub async fn send_to_client(&self, client_id: ClientId, msg: Message);

    /// Send to all members of a channel
    pub async fn send_to_channel(
        &self,
        channel: &str,
        msg: Message,
        exclude: Option<ClientId>,
    );

    /// Send to all members of a channel with a specific prefix
    pub async fn send_to_channel_ops(
        &self,
        channel: &str,
        msg: Message,
    );

    /// Send to all clients with +w mode (WALLOPS)
    pub async fn send_wallops(&self, msg: Message);

    /// Send to all connected clients (rare, e.g., server shutdown)
    pub async fn broadcast(&self, msg: Message);
}
```

## Connection Lifecycle

```
┌─────────┐   TCP Connect   ┌─────────┐   NICK+USER   ┌────────────┐
│   New   │ ───────────────▶│Handshake│ ─────────────▶│ Registered │
└─────────┘                 └─────────┘               └────────────┘
                                 │                          │
                            timeout/error                 QUIT
                                 │                          │
                                 ▼                          ▼
                           ┌───────────┐            ┌───────────┐
                           │Disconnected│◀───────────│Disconnected│
                           └───────────┘            └───────────┘
```

### Registration Flow

1. Client connects, server optionally sends `NOTICE AUTH`
2. Client sends `PASS` (optional)
3. Client sends `NICK` and `USER` (either order)
4. Server validates nickname availability
5. Server sends welcome burst:
   - `001 RPL_WELCOME`
   - `002 RPL_YOURHOST`
   - `003 RPL_CREATED`
   - `004 RPL_MYINFO`
   - `005 RPL_ISUPPORT` (one or more)
   - `251-255` LUSERS info
   - `375/372/376` MOTD
6. Client is now registered and can send commands

## Configuration

```rust
/// Server configuration
#[derive(Debug, Deserialize)]
pub struct ServerConfig {
    /// Server name (e.g., "irc.example.com")
    pub server_name: String,

    /// Network name (e.g., "ExampleNet")
    pub network_name: String,

    /// Server description
    pub description: String,

    /// Admin info
    pub admin: AdminConfig,

    /// Listen addresses
    pub listen: Vec<ListenConfig>,

    /// Connection limits
    pub limits: LimitsConfig,

    /// Operators
    pub opers: Vec<OperConfig>,

    /// MOTD file path
    pub motd_file: Option<PathBuf>,

    /// Logging configuration
    pub logging: LoggingConfig,
}

#[derive(Debug, Deserialize)]
pub struct ListenConfig {
    pub address: SocketAddr,
    pub tls: Option<TlsConfig>,
    /// Enable WebSocket transport on this listener (for browser clients)
    pub websocket: Option<bool>,
}

#[derive(Debug, Deserialize)]
pub struct TlsConfig {
    pub cert_file: PathBuf,
    pub key_file: PathBuf,
}

#[derive(Debug, Deserialize)]
pub struct LimitsConfig {
    /// Max clients
    pub max_clients: u32,

    /// Max channels per client
    pub max_channels_per_client: u32,

    /// Max clients per channel
    pub max_clients_per_channel: u32,

    /// Registration timeout (seconds)
    pub registration_timeout: u64,

    /// Ping timeout (seconds)
    pub ping_timeout: u64,

    /// Ping frequency (seconds)
    pub ping_frequency: u64,

    /// Messages per second (rate limit)
    pub messages_per_second: f32,
}

#[derive(Debug, Deserialize)]
pub struct OperConfig {
    pub name: String,
    pub password_hash: String,  // argon2
    pub host_mask: Option<String>,
}
```

### Example Configuration File

```toml
# server.toml
server_name = "irc.local"
network_name = "LocalNet"
description = "A local IRC server"
motd_file = "/etc/ircd/motd.txt"

[admin]
name = "Admin"
email = "admin@example.com"

[[listen]]
address = "0.0.0.0:6667"

[[listen]]
address = "0.0.0.0:6697"
[listen.tls]
cert_file = "/etc/ircd/cert.pem"
key_file = "/etc/ircd/key.pem"

[[listen]]
address = "0.0.0.0:8080"
websocket = true

[limits]
max_clients = 10000
max_channels_per_client = 20
max_clients_per_channel = 1000
registration_timeout = 60
ping_timeout = 180
ping_frequency = 60
messages_per_second = 2.0

[[opers]]
name = "admin"
password_hash = "$argon2id$..."
host_mask = "*@localhost"
```

## Internal Structure

```
irc-server/
├── Cargo.toml
└── src/
    ├── main.rs              # Entry point, CLI
    ├── server.rs            # Server struct, startup
    ├── config.rs            # Configuration parsing
    ├── state.rs             # ServerState
    ├── client.rs            # Client, ClientState
    ├── channel.rs           # Channel
    ├── connection.rs        # Connection handling, codec
    ├── routing.rs           # Message routing
    ├── handlers/
    │   ├── mod.rs
    │   ├── registration.rs
    │   ├── channel.rs
    │   ├── messaging.rs
    │   ├── modes.rs
    │   ├── queries.rs
    │   ├── server.rs
    │   ├── oper.rs
    │   └── misc.rs
    ├── modes.rs             # Mode handling logic
    └── ratelimit.rs         # Rate limiting
```

## Dependencies

```toml
[dependencies]
irc-proto = { path = "../irc-proto" }
tokio = { version = "1", features = ["full"] }
tokio-util = { version = "0.7", features = ["codec"] }
tokio-rustls = "0.26"
tokio-tungstenite = "0.24"
rustls-pemfile = "2"
dashmap = "6"
unicase = "2"
tracing = "0.1"
tracing-subscriber = "0.3"
serde = { version = "1", features = ["derive"] }
toml = "0.8"
argon2 = "0.5"
thiserror = "2"
clap = { version = "4", features = ["derive"] }

[dev-dependencies]
tokio-test = "0.4"
```

## Error Handling

```rust
#[derive(Debug, thiserror::Error)]
pub enum ServerError {
    #[error("failed to bind to {addr}: {source}")]
    Bind { addr: SocketAddr, source: std::io::Error },

    #[error("TLS configuration error: {0}")]
    Tls(String),

    #[error("configuration error: {0}")]
    Config(String),
}

#[derive(Debug, thiserror::Error)]
pub enum CommandError {
    #[error("not registered")]
    NotRegistered,

    #[error("already registered")]
    AlreadyRegistered,

    #[error("need more parameters")]
    NeedMoreParams,

    #[error("no such nick: {0}")]
    NoSuchNick(String),

    #[error("no such channel: {0}")]
    NoSuchChannel(String),

    #[error("nickname in use: {0}")]
    NicknameInUse(String),

    #[error("not on channel: {0}")]
    NotOnChannel(String),

    #[error("not channel operator")]
    NotChannelOp,

    #[error("channel is full")]
    ChannelFull,

    #[error("invite only channel")]
    InviteOnly,

    #[error("banned from channel")]
    Banned,

    #[error("bad channel key")]
    BadKey,
}
```

## Testing

1. **Unit tests**: Handler logic with mock state
2. **Integration tests**: Full client connections
3. **Multi-client scenarios**: JOINs, messaging, modes
4. **Load tests**: Connection limits, message throughput

```rust
#[tokio::test]
async fn test_basic_registration() {
    let server = TestServer::start().await;
    let mut client = server.connect().await;

    client.send("NICK testuser").await;
    client.send("USER test 0 * :Test User").await;

    let welcome = client.recv().await;
    assert!(welcome.starts_with(":testserver 001"));
}
```

## Open Questions

1. **Hostname Resolution**: Resolve PTR records or use IP?
   - Recommendation: IP by default, optional async resolution

2. **Host Cloaking**: Implement IRCv3 host cloaking?
   - Recommendation: Simple cloaking via config, not initially

3. **Services Integration**: NickServ/ChanServ support?
   - Recommendation: Defer to Phase 5+, focus on core protocol

4. **WebSocket Support**: Native WS or via proxy?
   - Recommendation: Native via `tokio-tungstenite` as optional listener

5. **Database Backend**: Persist state (bans, etc.)?
   - Recommendation: Defer, use files or in-memory initially
