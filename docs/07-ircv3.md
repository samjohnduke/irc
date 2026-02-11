# IRCv3 Extensions

A comprehensive analysis of [IRCv3 specifications](https://ircv3.net/irc/) and their relevance to this project.

## Overview

IRCv3 is a working group that develops extensions to the IRC protocol. Extensions are negotiated via capability (`CAP`) negotiation, allowing clients and servers to agree on supported features.

```
Client: CAP LS 302
Server: CAP * LS :sasl message-tags server-time echo-message batch ...
Client: CAP REQ :sasl message-tags server-time
Server: CAP * ACK :sasl message-tags server-time
Client: CAP END
```

## Priority Classification

| Priority | Meaning |
|----------|---------|
| **P1 - Essential** | Core functionality, implement early |
| **P2 - Important** | Significantly improves UX, implement in main phases |
| **P3 - Nice to Have** | Useful but not critical, implement if time permits |
| **P4 - Optional** | Niche use cases, defer or skip |
| **Skip** | Deprecated, problematic, or out of scope |

---

## P1 - Essential

### CAP (Capability Negotiation)

**Status:** Standard
**Specs:** [CAP](https://ircv3.net/specs/extensions/capability-negotiation), [cap-notify](https://ircv3.net/specs/extensions/capability-negotiation#cap-notify)

The foundation for all other IRCv3 features. Must implement first.

```
CAP LS 302              # List capabilities (version 302)
CAP REQ :cap1 cap2      # Request capabilities
CAP ACK :cap1 cap2      # Server acknowledges
CAP NAK :cap3           # Server denies
CAP END                 # Finish negotiation
CAP NEW :cap4           # New cap available (cap-notify)
CAP DEL :cap2           # Cap removed (cap-notify)
```

**Implementation notes:**
- Support `CAP LS 302` (version 302 format with values)
- Track enabled capabilities per client
- `cap-notify` allows dynamic capability changes

---

### SASL Authentication

**Status:** Standard
**Specs:** [SASL 3.1](https://ircv3.net/specs/extensions/sasl-3.1), [SASL 3.2](https://ircv3.net/specs/extensions/sasl-3.2)

Secure authentication during connection. Already documented in [03-security.md](03-security.md).

**Mechanisms to support:**
- `PLAIN` - Simple, requires TLS
- `SCRAM-SHA-256` - Challenge-response, no password sent
- `EXTERNAL` - Client certificate auth

---

### Message Tags

**Status:** Standard
**Spec:** [message-tags](https://ircv3.net/specs/extensions/message-tags)

Extends IRC message format with metadata. Required for many other extensions.

```
@tag1=value1;tag2;tag3=value3 :nick!user@host PRIVMSG #channel :Hello
```

**Tag types:**
- Server tags: Added by server (e.g., `time`, `msgid`)
- Client-only tags: Prefixed with `+`, passed through by server

**Implementation:**
```rust
pub struct Message {
    pub tags: Option<HashMap<String, Option<String>>>,
    pub prefix: Option<Prefix>,
    pub command: Command,
}

// Parse: @tag1=value;tag2 :prefix COMMAND params
fn parse_tags(input: &str) -> HashMap<String, Option<String>> {
    input.split(';')
        .map(|tag| {
            if let Some((key, value)) = tag.split_once('=') {
                (key.to_string(), Some(unescape_tag_value(value)))
            } else {
                (tag.to_string(), None)
            }
        })
        .collect()
}
```

---

### Server Time

**Status:** Standard
**Spec:** [server-time](https://ircv3.net/specs/extensions/server-time)

Accurate timestamps on messages. Essential for history and bouncers.

```
@time=2024-01-15T14:32:00.000Z :alice!a@host PRIVMSG #channel :Hello
```

**Format:** ISO 8601 with milliseconds: `YYYY-MM-DDThh:mm:ss.sssZ`

---

### Message IDs

**Status:** Standard
**Spec:** [message-ids](https://ircv3.net/specs/extensions/message-ids)

Unique identifier for each message. Required for replies, reactions, history.

```
@msgid=abc123xyz :alice!a@host PRIVMSG #channel :Hello
```

**Implementation notes:**
- Generate unique IDs server-side (UUID or similar)
- Store with message for history retrieval
- Client uses for `+reply` and `+react` tags

---

### Echo Message

**Status:** Standard
**Spec:** [echo-message](https://ircv3.net/specs/extensions/echo-message)

Server echoes client's messages back with server-added tags.

```
Client: PRIVMSG #channel :Hello
Server: @msgid=abc;time=... :yournick!you@host PRIVMSG #channel :Hello
```

**Benefits:**
- Client gets `msgid` and `time` for own messages
- Confirms message was sent
- Essential for consistent history

---

### Batch

**Status:** Standard
**Spec:** [batch](https://ircv3.net/specs/extensions/batch)

Groups related messages together.

```
:server BATCH +abc chathistory #channel
@batch=abc :alice PRIVMSG #channel :Old message 1
@batch=abc :bob PRIVMSG #channel :Old message 2
:server BATCH -abc
```

**Batch types we'll support:**
- `chathistory` - History replay
- `labeled-response` - Grouped responses
- `netjoin` / `netsplit` - Network events

---

### Labeled Response

**Status:** Standard
**Spec:** [labeled-response](https://ircv3.net/specs/extensions/labeled-response)

Links commands to their responses.

```
Client: @label=abc123 WHO #channel
Server: @label=abc123 :server BATCH +xyz labeled-response
Server: @batch=xyz :server 352 ...
Server: @batch=xyz :server 315 ...
Server: :server BATCH -xyz
```

**Use cases:**
- Bouncer routes responses to correct client
- Client correlates async responses
- Essential for modern client UX

---

## P2 - Important

### CHATHISTORY

**Status:** Draft
**Spec:** [chathistory](https://ircv3.net/specs/extensions/chathistory)

Request message history from server.

```
CHATHISTORY LATEST #channel * 50
CHATHISTORY BEFORE #channel msgid=abc123 100
CHATHISTORY AFTER #channel timestamp=2024-01-15T00:00:00Z 50
CHATHISTORY BETWEEN #channel timestamp=... timestamp=... 100
CHATHISTORY AROUND #channel msgid=xyz 50
CHATHISTORY TARGETS timestamp=2024-01-01T00:00:00Z timestamp=2024-01-15T00:00:00Z 20
```

**Implementation:**
```rust
pub struct ChatHistoryStore {
    // Ring buffer or database-backed
    messages: HashMap<Target, VecDeque<StoredMessage>>,
}

pub struct StoredMessage {
    pub msgid: String,
    pub time: DateTime<Utc>,
    pub sender: Prefix,
    pub target: String,
    pub text: String,
}

impl ChatHistoryStore {
    pub fn query_before(&self, target: &str, reference: &str, limit: usize)
        -> Vec<StoredMessage>;
    pub fn query_after(&self, target: &str, reference: &str, limit: usize)
        -> Vec<StoredMessage>;
    pub fn query_latest(&self, target: &str, limit: usize)
        -> Vec<StoredMessage>;
}
```

**Storage options:**
- In-memory ring buffer (simple, volatile)
- SQLite (persistent, queryable)
- PostgreSQL (scalable, production)

---

### Away Notify

**Status:** Standard
**Spec:** [away-notify](https://ircv3.net/specs/extensions/away-notify)

Real-time away status notifications.

```
:alice!a@host AWAY :Gone for lunch
:alice!a@host AWAY                   # Returned (no message)
```

Clients receive AWAY for users in shared channels, eliminating need to poll.

---

### Account Notify

**Status:** Standard
**Spec:** [account-notify](https://ircv3.net/specs/extensions/account-notify)

Notification when users log in/out of accounts.

```
:alice!a@host ACCOUNT alice    # Logged into account "alice"
:alice!a@host ACCOUNT *        # Logged out
```

---

### Account Tag

**Status:** Standard
**Spec:** [account-tag](https://ircv3.net/specs/extensions/account-tag)

Include account name in message tags.

```
@account=alice :alice!a@host PRIVMSG #channel :Hello
```

Useful for access control, highlighting registered users.

---

### Extended Join

**Status:** Standard
**Spec:** [extended-join](https://ircv3.net/specs/extensions/extended-join)

Include account and realname in JOIN.

```
:alice!a@host JOIN #channel alice :Alice Smith
```

Versus standard:
```
:alice!a@host JOIN #channel
```

---

### Multi-Prefix

**Status:** Standard
**Spec:** [multi-prefix](https://ircv3.net/specs/extensions/multi-prefix)

Show all user prefixes, not just highest.

```
:server 353 you = #channel :@+alice @bob +carol dave
```

Standard IRC only shows highest (`@alice` not `@+alice`).

---

### CHGHOST

**Status:** Standard
**Spec:** [chghost](https://ircv3.net/specs/extensions/chghost)

Notify when user's host changes (e.g., vhost applied).

```
:alice!olduser@oldhost CHGHOST newuser newhost
```

---

### SETNAME

**Status:** Standard
**Spec:** [setname](https://ircv3.net/specs/extensions/setname)

Change realname after connecting.

```
Client: SETNAME :My New Real Name
Server: :yournick!user@host SETNAME :My New Real Name
```

---

### Standard Replies

**Status:** Standard
**Spec:** [standard-replies](https://ircv3.net/specs/extensions/standard-replies)

Consistent format for informational messages.

```
FAIL COMMAND CODE :Human readable message
WARN COMMAND CODE :Warning message
NOTE COMMAND CODE :Informational message
```

Better than ad-hoc NOTICEs for machine parsing.

---

### STS (Strict Transport Security)

**Status:** Standard
**Spec:** [sts](https://ircv3.net/specs/extensions/sts)

Force TLS upgrade, prevent downgrade attacks.

```
CAP * LS :sts=port=6697,duration=2592000
```

Client must reconnect on TLS port and remember for `duration` seconds.

---

## P3 - Nice to Have

### Typing Indicator

**Status:** Draft (client-only tag)
**Spec:** [typing](https://ircv3.net/specs/client-tags/typing)

Show when users are typing.

```
@+typing=active TAGMSG #channel
@+typing=paused TAGMSG #channel
@+typing=done TAGMSG #channel
```

**Implementation notes:**
- Client-only tag (server passes through)
- Rate limit to avoid spam
- Privacy: make optional

---

### Reply

**Status:** Draft (client-only tag)
**Spec:** [reply](https://ircv3.net/specs/client-tags/reply)

Mark message as reply to another.

```
@+reply=abc123 PRIVMSG #channel :I agree with that!
```

References `msgid` of original message.

---

### React

**Status:** Draft (client-only tag)
**Spec:** [react](https://ircv3.net/specs/client-tags/react)

Emoji reactions to messages.

```
@+reply=abc123;+react=👍 TAGMSG #channel
```

**Implementation notes:**
- Uses TAGMSG (no text body)
- Single emoji per reaction message
- Client aggregates for display

---

### Read Marker

**Status:** Draft
**Spec:** [read-marker](https://ircv3.net/specs/extensions/read-marker)

Sync read position across clients.

```
MARKREAD #channel timestamp=2024-01-15T14:32:00.000Z
:server MARKREAD #channel timestamp=2024-01-15T14:32:00.000Z
```

Useful for multi-device users.

---

### Monitor

**Status:** Standard
**Spec:** [monitor](https://ircv3.net/specs/extensions/monitor)

Watch for users coming online/offline.

```
MONITOR + alice,bob,carol
MONITOR - alice
MONITOR C                    # Clear list
MONITOR L                    # List monitored
MONITOR S                    # Status of all
```

Server sends `730` (online) and `731` (offline) numerics.

---

### WHOX

**Status:** Standard
**Spec:** [whox](https://ircv3.net/specs/extensions/whox)

Extended WHO with field selection.

```
WHO #channel %tcuhnfar
```

Flags specify which fields to return. More efficient than standard WHO.

---

### Userhost in Names

**Status:** Standard
**Spec:** [userhost-in-names](https://ircv3.net/specs/extensions/userhost-in-names)

Include full hostmask in NAMES reply.

```
:server 353 you = #channel :@alice!a@host bob!b@host
```

Reduces need for separate WHO queries.

---

### Bot Mode

**Status:** Standard
**Spec:** [bot-mode](https://ircv3.net/specs/extensions/bot-mode)

Mark clients as bots.

```
MODE yournick +B
```

Servers may tag messages from bots:
```
@bot :botname!b@host PRIVMSG #channel :Automated message
```

---

### Invite Notify

**Status:** Standard
**Spec:** [invite-notify](https://ircv3.net/specs/extensions/invite-notify)

Channel ops see when someone is invited.

```
:alice!a@host INVITE bob #channel
```

Sent to ops when alice invites bob.

---

### WebSocket

**Status:** Draft
**Spec:** [websocket](https://ircv3.net/specs/extensions/websocket)

IRC over WebSocket for browser clients.

```
wss://irc.example.com/
```

**Implementation notes:**
- Wrap IRC messages in WebSocket frames
- Binary or text frames
- Enables web-based clients

---

## P4 - Optional / Defer

### Chathistory Persistence (Database)

Full database-backed message storage. Complex, defer to later phase.

### Message Redaction

**Spec:** [message-redaction](https://ircv3.net/specs/extensions/message-redaction)

Delete messages from history. Moderation feature, implement with chathistory.

### Channel Rename

**Spec:** [channel-rename](https://ircv3.net/specs/extensions/channel-rename)

Rename channels without closing. Niche use case.

### Multiline Messages

**Spec:** [multiline](https://ircv3.net/specs/extensions/multiline)

Messages spanning multiple lines. Complex batching.

### Pre-Away

**Spec:** [pre-away](https://ircv3.net/specs/extensions/pre-away)

Set away during registration. Minor feature.

### Network Icon

**Spec:** [network-icon](https://ircv3.net/specs/extensions/network-icon)

ISUPPORT token for network branding. Cosmetic.

### Extended ISUPPORT

**Spec:** [extended-isupport](https://ircv3.net/specs/extensions/extended-isupport)

Request ISUPPORT before registration. Edge case.

---

## Skip

### STARTTLS

**Status:** Deprecated
**Reason:** Use implicit TLS (port 6697) + STS instead.

### No Implicit Names

**Status:** Draft
**Reason:** Niche optimization, low value.

---

## Implementation Phases

### Phase 4: Core IRCv3

```
CAP negotiation
├── cap-notify
├── sasl (3.1, 3.2)
├── message-tags
├── server-time
├── msgid
├── echo-message
├── batch
└── labeled-response
```

### Phase 5: Enhanced Features

```
Account features
├── account-notify
├── account-tag
└── extended-join

User tracking
├── away-notify
├── chghost
├── setname
└── multi-prefix

Infrastructure
├── standard-replies
├── sts
└── monitor
```

### Phase 6: Modern Experience

```
History
├── chathistory (in-memory)
└── chathistory (persistent)

Client tags
├── +typing
├── +reply
└── +react

Advanced
├── read-marker
├── whox
├── userhost-in-names
└── websocket
```

---

## Capability Advertisement

Example ISUPPORT and CAP output:

```
CAP * LS :
  account-notify account-tag away-notify batch cap-notify chghost
  echo-message extended-join labeled-response message-tags monitor
  multi-prefix sasl=PLAIN,SCRAM-SHA-256 server-time setname
  userhost-in-names draft/chathistory

:server 005 nick
  ACCOUNTEXTBAN=a AWAYLEN=390 BOT=B CASEMAPPING=ascii CHANLIMIT=#:100
  CHANMODES=Ibe,k,l,imnstp CHANNELLEN=64 CHANTYPES=# ELIST=CMNTU
  EXCEPTS EXTBAN=,a HOSTLEN=64 INVEX KICKLEN=390 MAXLIST=beI:100
  MAXTARGETS=4 MODES=4 NAMELEN=128 NETWORK=ExampleNet NICKLEN=30
  PREFIX=(ov)@+ SAFELIST STATUSMSG=@+ TOPICLEN=390 USERLEN=10
  FILEHOST=https://irc.example.com/upload :are supported by this server
```

---

## Client Capability Request Strategy

```rust
/// Capabilities to request in order of preference
const DESIRED_CAPS: &[&str] = &[
    // Essential
    "sasl",
    "message-tags",
    "server-time",
    "msgid",
    "echo-message",
    "batch",
    "labeled-response",
    "cap-notify",

    // Important
    "account-notify",
    "account-tag",
    "away-notify",
    "extended-join",
    "multi-prefix",
    "chghost",
    "setname",

    // Nice to have
    "draft/chathistory",
    "monitor",
    "userhost-in-names",
];

impl Client {
    async fn negotiate_capabilities(&mut self) -> Result<HashSet<String>> {
        self.send("CAP LS 302").await?;

        let available = self.receive_cap_ls().await?;

        let to_request: Vec<_> = DESIRED_CAPS.iter()
            .filter(|cap| available.contains(**cap))
            .collect();

        if !to_request.is_empty() {
            self.send(&format!("CAP REQ :{}", to_request.join(" "))).await?;
            let acked = self.receive_cap_ack().await?;
            // Handle partial ACK/NAK
        }

        // SASL if available and configured
        if available.contains("sasl") && self.config.sasl.is_some() {
            self.perform_sasl().await?;
        }

        self.send("CAP END").await?;
        Ok(enabled)
    }
}
```

---

## References

- [IRCv3 Specifications](https://ircv3.net/irc/)
- [IRCv3 Support Tables](https://ircv3.net/software/)
- [Modern IRC Docs](https://modern.ircdocs.horse/)
- [Ergo IRCv3 Support](https://github.com/ergochat/ergo)
