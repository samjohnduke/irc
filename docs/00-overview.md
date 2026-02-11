# Project Overview

## What We're Building

A complete IRC implementation in Rust consisting of five crates:

| Component | What It Does |
|-----------|--------------|
| **irc-proto** | Parses and serializes IRC messages. Defines types for commands, replies, modes. Shared by all other crates. |
| **irc-server** | A full IRC daemon. Accepts connections, manages channels, routes messages. Can run a standalone network or link with other servers. |
| **irc-client-lib** | Connection and session management for clients. Handles the network layer so UI code can focus on display and interaction. |
| **irc-cli** | A terminal-based IRC client with a modern TUI. Channels, scrollback, nick completion, the works. |
| **irc-gui** | A desktop IRC client with a graphical interface. Native look and feel, system notifications, multiple server support. |

## Goals

### Correctness

We implement the protocol as specified in RFC 2810-2813. Our server should work with any compliant client (irssi, WeeChat, HexChat, etc.). Our clients should work with any compliant server (Libera, OFTC, self-hosted).

### Performance

The server should handle thousands of concurrent connections efficiently. Message parsing should be fast enough to never be a bottleneck. Clients should feel responsive even on slow connections.

### Security

- TLS support for all connections (optional but encouraged)
- No memory safety bugs (Rust handles this)
- Input validation on all protocol messages
- Rate limiting to prevent abuse
- Secure password storage for operators

### Usability

- Server: single binary, simple config file, sensible defaults
- CLI: familiar keybindings, intuitive commands, works over SSH
- GUI: modern interface, system integration, low learning curve

### Maintainability

- Clear code organization
- Comprehensive documentation
- Good test coverage
- Minimal dependencies

## Non-Goals

Things we're explicitly not trying to do:

- **Invent new protocols.** We implement IRC as specified, plus established extensions (IRCv3).
- **Replace Discord/Slack.** IRC is IRC. We're not adding reactions, threads, or file previews.
- **Support every extension.** We focus on core functionality and widely-adopted IRCv3 specs.
- **Maximum compatibility with broken clients.** We follow the spec; if a client is buggy, that's not our problem.

## Protocol Scope

### Core (RFC 2810-2813)

Everything in the base RFCs:

- Connection registration (NICK, USER, PASS)
- Channel operations (JOIN, PART, KICK, INVITE, TOPIC, MODE)
- Messaging (PRIVMSG, NOTICE)
- User queries (WHO, WHOIS, WHOWAS)
- Server queries (MOTD, LUSERS, VERSION, STATS)
- Operator commands (KILL, WALLOPS)
- All numeric replies

### IRCv3 Extensions (Planned)

Modern extensions that improve the experience:

| Extension | Purpose | Priority |
|-----------|---------|----------|
| CAP | Capability negotiation | High |
| SASL | Secure authentication | High |
| message-tags | Metadata on messages | Medium |
| server-time | Accurate timestamps | Medium |
| away-notify | Real-time away status | Medium |
| multi-prefix | Show all user modes | Medium |
| account-notify | Track user accounts | Low |
| batch | Group related messages | Low |

### Out of Scope

- DCC (direct client-to-client file transfer) - complex, security concerns
- IRCv3 specs marked experimental or deprecated
- Non-standard extensions specific to particular networks

## Architecture Decisions

### Why Rust?

- Memory safety without garbage collection
- Excellent async ecosystem (tokio)
- Strong type system catches bugs at compile time
- Single binary deployment
- Good cross-platform support

### Why Tokio?

- Mature, battle-tested async runtime
- Excellent documentation
- Rich ecosystem (codecs, TLS, etc.)
- Used by major production systems

### Why Iced for GUI?

- Pure Rust (no C bindings)
- Elm-like architecture (simple state management)
- Cross-platform
- Active development

Alternatives considered:
- egui: good but more suited to tools than chat apps
- Tauri: adds web complexity, loses "pure Rust" benefit
- GTK/Qt bindings: C dependencies, more complex builds

### Why Ratatui for CLI?

- Standard choice for Rust TUIs
- Active community
- Flexible widget system
- Works well with tokio

## Implementation Phases

### Phase 1: Foundation

- [ ] `irc-proto`: message parsing, core types
- [ ] `irc-server`: basic connection handling, registration
- [ ] Integration: connect client to server, exchange messages

### Phase 2: Core Protocol

- [ ] `irc-server`: channels, messaging, modes, queries
- [ ] `irc-client-lib`: connection management, session state
- [ ] `irc-cli`: basic TUI, single server

### Phase 3: Full Server

- [ ] `irc-server`: operators, MOTD, all commands
- [ ] `irc-server`: TLS encryption (port 6697)
- [ ] `irc-server`: configuration, logging

### Phase 4: IRCv3 Core + Auth

- [ ] CAP negotiation, cap-notify
- [ ] SASL authentication (PLAIN, SCRAM-SHA-256)
- [ ] message-tags, server-time, msgid
- [ ] echo-message, batch, labeled-response
- [ ] `irc-cli`: multi-server, full command set
- [ ] `irc-gui`: initial implementation

### Phase 5: Services + Account Features

- [ ] Built-in account registration
- [ ] NickServ (nickname enforcement)
- [ ] ChanServ (channel registration)
- [ ] Account database (SQLite)
- [ ] IRCv3: account-notify, account-tag, extended-join
- [ ] IRCv3: away-notify, chghost, setname, multi-prefix

### Phase 6: Modern Experience

- [ ] FILEHOST: HTTP upload endpoint (see [06-filehost.md](06-filehost.md))
- [ ] FILEHOST: Storage backends (filesystem, S3)
- [ ] CHATHISTORY: Message history storage and retrieval
- [ ] Client: File upload, URL preview, markdown input
- [ ] Client: +typing, +reply, +react tags
- [ ] Client: Read markers, message grouping, polish
- [ ] `irc-gui`: themes, notifications, polish

### Phase 7: Advanced (Optional)

- [ ] Server-to-server linking (RFC 2813)
- [ ] External services support (Anope/Atheme)
- [ ] WebSocket transport
- [ ] Monitor, WHOX, userhost-in-names

See [07-ircv3.md](07-ircv3.md) for complete IRCv3 specification analysis.
- [ ] IPFS/libp2p media sharing (experimental)

See [03-security.md](03-security.md) for TLS and authentication details.
See [04-services.md](04-services.md) for NickServ/ChanServ architecture.

## Testing Strategy

See [02-verification.md](02-verification.md) for the complete testing and compliance strategy.

**Summary:**

1. **Unit tests** - Protocol parsing, command handling, state management
2. **Fuzz testing** - Malformed input to the protocol parser
3. **Integration tests** - Our server + our clients, end-to-end
4. **Conformance tests** - [irctest](https://github.com/progval/irctest) suite against RFC 2812 and Modern IRC spec
5. **Interoperability** - Our clients vs real servers, real clients vs our server

## Success Criteria

How we'll know we're done:

1. **Server**: Can run a small community IRC network (10-100 users)
2. **CLI**: Daily-driveable as a primary IRC client
3. **GUI**: Polished enough that non-technical users find it approachable
4. **Interop**: Works with major networks and clients without issues
5. **Docs**: Someone new can understand the codebase and contribute
