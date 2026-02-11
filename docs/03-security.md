# Security

Transport encryption and authentication for IRC connections.

## TLS Encryption

### Overview

TLS encrypts all traffic between client and server. It's the primary security mechanism for IRC.

```
┌────────┐                      ┌────────┐
│ Client │◄────TLS Tunnel──────►│ Server │
└────────┘                      └────────┘
    │                               │
    └─ All IRC traffic encrypted ───┘
```

### Standard Ports

| Port | Protocol | Status |
|------|----------|--------|
| 6667 | Plain TCP | Legacy, discouraged |
| 6697 | TLS | Standard secure port |
| 6660-6669 | Plain TCP | Traditional range |
| 7000-7002 | TLS | Alternative secure range |

### Server Implementation

We use `tokio-rustls` for TLS support.

```rust
use tokio_rustls::TlsAcceptor;
use rustls::ServerConfig;

// Load certificate and key
let certs = load_certs(&config.tls.cert_file)?;
let key = load_private_key(&config.tls.key_file)?;

// Build TLS config
let tls_config = ServerConfig::builder()
    .with_no_client_auth()
    .with_single_cert(certs, key)?;

let acceptor = TlsAcceptor::from(Arc::new(tls_config));

// Accept connections
let tls_stream = acceptor.accept(tcp_stream).await?;
```

### Server Configuration

```toml
# Plain TCP (development only)
[[listen]]
address = "0.0.0.0:6667"

# TLS (recommended for production)
[[listen]]
address = "0.0.0.0:6697"
[listen.tls]
cert_file = "/etc/irc/cert.pem"
key_file = "/etc/irc/key.pem"
```

### Certificate Options

| Option | Use Case | Notes |
|--------|----------|-------|
| Let's Encrypt | Production | Free, auto-renewal with ACME |
| Self-signed | Development | Generate with `openssl` or `mkcert` |
| Purchased CA | Enterprise | Traditional approach |

### Client Implementation

```rust
use tokio_rustls::TlsConnector;
use rustls::ClientConfig;

// Use system root certificates
let root_store = rustls::RootCertStore::from_iter(
    webpki_roots::TLS_SERVER_ROOTS.iter().cloned()
);

let tls_config = ClientConfig::builder()
    .with_root_certificates(root_store)
    .with_no_client_auth();

let connector = TlsConnector::from(Arc::new(tls_config));

// Connect with server name for SNI
let server_name = "irc.example.com".try_into()?;
let tls_stream = connector.connect(server_name, tcp_stream).await?;
```

### Certificate Verification

Clients should verify server certificates by default. Options:

| Mode | Security | Use Case |
|------|----------|----------|
| Verify (default) | High | Production |
| TOFU (Trust On First Use) | Medium | Self-signed servers |
| Insecure (skip verify) | None | Testing only |

```rust
// Configuration option
pub struct TlsOptions {
    /// Verify server certificate (default: true)
    pub verify: bool,

    /// Accept self-signed on first connection, remember for future
    pub trust_on_first_use: bool,

    /// Path to custom CA certificate
    pub ca_cert: Option<PathBuf>,
}
```

## Authentication

### Methods Overview

IRC supports multiple authentication methods:

```
┌─────────────────────────────────────────────────────────────┐
│                  Authentication Timeline                     │
├─────────────────────────────────────────────────────────────┤
│                                                             │
│  Connection ──► CAP ──► SASL ──► NICK/USER ──► Registered   │
│       │                   │                                 │
│       │                   └─ Auth BEFORE registration       │
│       │                                                     │
│  Connection ──► PASS ──► NICK/USER ──► Registered           │
│                   │                                         │
│                   └─ Server password (simple)               │
│                                                             │
│  Connection ──► NICK/USER ──► Registered ──► NickServ       │
│                                                 │           │
│                                                 └─ Auth     │
│                                                    AFTER    │
│                                                             │
└─────────────────────────────────────────────────────────────┘
```

| Method | When | Security | Notes |
|--------|------|----------|-------|
| PASS | Before NICK/USER | Low | Simple server password |
| SASL | During CAP negotiation | High | Modern, recommended |
| NickServ | After registration | Medium | Legacy, still common |

### Server Password (PASS)

Simple shared password for server access.

```
Client: PASS secretpassword
Client: NICK alice
Client: USER alice 0 * :Alice
Server: :server 001 alice :Welcome
```

**Use cases:**
- Private servers
- Bouncer authentication (ZNC)
- Simple access control

**Limitations:**
- Single shared password
- No per-user accounts
- Sent in plaintext (requires TLS)

### SASL Authentication

SASL is the modern, recommended approach. Authenticates during connection before joining channels.

#### Capability Negotiation

```
Client: CAP LS 302
Server: CAP * LS :sasl=PLAIN,SCRAM-SHA-256 ...

Client: CAP REQ :sasl
Server: CAP * ACK :sasl
```

#### PLAIN Mechanism

Simple username/password. **Requires TLS.**

```
Client: AUTHENTICATE PLAIN
Server: AUTHENTICATE +
Client: AUTHENTICATE AGFsaWNlAHNlY3JldHBhc3N3b3Jk
Server: :server 900 alice alice!alice@host alice :You are now logged in as alice
Server: :server 903 alice :SASL authentication successful

Client: CAP END
Client: NICK alice
Client: USER alice 0 * :Alice
Server: :server 001 alice :Welcome
```

The AUTHENTICATE payload is base64-encoded: `\0username\0password`

```rust
fn encode_plain(username: &str, password: &str) -> String {
    let payload = format!("\0{}\0{}", username, password);
    base64::encode(payload)
}
```

#### SCRAM-SHA-256 Mechanism

Challenge-response protocol. Password never sent over wire.

```
Client: AUTHENTICATE SCRAM-SHA-256
Server: AUTHENTICATE +
Client: AUTHENTICATE <client-first-message>
Server: AUTHENTICATE <server-first-message>
Client: AUTHENTICATE <client-final-message>
Server: AUTHENTICATE <server-final-message>
Server: :server 903 alice :SASL authentication successful
```

**Advantages:**
- Password not transmitted
- Mutual authentication
- Works without TLS (but TLS still recommended)

#### EXTERNAL Mechanism

Client certificate authentication.

```
Client: AUTHENTICATE EXTERNAL
Server: AUTHENTICATE +
Client: AUTHENTICATE +
Server: :server 903 alice :SASL authentication successful
```

Requires client to present a certificate during TLS handshake.

#### Implementation

```rust
/// SASL session state
pub struct SaslSession {
    mechanism: SaslMechanism,
    buffer: Vec<u8>,  // For multi-chunk AUTHENTICATE
}

pub enum SaslMechanism {
    Plain,
    ScramSha256(ScramState),
    External,
}

/// SASL result
pub enum SaslResult {
    /// Need more data
    Continue(Vec<u8>),
    /// Authentication successful
    Success { account: String },
    /// Authentication failed
    Failure(SaslError),
}

impl SaslSession {
    pub fn step(&mut self, data: &[u8]) -> SaslResult {
        match &mut self.mechanism {
            SaslMechanism::Plain => self.handle_plain(data),
            SaslMechanism::ScramSha256(state) => state.step(data),
            SaslMechanism::External => self.handle_external(),
        }
    }
}
```

#### SASL Numerics

| Code | Name | Meaning |
|------|------|---------|
| 900 | RPL_LOGGEDIN | Successfully logged in |
| 901 | RPL_LOGGEDOUT | Logged out |
| 902 | ERR_NICKLOCKED | Cannot change nick while registered |
| 903 | RPL_SASLSUCCESS | SASL auth successful |
| 904 | ERR_SASLFAIL | SASL auth failed |
| 905 | ERR_SASLTOOLONG | SASL message too long |
| 906 | ERR_SASLABORTED | SASL aborted |
| 907 | ERR_SASLALREADY | Already authenticated |
| 908 | RPL_SASLMECHS | Available mechanisms |

### Password Hashing

For stored passwords (accounts), always use argon2:

```rust
use argon2::{Argon2, PasswordHash, PasswordHasher, PasswordVerifier};
use argon2::password_hash::{rand_core::OsRng, SaltString};

pub fn hash_password(password: &str) -> Result<String, Error> {
    let salt = SaltString::generate(&mut OsRng);
    let argon2 = Argon2::default();
    let hash = argon2.hash_password(password.as_bytes(), &salt)?;
    Ok(hash.to_string())
}

pub fn verify_password(password: &str, hash: &str) -> Result<bool, Error> {
    let parsed = PasswordHash::new(hash)?;
    Ok(Argon2::default()
        .verify_password(password.as_bytes(), &parsed)
        .is_ok())
}
```

**Never:**
- Store plaintext passwords
- Use MD5, SHA1, or unsalted hashes
- Use bcrypt with low cost factor

## Rate Limiting

Prevent abuse with connection and command rate limits.

```rust
pub struct RateLimiter {
    /// Commands per second allowed
    commands_per_second: f32,

    /// Burst allowance
    burst: u32,

    /// Current token bucket
    tokens: f32,
    last_update: Instant,
}

impl RateLimiter {
    pub fn check(&mut self) -> bool {
        self.refill();
        if self.tokens >= 1.0 {
            self.tokens -= 1.0;
            true
        } else {
            false  // Rate limited
        }
    }
}
```

### Default Limits

| Resource | Limit | Notes |
|----------|-------|-------|
| Commands/sec | 2 | Per client |
| Connection attempts | 5/min | Per IP |
| Registration timeout | 60s | Before NICK/USER complete |
| SASL attempts | 3 | Per connection |

## Security Checklist

### Server

- [ ] TLS enabled on production port (6697)
- [ ] Valid certificate (not self-signed in production)
- [ ] SASL mechanisms configured
- [ ] Passwords hashed with argon2
- [ ] Rate limiting enabled
- [ ] Registration timeout configured

### Client

- [ ] TLS verification enabled by default
- [ ] SASL preferred over NickServ
- [ ] Credentials stored securely (keyring)
- [ ] Certificate pinning for known servers (optional)

## References

- [IRCv3 SASL 3.1](https://ircv3.net/specs/extensions/sasl-3.1)
- [IRCv3 SASL 3.2](https://ircv3.net/specs/extensions/sasl-3.2)
- [SASL Mechanisms](https://ircv3.net/docs/sasl-mechs)
- [RFC 5802](https://tools.ietf.org/html/rfc5802) - SCRAM
- [Libera.Chat SASL Guide](https://libera.chat/guides/sasl)
