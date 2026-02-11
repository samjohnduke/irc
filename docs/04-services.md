# Services

IRC services manage user accounts (NickServ), channel registration (ChanServ), and related features.

## What Are Services?

Services are special clients or built-in functionality that provide:

- **Account management** - Register usernames, store preferences
- **Nickname protection** - Enforce ownership of registered nicks
- **Channel registration** - Persistent channel ownership and settings
- **Access control** - Channel operator lists, ban management

```
┌─────────────────────────────────────────────────────────────┐
│                     Services Overview                        │
├─────────────────────────────────────────────────────────────┤
│                                                             │
│  NickServ     - Account registration and authentication    │
│  ChanServ     - Channel registration and management        │
│  HostServ     - Virtual host (vhost) management            │
│  OperServ     - Network operator tools                     │
│  MemoServ     - Offline messaging                          │
│  BotServ      - Channel bot management                     │
│                                                             │
└─────────────────────────────────────────────────────────────┘
```

## Architecture Options

### Option 1: No Services

Run the server without account or channel management.

```
┌─────────────────┐
│   irc-server    │
│                 │
│  No accounts    │
│  No persistence │
└─────────────────┘
```

**Pros:**
- Simplest to implement and deploy
- No database dependency
- Minimal attack surface

**Cons:**
- No nickname protection
- No channel persistence
- Channels disappear when empty

**Use cases:**
- Private team servers
- Trusted networks
- Development and testing

### Option 2: Built-in Services (Recommended)

The server itself handles accounts and channel registration.

```
┌─────────────────────────────────────────┐
│              irc-server                 │
│  ┌─────────────────────────────────┐    │
│  │        Core IRC Protocol        │    │
│  └─────────────────────────────────┘    │
│  ┌─────────────────────────────────┐    │
│  │      Built-in Services          │    │
│  │  ┌─────────┐ ┌─────────┐        │    │
│  │  │NickServ │ │ChanServ │        │    │
│  │  └─────────┘ └─────────┘        │    │
│  └─────────────────────────────────┘    │
│  ┌─────────────────────────────────┐    │
│  │         SQLite Database         │    │
│  └─────────────────────────────────┘    │
└─────────────────────────────────────────┘
```

**Pros:**
- Single binary deployment
- Tight integration with SASL
- No services link protocol needed
- Simpler administration

**Cons:**
- More server complexity
- Less feature-rich than Anope/Atheme
- Must maintain database schema

**Use cases:**
- Self-hosted servers
- Small to medium networks
- Modern deployments

**Reference:** [Ergo IRC Server](https://github.com/ergochat/ergo) uses this approach.

### Option 3: External Services

Separate services program links to the server.

```
┌─────────────────┐         ┌─────────────────┐
│   irc-server    │◄───────►│    Services     │
│                 │  Link   │  (Anope/Atheme) │
│                 │ Protocol│                 │
└─────────────────┘         │  NickServ       │
                            │  ChanServ       │
                            │  OperServ       │
                            │  MemoServ       │
                            │  ...            │
                            └─────────────────┘
```

**Pros:**
- Mature, feature-rich services
- Existing ecosystem (Anope, Atheme)
- Separation of concerns
- Can swap services implementations

**Cons:**
- Requires server-to-server linking
- More deployment complexity
- Additional maintenance burden

**Use cases:**
- Large networks
- Networks migrating from other IRCds
- Need for advanced features

## Our Approach: Built-in Services

We implement built-in services for simplicity and modern deployment.

### NickServ

Account and nickname management.

#### Commands

| Command | Description |
|---------|-------------|
| `REGISTER <password> [email]` | Create account |
| `IDENTIFY <password>` | Log in (prefer SASL) |
| `SET PASSWORD <new>` | Change password |
| `SET EMAIL <email>` | Change email |
| `INFO <nick>` | View account info |
| `GHOST <nick>` | Disconnect old session |
| `GROUP` | Link nick to account |
| `UNGROUP <nick>` | Unlink nick from account |
| `DROP` | Delete account |

#### Example Session

```
/msg NickServ REGISTER mypassword me@example.com
-NickServ- Account alice registered. You are now logged in.
-NickServ- Please verify your email within 24 hours.

/msg NickServ INFO alice
-NickServ- Account: alice
-NickServ- Registered: 2024-01-15 10:30:00 UTC
-NickServ- Nicknames: alice, alice_, alice_afk
-NickServ- Channels: #mychannel (founder), #other (op)
```

#### Implementation

```rust
pub struct NickServHandler {
    db: Database,
}

impl NickServHandler {
    pub async fn handle(
        &self,
        client: &Client,
        command: &str,
        args: &[&str],
    ) -> Result<Vec<Notice>, Error> {
        match command.to_uppercase().as_str() {
            "REGISTER" => self.handle_register(client, args).await,
            "IDENTIFY" => self.handle_identify(client, args).await,
            "INFO" => self.handle_info(client, args).await,
            "SET" => self.handle_set(client, args).await,
            "GHOST" => self.handle_ghost(client, args).await,
            "GROUP" => self.handle_group(client, args).await,
            _ => Ok(vec![Notice::new("Unknown command. Use HELP for commands.")]),
        }
    }
}
```

### ChanServ

Channel registration and access management.

#### Commands

| Command | Description |
|---------|-------------|
| `REGISTER <#channel>` | Register channel |
| `OP <#channel> [nick]` | Give operator status |
| `DEOP <#channel> [nick]` | Remove operator status |
| `VOICE <#channel> [nick]` | Give voice |
| `KICK <#channel> <nick> [reason]` | Kick user |
| `BAN <#channel> <mask>` | Add ban |
| `UNBAN <#channel> <mask>` | Remove ban |
| `ACCESS <#channel> LIST` | List access entries |
| `ACCESS <#channel> ADD <account> <level>` | Add access |
| `SET <#channel> <option> <value>` | Configure channel |
| `INFO <#channel>` | View channel info |
| `DROP <#channel>` | Unregister channel |

#### Access Levels

| Level | Name | Permissions |
|-------|------|-------------|
| 100 | Founder | Full control, can drop channel |
| 50 | Admin | Manage access list, all modes |
| 30 | Op | +o, kick, ban |
| 10 | Voice | +v |
| 0 | None | No automatic privileges |

#### Example Session

```
/join #mychannel
/msg ChanServ REGISTER #mychannel
-ChanServ- Channel #mychannel registered to alice.

/msg ChanServ ACCESS #mychannel ADD bob 30
-ChanServ- Added bob to #mychannel access list at level 30 (Op).

/msg ChanServ SET #mychannel DESCRIPTION A friendly channel
-ChanServ- Description for #mychannel set.
```

#### Implementation

```rust
pub struct ChanServHandler {
    db: Database,
}

impl ChanServHandler {
    pub async fn handle(
        &self,
        client: &Client,
        command: &str,
        args: &[&str],
    ) -> Result<Vec<Notice>, Error> {
        match command.to_uppercase().as_str() {
            "REGISTER" => self.handle_register(client, args).await,
            "OP" => self.handle_op(client, args).await,
            "ACCESS" => self.handle_access(client, args).await,
            "SET" => self.handle_set(client, args).await,
            "INFO" => self.handle_info(client, args).await,
            _ => Ok(vec![Notice::new("Unknown command.")]),
        }
    }

    async fn handle_register(
        &self,
        client: &Client,
        args: &[&str],
    ) -> Result<Vec<Notice>, Error> {
        let channel = args.get(0).ok_or(Error::NeedMoreParams)?;

        // Must be logged in
        let account = client.account()
            .ok_or(Error::NotLoggedIn)?;

        // Must be in the channel
        if !client.is_in_channel(channel) {
            return Err(Error::NotOnChannel);
        }

        // Register
        self.db.register_channel(channel, &account).await?;

        // Set founder as channel op
        client.server().set_mode(channel, "+o", &client.nick()).await?;

        Ok(vec![Notice::new(format!(
            "Channel {} registered to {}.",
            channel, account
        ))])
    }
}
```

## Database Schema

SQLite database for account and channel persistence.

```sql
-- Accounts
CREATE TABLE accounts (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    name TEXT UNIQUE NOT NULL COLLATE NOCASE,
    password_hash TEXT NOT NULL,
    email TEXT,
    registered_at TEXT NOT NULL DEFAULT (datetime('now')),
    last_seen TEXT,
    verified INTEGER NOT NULL DEFAULT 0
);

-- Nicknames linked to accounts
CREATE TABLE nicknames (
    nickname TEXT PRIMARY KEY COLLATE NOCASE,
    account_id INTEGER NOT NULL REFERENCES accounts(id) ON DELETE CASCADE,
    registered_at TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE INDEX idx_nicknames_account ON nicknames(account_id);

-- Registered channels
CREATE TABLE channels (
    name TEXT PRIMARY KEY COLLATE NOCASE,
    founder_id INTEGER NOT NULL REFERENCES accounts(id),
    registered_at TEXT NOT NULL DEFAULT (datetime('now')),
    last_used TEXT,
    topic TEXT,
    topic_setter TEXT,
    modes TEXT DEFAULT ''
);

-- Channel access list
CREATE TABLE channel_access (
    channel TEXT NOT NULL REFERENCES channels(name) ON DELETE CASCADE,
    account_id INTEGER NOT NULL REFERENCES accounts(id) ON DELETE CASCADE,
    level INTEGER NOT NULL DEFAULT 0,
    added_by TEXT,
    added_at TEXT NOT NULL DEFAULT (datetime('now')),
    PRIMARY KEY (channel, account_id)
);

CREATE INDEX idx_channel_access_account ON channel_access(account_id);

-- Channel settings
CREATE TABLE channel_settings (
    channel TEXT NOT NULL REFERENCES channels(name) ON DELETE CASCADE,
    key TEXT NOT NULL,
    value TEXT,
    PRIMARY KEY (channel, key)
);

-- Account settings/preferences
CREATE TABLE account_settings (
    account_id INTEGER NOT NULL REFERENCES accounts(id) ON DELETE CASCADE,
    key TEXT NOT NULL,
    value TEXT,
    PRIMARY KEY (account_id, key)
);
```

### Database Interface

```rust
pub struct Database {
    pool: SqlitePool,
}

impl Database {
    // Accounts
    pub async fn create_account(&self, name: &str, password_hash: &str, email: Option<&str>) -> Result<i64>;
    pub async fn get_account(&self, name: &str) -> Result<Option<Account>>;
    pub async fn verify_password(&self, name: &str, password: &str) -> Result<bool>;
    pub async fn update_last_seen(&self, account_id: i64) -> Result<()>;

    // Nicknames
    pub async fn register_nickname(&self, nick: &str, account_id: i64) -> Result<()>;
    pub async fn get_account_for_nick(&self, nick: &str) -> Result<Option<Account>>;
    pub async fn get_nicknames(&self, account_id: i64) -> Result<Vec<String>>;

    // Channels
    pub async fn register_channel(&self, name: &str, founder_id: i64) -> Result<()>;
    pub async fn get_channel(&self, name: &str) -> Result<Option<RegisteredChannel>>;
    pub async fn get_access_level(&self, channel: &str, account_id: i64) -> Result<i32>;
    pub async fn set_access_level(&self, channel: &str, account_id: i64, level: i32) -> Result<()>;
}
```

## Configuration

```toml
[services]
enabled = true

[services.nickserv]
# Enforce registered nicknames
enforce_nicks = true
# Seconds before enforcement
enforce_delay = 30
# Guest nick format when enforced
guest_format = "Guest{random}"
# Max nicknames per account
max_nicknames = 5

[services.chanserv]
enabled = true
# Max channels per account
max_channels = 10
# Expire unused channels after (0 = never)
expire_days = 0

[services.registration]
# Allow new account registration
enabled = true
# Require email address
require_email = false
# Require email verification
require_verification = false

[services.database]
# SQLite database path
path = "/var/lib/irc/services.db"
```

## Nickname Enforcement

When a user connects with a registered nickname:

```
┌────────────────────────────────────────────────────────────┐
│                  Nickname Enforcement Flow                  │
├────────────────────────────────────────────────────────────┤
│                                                            │
│  User connects as "alice" (registered nick)                │
│         │                                                  │
│         ▼                                                  │
│  ┌─────────────────────────────────────────────────────┐   │
│  │ Is user authenticated via SASL to account "alice"? │   │
│  └─────────────────────────────────────────────────────┘   │
│         │                     │                            │
│        Yes                   No                            │
│         │                     │                            │
│         ▼                     ▼                            │
│  ┌─────────────┐    ┌─────────────────────────────────┐    │
│  │   Allow     │    │ Start 30s timer                 │    │
│  │             │    │ Send: "Nick is registered..."   │    │
│  └─────────────┘    └─────────────────────────────────┘    │
│                              │                             │
│                       Timer expires                        │
│                       without IDENTIFY                     │
│                              │                             │
│                              ▼                             │
│                     ┌─────────────────┐                    │
│                     │ Force nick to   │                    │
│                     │ Guest12345      │                    │
│                     └─────────────────┘                    │
│                                                            │
└────────────────────────────────────────────────────────────┘
```

```rust
pub async fn check_nickname_enforcement(&self, client: &Client) {
    let nick = client.nick();

    // Check if nick is registered
    let Some(owner) = self.db.get_account_for_nick(&nick).await? else {
        return; // Not registered, allow
    };

    // Check if client is logged into the owning account
    if client.account() == Some(&owner.name) {
        return; // Logged in as owner, allow
    }

    // Start enforcement timer
    client.send_notice(
        "NickServ",
        &format!(
            "This nickname is registered. Please identify via /msg NickServ IDENTIFY <password> \
             or change your nickname within {} seconds.",
            self.config.enforce_delay
        )
    );

    // Schedule enforcement
    let client_id = client.id();
    let delay = Duration::from_secs(self.config.enforce_delay);

    tokio::spawn(async move {
        tokio::time::sleep(delay).await;

        // Re-check if still needs enforcement
        if let Some(client) = self.get_client(client_id) {
            if client.account().is_none() && client.nick() == nick {
                self.force_nick_change(&client).await;
            }
        }
    });
}

async fn force_nick_change(&self, client: &Client) {
    let guest_nick = self.generate_guest_nick();
    client.change_nick(&guest_nick).await;
    client.send_notice(
        "NickServ",
        &format!("Your nickname has been changed to {} (nick enforcement).", guest_nick)
    );
}
```

## Integration with SASL

SASL is the preferred authentication method. When a user authenticates via SASL:

1. User sends `AUTHENTICATE PLAIN` with credentials
2. Server verifies against accounts database
3. On success, user is logged in before completing registration
4. Nickname enforcement is automatically satisfied
5. Channel auto-op is applied on JOIN

```rust
pub async fn handle_sasl_success(&self, client: &mut Client, account: &str) {
    // Mark client as logged in
    client.set_account(account);

    // Send login confirmation
    client.send_numeric(
        900,
        &format!("{} {} :You are now logged in as {}", client.nick(), account, account)
    );

    // Check if current nick belongs to this account
    if let Some(owner) = self.db.get_account_for_nick(&client.nick()).await? {
        if owner.name != account {
            // Using someone else's registered nick
            self.check_nickname_enforcement(client).await;
        }
    }
}
```

## Auto-op on Join

When a logged-in user joins a registered channel:

```rust
pub async fn on_channel_join(&self, client: &Client, channel: &str) {
    let Some(account) = client.account() else {
        return; // Not logged in
    };

    let Some(reg_channel) = self.db.get_channel(channel).await? else {
        return; // Not registered
    };

    let level = self.db.get_access_level(channel, account).await?;

    // Apply appropriate mode based on access level
    let mode = match level {
        50..=100 => Some("+o"),  // Op
        10..=49 => Some("+v"),   // Voice
        _ => None,
    };

    if let Some(mode) = mode {
        self.set_mode(channel, mode, &client.nick()).await;
    }
}
```

## References

- [Ergo IRC Server](https://github.com/ergochat/ergo) - Built-in services reference
- [Atheme](https://atheme.github.io/) - External services
- [Anope](https://www.anope.org/) - External services
