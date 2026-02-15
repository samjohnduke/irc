# IRC

A modern, from-scratch implementation of the Internet Relay Chat protocol in Rust.

## Vision

IRC is one of the oldest chat protocols still in active use, powering communities for over 35 years. While newer platforms have emerged, IRC remains valued for its simplicity, openness, and lack of corporate control.

This project aims to build a complete IRC ecosystem in Rust:

- **A server** that's fast, secure, and easy to deploy
- **A terminal client** for power users and remote access
- **A desktop client** with a modern, polished interface

We're not trying to reinvent IRC or extend it beyond recognition. The goal is a faithful, well-crafted implementation of the protocol as specified in the IETF RFCs, with sensible modern additions (TLS, IRCv3 extensions) where they improve security and usability.

## Why Build This?

**Learning.** IRC is complex enough to be interesting but well-documented enough to be tractable. Building a full implementation teaches networking, async programming, protocol design, and UI development.

**Quality.** Many existing IRC implementations are decades old, written in C, and carry significant technical debt. Rust offers memory safety, modern tooling, and excellent async support.

**Completeness.** Most projects implement either a server or a client. We want both, sharing protocol code, tested together, documented together.

## Project Structure

```
irc/
├── crates/
│   ├── irc-proto/        # Protocol types and parsing
│   ├── irc-server/       # IRC daemon
│   ├── irc-client-lib/   # Shared client logic
│   ├── irc-cli/          # Terminal client
│   └── irc-gui/          # Desktop client
└── docs/
    ├── 00-overview.md    # Goals, scope, success criteria
    ├── 01-architecture.md # Technical architecture
    ├── crates/           # Per-crate design documents
    │   ├── irc-proto.md
    │   ├── irc-server.md
    │   ├── irc-client-lib.md
    │   ├── irc-cli.md
    │   └── irc-gui.md
    └── rfc/              # IETF RFC reference documents
```

## Design Principles

1. **Correctness first.** Follow the RFCs. Pass interoperability tests with existing clients and servers.

2. **Safe by default.** TLS everywhere. No buffer overflows. Validate all input.

3. **Simple deployment.** Single binary for the server. No external dependencies beyond optional TLS certificates.

4. **Code reuse.** Protocol parsing is shared. Client connection logic is shared. Don't repeat yourself.

5. **Readable code.** Clear over clever. Good names. Useful comments where behavior isn't obvious.

## Installation

**Quick install:**

```sh
curl -sSf https://raw.githubusercontent.com/samjohnduke/irc/main/install.sh | sh
```

**Or download binaries** from the [releases page](https://github.com/samjohnduke/irc/releases).

**Build from source:**

```sh
git clone https://github.com/samjohnduke/irc.git
cd irc
cargo build --release
```

## Status

**Phase: Active Development**

The core protocol implementation is complete. We're currently polishing the CLI client UI.

## Documentation

See the `docs/` directory for detailed design documents:

- [Overview](docs/00-overview.md) - Goals, scope, and success criteria
- [Architecture](docs/01-architecture.md) - Technical architecture and design principles
- [Verification](docs/02-verification.md) - Testing strategy and compliance checking
- [Security](docs/03-security.md) - TLS encryption, SASL authentication
- [Services](docs/04-services.md) - NickServ, ChanServ, account management
- [Extensions](docs/05-extensions.md) - Rich text, image sharing, modern features
- [FILEHOST](docs/06-filehost.md) - Server-side file upload implementation
- [IRCv3](docs/07-ircv3.md) - Modern protocol extensions analysis
- Crate designs:
  - [irc-proto](docs/crates/irc-proto.md) - Protocol types and parsing
  - [irc-server](docs/crates/irc-server.md) - Server implementation
  - [irc-client-lib](docs/crates/irc-client-lib.md) - Client library
  - [irc-cli](docs/crates/irc-cli.md) - Terminal client
  - [irc-gui](docs/crates/irc-gui.md) - Desktop client
- [RFC Reference](docs/rfc/) - IETF RFC 2810-2813

## License

Licensed under MIT OR Apache-2.0
