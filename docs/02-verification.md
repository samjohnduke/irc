# Verification and Compliance Testing

How we verify that our IRC implementation is correct and interoperable.

## Testing Layers

```
┌─────────────────────────────────────────────────────────────┐
│                    Interoperability                         │
│         (Our code ↔ Real-world servers/clients)             │
├─────────────────────────────────────────────────────────────┤
│                  Conformance Testing                        │
│              (irctest suite, parser tests)                  │
├─────────────────────────────────────────────────────────────┤
│                   Integration Tests                         │
│            (Our server ↔ Our clients, E2E)                  │
├─────────────────────────────────────────────────────────────┤
│                      Unit Tests                             │
│          (Protocol parsing, command handling)               │
├─────────────────────────────────────────────────────────────┤
│                     Fuzz Testing                            │
│              (Malformed input, edge cases)                  │
└─────────────────────────────────────────────────────────────┘
```

## 1. Unit Tests

### Protocol Parsing (`irc-proto`)

Every message type must round-trip correctly:

```rust
#[test]
fn test_privmsg_parse_serialize() {
    let input = ":nick!user@host PRIVMSG #channel :Hello, world!\r\n";
    let msg = parse_message(input.as_bytes()).unwrap();
    assert_eq!(msg.to_string() + "\r\n", input);
}
```

Test categories:
- **Valid messages**: All command types parse correctly
- **Edge cases**: Empty params, max-length messages, special characters
- **Invalid messages**: Proper errors for malformed input
- **Numeric replies**: All reply codes parse and serialize

### Command Handling (`irc-server`)

Each command handler tested in isolation:

```rust
#[tokio::test]
async fn test_join_creates_channel() {
    let state = ServerState::new_test();
    let client = state.add_test_client("nick", "user", "host").await;

    handle_join(&state, &client, &["#test"]).await.unwrap();

    assert!(state.channel_exists("#test"));
    assert!(state.is_member(&client.id, "#test"));
}
```

## 2. Fuzz Testing

The protocol parser is a security boundary. We fuzz it extensively.

### Setup with cargo-fuzz

```rust
// fuzz/fuzz_targets/parse_message.rs
#![no_main]
use libfuzzer_sys::fuzz_target;
use irc_proto::parse_message;

fuzz_target!(|data: &[u8]| {
    // Should never panic, regardless of input
    let _ = parse_message(data);
});
```

### Fuzz targets

| Target | Input | Goal |
|--------|-------|------|
| `parse_message` | Arbitrary bytes | No panics, no hangs |
| `parse_mode` | Mode strings | Handle malformed modes |
| `hostmask_match` | Hostmask + user | No regex catastrophic backtracking |
| `validate_nick` | Arbitrary strings | Consistent accept/reject |

### Running fuzzing

```bash
# Install cargo-fuzz
cargo install cargo-fuzz

# Run for 1 hour
cargo +nightly fuzz run parse_message -- -max_total_time=3600

# Run with sanitizers
RUSTFLAGS="-Zsanitizer=address" cargo +nightly fuzz run parse_message
```

## 3. Integration Tests

End-to-end tests with real TCP connections between our components.

### Test Infrastructure

```rust
/// Spawns a test server on a random port
pub async fn spawn_test_server() -> TestServer {
    let config = ServerConfig::test_default();
    let (server, addr) = Server::bind_random(config).await;
    tokio::spawn(server.run());
    TestServer { addr }
}

/// Connects a raw TCP client for protocol-level testing
pub async fn connect_raw(addr: SocketAddr) -> RawClient {
    let stream = TcpStream::connect(addr).await.unwrap();
    RawClient::new(stream)
}
```

### Scenario Tests

```rust
#[tokio::test]
async fn test_two_users_chat() {
    let server = spawn_test_server().await;

    let mut alice = connect_raw(server.addr).await;
    let mut bob = connect_raw(server.addr).await;

    // Register both users
    alice.send("NICK alice\r\nUSER a 0 * :Alice\r\n").await;
    bob.send("NICK bob\r\nUSER b 0 * :Bob\r\n").await;

    // Wait for registration
    alice.expect_numeric(001).await;
    bob.expect_numeric(001).await;

    // Join same channel
    alice.send("JOIN #test\r\n").await;
    bob.send("JOIN #test\r\n").await;

    // Bob should see Alice in channel
    let names = bob.expect_numeric(353).await;
    assert!(names.contains("alice"));

    // Alice sends message
    alice.send("PRIVMSG #test :Hello Bob!\r\n").await;

    // Bob receives it
    let msg = bob.expect_privmsg().await;
    assert_eq!(msg.sender, "alice");
    assert_eq!(msg.text, "Hello Bob!");
}
```

### Test Scenarios

| Category | Tests |
|----------|-------|
| Registration | NICK/USER sequence, password, nick collision |
| Channels | JOIN, PART, KICK, INVITE, TOPIC |
| Modes | Channel modes, user modes, op privileges |
| Messaging | PRIVMSG, NOTICE, to channels, to users |
| Queries | WHO, WHOIS, NAMES, LIST |
| Edge cases | Max nick length, max message length, UTF-8 |
| Errors | All error numerics triggered correctly |

## 4. Conformance Testing with irctest

[irctest](https://github.com/progval/irctest) is the standard conformance suite for IRC implementations. It tests against:
- RFC 1459 / RFC 2812
- The [Modern IRC specification](https://modern.ircdocs.horse/)
- IRCv3 extensions

### Setting Up irctest

```bash
# Clone irctest
git clone https://github.com/progval/irctest
cd irctest

# Install dependencies
pip install -r requirements.txt

# Create controller for our server (see below)
```

### Writing a Controller

irctest needs a "controller" to start/stop our server:

```python
# irctest/controllers/irc_server.py
from irctest.basecontrollers import BaseServerController

class IrcServerController(BaseServerController):
    software_name = "irc-server"

    def run(self, hostname, port, start_tls=False, config=None):
        self.proc = subprocess.Popen([
            "irc-server",
            "--host", hostname,
            "--port", str(port),
            "--test-mode",  # Faster timeouts, no DNS lookups
        ])

    def kill(self):
        self.proc.terminate()
        self.proc.wait()
```

### Running Tests

```bash
# Run core tests (skip strict/deprecated)
pytest --controller irc_server -m 'not strict and not deprecated'

# Run specific test categories
pytest --controller irc_server -k "join"
pytest --controller irc_server -k "privmsg"
pytest --controller irc_server -k "mode"

# Run with verbose output
pytest --controller irc_server -v --tb=short
```

### Test Dashboard

irctest publishes daily results at [dashboard.irctest.limnoria.net](https://dashboard.irctest.limnoria.net/). Once we're passing tests, we can submit our server to be included.

## 5. Parser Test Corpus

[IRC DevDocs](https://dd.ircdocs.horse/tools) provides parser test files for edge cases.

### msg-split Tests

Tests for message parsing edge cases:

```yaml
# Example test case
- input: ":server 001 nick :Welcome to IRC\r\n"
  atoms:
    source: "server"
    command: "001"
    params: ["nick", "Welcome to IRC"]
```

### Using the Test Corpus

```rust
#[test]
fn test_ircdocs_parser_corpus() {
    let corpus = load_yaml("tests/data/msg-split.yaml");

    for test in corpus {
        let result = parse_message(test.input.as_bytes());

        match result {
            Ok(msg) => {
                assert_eq!(msg.prefix.map(|p| p.to_string()), test.atoms.source);
                assert_eq!(msg.command.to_string(), test.atoms.command);
                assert_eq!(msg.params(), test.atoms.params);
            }
            Err(e) => {
                assert!(test.atoms.is_none(), "Expected parse, got error: {}", e);
            }
        }
    }
}
```

## 6. Interoperability Testing

### Our Clients → Real Servers

Test our clients against established networks:

| Server | Address | Tests |
|--------|---------|-------|
| Libera.Chat | `irc.libera.chat:6697` | Full client workflow |
| OFTC | `irc.oftc.net:6697` | Registration, channels |
| IRCCloud | Via bouncer | Persistence |

```rust
#[tokio::test]
#[ignore] // Run manually, requires network
async fn test_client_against_libera() {
    let mut client = Client::connect("irc.libera.chat", 6697, true).await?;

    client.register("testnick", "testuser", "Test Client").await?;
    client.join("#libera").await?;

    // Verify we get NAMES reply
    let event = client.next_event().await?;
    assert!(matches!(event, Event::NamesUpdated { .. }));

    client.quit("Test complete").await?;
}
```

### Real Clients → Our Server

Test established clients against our server:

| Client | Platform | Test Method |
|--------|----------|-------------|
| irssi | Terminal | Manual + scripted |
| WeeChat | Terminal | Manual + scripted |
| HexChat | GUI | Manual |
| Textual | macOS | Manual |
| The Lounge | Web | Manual |

Create test scripts for terminal clients:

```perl
# irssi test script
/connect localhost 6667
/nick testuser
/join #test
/msg #test Hello from irssi!
/part #test
/quit
```

### Docker Test Environment

Use IRCDocs Docker images for comparison testing:

```bash
# Pull reference server images
docker pull irccom/inspircd
docker pull irccom/unrealircd
docker pull irccom/ergo

# Run side-by-side comparison
./scripts/compare-servers.sh
```

## 7. Continuous Integration

### GitHub Actions Workflow

```yaml
name: Test

on: [push, pull_request]

jobs:
  test:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4

      - name: Unit tests
        run: cargo test --all

      - name: Integration tests
        run: cargo test --test integration

      - name: Clippy
        run: cargo clippy -- -D warnings

  conformance:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4

      - name: Build server
        run: cargo build --release -p irc-server

      - name: Setup irctest
        run: |
          git clone https://github.com/progval/irctest
          pip install -r irctest/requirements.txt

      - name: Run conformance tests
        run: |
          cd irctest
          pytest --controller irc_server -m 'not strict' --tb=short

  fuzz:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4

      - name: Install cargo-fuzz
        run: cargo install cargo-fuzz

      - name: Fuzz parser (10 minutes)
        run: |
          cd crates/irc-proto
          cargo +nightly fuzz run parse_message -- -max_total_time=600
```

## 8. Compliance Checklist

Track which RFC requirements are implemented and tested:

### RFC 2812 Commands

| Command | Implemented | Unit Test | Integration | irctest |
|---------|-------------|-----------|-------------|---------|
| PASS | [ ] | [ ] | [ ] | [ ] |
| NICK | [ ] | [ ] | [ ] | [ ] |
| USER | [ ] | [ ] | [ ] | [ ] |
| OPER | [ ] | [ ] | [ ] | [ ] |
| QUIT | [ ] | [ ] | [ ] | [ ] |
| JOIN | [ ] | [ ] | [ ] | [ ] |
| PART | [ ] | [ ] | [ ] | [ ] |
| MODE | [ ] | [ ] | [ ] | [ ] |
| TOPIC | [ ] | [ ] | [ ] | [ ] |
| NAMES | [ ] | [ ] | [ ] | [ ] |
| LIST | [ ] | [ ] | [ ] | [ ] |
| INVITE | [ ] | [ ] | [ ] | [ ] |
| KICK | [ ] | [ ] | [ ] | [ ] |
| PRIVMSG | [ ] | [ ] | [ ] | [ ] |
| NOTICE | [ ] | [ ] | [ ] | [ ] |
| MOTD | [ ] | [ ] | [ ] | [ ] |
| WHO | [ ] | [ ] | [ ] | [ ] |
| WHOIS | [ ] | [ ] | [ ] | [ ] |
| PING | [ ] | [ ] | [ ] | [ ] |
| PONG | [ ] | [ ] | [ ] | [ ] |

### Numeric Replies

Track that each numeric is sent in the right circumstances with correct format.

### Channel Modes

| Mode | Meaning | Implemented | Tested |
|------|---------|-------------|--------|
| o | Operator | [ ] | [ ] |
| v | Voice | [ ] | [ ] |
| i | Invite-only | [ ] | [ ] |
| m | Moderated | [ ] | [ ] |
| n | No external | [ ] | [ ] |
| t | Topic lock | [ ] | [ ] |
| k | Key | [ ] | [ ] |
| l | Limit | [ ] | [ ] |
| b | Ban | [ ] | [ ] |
| e | Exception | [ ] | [ ] |
| I | Invite exception | [ ] | [ ] |

## Sources and References

- [irctest](https://github.com/progval/irctest) - Conformance test suite
- [Modern IRC Specification](https://modern.ircdocs.horse/) - Living specification
- [IRC DevDocs Testing Tools](https://dd.ircdocs.horse/tools) - Parser tests, Docker images
- [irctest Dashboard](https://dashboard.irctest.limnoria.net/) - Daily test results
- RFC 2810-2813 - Original IETF specifications (in `docs/rfc/`)
