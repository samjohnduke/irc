# IRC Project Architecture

## Overview

A Rust-based IRC implementation consisting of five crates organized as a Cargo workspace. The design prioritizes correctness, performance, and code reuse across server and client components.

## Crate Dependency Graph

```
                    ┌─────────────┐
                    │  irc-proto  │
                    └──────┬──────┘
                           │
              ┌────────────┴────────────┐
              │                         │
              ▼                         ▼
       ┌─────────────┐          ┌──────────────┐
       │ irc-server  │          │irc-client-lib│
       └─────────────┘          └──────┬───────┘
                                       │
                                ┌──────┴──────┐
                                │             │
                                ▼             ▼
                          ┌─────────┐   ┌─────────┐
                          │ irc-cli │   │ irc-gui │
                          └─────────┘   └─────────┘
```

> **Note:** `irc-client-lib` re-exports `irc-proto` types. CLI and GUI access
> protocol types (e.g., `Command`, `Message`) through `irc-client-lib` rather
> than depending on `irc-proto` directly.

## Crate Responsibilities

| Crate | Purpose | Binary? |
|-------|---------|---------|
| `irc-proto` | Protocol parsing, serialization, types | No (library) |
| `irc-server` | IRC daemon implementation | Yes |
| `irc-client-lib` | Shared client connection/session logic | No (library) |
| `irc-cli` | Terminal-based IRC client | Yes |
| `irc-gui` | Desktop GUI IRC client | Yes |

## Protocol Compliance

### Implemented RFCs
- **RFC 2810** - IRC Architecture
- **RFC 2811** - Channel Management
- **RFC 2812** - Client Protocol
- **RFC 2813** - Server Protocol (Phase 5 - optional)

### Future Extensions (IRCv3)
- Capability negotiation (`CAP`)
- SASL authentication
- Message tags
- Server-time
- Account tracking

## Design Principles

### 1. Zero-Copy Parsing Where Possible
The protocol layer uses borrowed data (`&str`, `&[u8]`) during parsing to avoid allocations. Owned versions are created only when storing state.

### 2. Type-Safe Commands
Each IRC command is a distinct enum variant with typed parameters, not stringly-typed maps.

### 3. Async-First
All I/O uses `tokio` for async operations. The server can handle thousands of concurrent connections.

### 4. Separation of Concerns
- Protocol logic is isolated in `irc-proto`
- Connection handling is separate from UI
- Server state management is decoupled from network I/O

### 5. Testability
- Protocol parsing is pure and easily unit-tested
- Server logic can be tested without network
- Integration tests use real TCP connections

## Error Handling Strategy

```rust
// Each crate defines its own error type
pub enum Error {
    // Protocol-level errors (malformed messages)
    Protocol(irc_proto::Error),

    // I/O errors (connection lost, etc.)
    Io(std::io::Error),

    // TLS errors
    Tls(/* ... */),

    // Application-specific errors
    // ...
}
```

All errors implement `std::error::Error` and are composable via `thiserror`.

## Configuration

Each binary crate uses a TOML configuration file:

```
~/.config/irc/
├── server.toml      # irc-server config
├── client.toml      # Shared client config (servers, identity)
└── gui.toml         # GUI-specific settings (theme, layout)
```

## Security Considerations

1. **TLS Support** - Optional but recommended for all connections
2. **Password Hashing** - Server stores operator passwords hashed (argon2)
3. **Rate Limiting** - Server implements per-client rate limits
4. **Input Validation** - All protocol input is validated before processing
5. **No Eval** - No dynamic code execution from network input

## Performance Targets

Measured with standard IRC messages (≤512 bytes) on a multi-core system:

| Metric | Target | Notes |
|--------|--------|-------|
| Concurrent connections (server) | 10,000+ | Idle connections, single instance |
| Message throughput | 100,000 msg/sec | Aggregate across all clients, multi-core |
| Memory per connection | < 64 KB | Excluding message history buffers |
| Latency (message relay) | < 1ms p99 | Sender → recipient, same server |

## Workspace Structure

The project is a Cargo workspace with all crates in the root:

```toml
# Cargo.toml (workspace root)
[workspace]
members = [
    "irc-proto",
    "irc-server",
    "irc-client-lib",
    "irc-cli",
    "irc-gui",
]

[workspace.dependencies]
tokio = { version = "1", features = ["full"] }
tokio-util = { version = "0.7", features = ["codec"] }
serde = { version = "1", features = ["derive"] }
thiserror = "2"
tracing = "0.1"
tracing-subscriber = "0.3"
```

## Graceful Shutdown

All binaries handle `SIGINT`/`SIGTERM` via `tokio::signal`:

- **Server**: Stops accepting new connections, sends `QUIT` / `ERROR :Server shutting down` to all clients, waits up to 5 seconds for connections to drain, then exits.
- **CLI/GUI clients**: Send `QUIT` to all connected servers, wait briefly for the server to acknowledge, then exit.

## Logging Conventions

All crates use `tracing` for structured logging:

- **ERROR**: Unrecoverable failures (bind failed, TLS misconfigured)
- **WARN**: Recoverable issues (client disconnected unexpectedly, rate limited)
- **INFO**: Lifecycle events (server started, client registered, channel created)
- **DEBUG**: Protocol-level detail (messages sent/received, state changes)
- **TRACE**: Very verbose (parsing steps, lock acquisitions)

Spans should include relevant context fields:
```rust
#[tracing::instrument(skip(state), fields(client_id = %client.id, nick = %nick))]
async fn handle_join(state: &ServerState, client: &Client, channel: &str) { ... }
```

## Testing Strategy

1. **Unit Tests** - Protocol parsing, command handling
2. **Integration Tests** - Multi-client scenarios
3. **Fuzz Testing** - Protocol parser (cargo-fuzz)
4. **Benchmarks** - Message parsing, routing (criterion)
