# Extensions: Rich Text and Media

Exploring modern features beyond core IRC: text formatting, image sharing, and making the experience feel more like Discord while respecting IRC's nature.

## The Tension

IRC is fundamentally a text protocol from 1988. Modern chat apps (Discord, Slack, Matrix) have:
- Rich text formatting (markdown)
- Inline images and media
- File uploads
- Reactions/emoji
- Threads

We can add some of these, but should be thoughtful about:
1. **Compatibility** - Will our clients work with other servers? Will other clients work with our server?
2. **Complexity** - Each feature adds code to maintain
3. **Philosophy** - IRC's simplicity is a feature, not a bug

## Text Formatting

### What IRC Already Has

IRC has [formatting control codes](https://modern.ircdocs.horse/formatting) using ASCII control characters:

| Character | Code | Effect |
|-----------|------|--------|
| `0x02` | Ctrl+B | **Bold** |
| `0x1D` | Ctrl+I | *Italic* |
| `0x1F` | Ctrl+U | Underline |
| `0x1E` | | ~~Strikethrough~~ |
| `0x11` | | `Monospace` |
| `0x03` | Ctrl+C | Color (followed by color codes) |
| `0x04` | | Hex color (followed by RRGGBB) |
| `0x0F` | Ctrl+O | Reset all formatting |

Example raw message:
```
This is \x02bold\x02 and \x1Ditalic\x1D text
```

These are widely supported but awkward to type.

### Option 1: IRC Codes Only (Conservative)

Support the existing IRC formatting codes. No markdown.

**Pros:**
- Maximum compatibility
- Simple implementation
- Works with all servers/clients

**Cons:**
- Hard to type control characters
- No code blocks, quotes, lists
- Feels dated

### Option 2: Markdown Rendering (Client-Side Only)

Client renders markdown-like syntax but sends plain text.

```
User types:    **bold** and `code`
Client shows:  bold (styled) and code (styled)
Wire format:   **bold** and `code` (plain text)
Other clients: **bold** and `code` (literal asterisks)
```

**Pros:**
- Better UX for our users
- No server changes
- Graceful degradation

**Cons:**
- Other clients see raw markdown
- Ambiguity (is `*word*` formatting or literal?)
- Can't distinguish intentional asterisks

### Option 3: Markdown to IRC Codes (Translation)

Client translates markdown to IRC control codes before sending.

```
User types:    **bold** and *italic*
Wire format:   \x02bold\x02 and \x1Ditalic\x1D
Other clients: bold (styled) and italic (styled)
```

**Pros:**
- Interoperable - other clients see formatting
- User-friendly input
- Uses existing standard

**Cons:**
- Lossy - no code blocks, quotes, lists in IRC codes
- One-way - incoming IRC codes aren't markdown

### Option 4: Full Markdown (Our Ecosystem Only)

Send markdown as-is, render in our clients.

**Pros:**
- Full markdown support
- Code blocks, quotes, lists, links

**Cons:**
- Other clients see raw markdown
- Creates an "our clients only" experience
- Against IRC interop philosophy

### Recommendation: Hybrid Approach

1. **Input**: Accept both markdown and IRC codes
2. **Translation**: Convert simple markdown to IRC codes when sending
   - `**bold**` → `\x02bold\x02`
   - `*italic*` → `\x1Ditalic\x1D`
   - `` `code` `` → `\x11code\x11`
3. **Extended**: Keep complex markdown (code blocks, quotes) as plain text
4. **Rendering**: Render incoming IRC codes as styled text
5. **Option**: User preference to disable markdown translation

```rust
pub fn markdown_to_irc(input: &str) -> String {
    // Simple patterns only - avoid ambiguity
    let mut output = input.to_string();

    // Bold: **text** → \x02text\x02
    output = BOLD_RE.replace_all(&output, "\x02$1\x02").to_string();

    // Italic: *text* (but not **) → \x1Dtext\x1D
    output = ITALIC_RE.replace_all(&output, "\x1D$1\x1D").to_string();

    // Code: `text` → \x11text\x11
    output = CODE_RE.replace_all(&output, "\x11$1\x11").to_string();

    output
}
```

## Image and File Sharing

### The Problem

IRC has no built-in way to share images. Historical approaches:

| Method | Era | Issues |
|--------|-----|--------|
| DCC | 1990s | Peer-to-peer, NAT problems, security |
| Paste URLs | 2000s | Requires external hosting |
| Base64 in messages | Never worked | Floods channel, blocks connection |

### Modern Approaches

#### Approach 1: URL Previews (Client-Side)

Client detects image URLs and shows inline previews.

```
User sends:    Check this out: https://example.com/photo.jpg
Wire format:   Check this out: https://example.com/photo.jpg
Our client:    Check this out: [inline image preview]
Other clients: Check this out: https://example.com/photo.jpg
```

**Implementation:**
```rust
pub async fn fetch_preview(url: &str) -> Option<Preview> {
    // Check if URL points to image
    if is_image_url(url) {
        let thumbnail = fetch_and_resize(url, MAX_PREVIEW_SIZE).await?;
        return Some(Preview::Image { url, thumbnail });
    }

    // Check for OpenGraph/Twitter cards
    if let Some(meta) = fetch_og_metadata(url).await {
        return Some(Preview::Link {
            title: meta.title,
            description: meta.description,
            image: meta.image,
        });
    }

    None
}
```

**Pros:**
- No server changes
- Works with any IRC server
- Graceful degradation

**Cons:**
- User must host images somewhere
- Privacy (client fetches external URLs)
- No persistence guarantee

#### Approach 2: Server-Provided Upload (FILEHOST)

Server provides upload endpoint, returns URL to share.

```
┌────────┐    POST /upload     ┌────────┐
│ Client │ ─────────────────► │ Server │
│        │ ◄───────────────── │        │
│        │   URL: https://... │        │
└────────┘                    └────────┘
     │
     │ PRIVMSG #channel :https://irc.example.com/f/abc123.jpg
     ▼
┌────────┐
│ Other  │  Sees URL, can preview
│ Client │
└────────┘
```

This is the [FILEHOST proposal](https://github.com/ircv3/ircv3-ideas/issues/100) being implemented by soju/goguma.

**Implementation:**
```rust
// Server-side upload endpoint
async fn handle_upload(
    auth: AuthenticatedUser,
    file: Multipart,
) -> Result<UploadResponse, Error> {
    // Validate file type and size
    let validated = validate_upload(&file, &config.upload)?;

    // Store file
    let id = generate_id();
    let path = storage.store(&id, validated.data).await?;

    // Generate URL
    let url = format!("{}/f/{}", config.public_url, id);

    Ok(UploadResponse { url })
}

// Configuration
pub struct UploadConfig {
    pub enabled: bool,
    pub max_size: usize,           // e.g., 10MB
    pub allowed_types: Vec<Mime>,  // image/*, video/*, etc.
    pub storage_path: PathBuf,
    pub retention_days: u32,       // Auto-delete after N days
}
```

**Pros:**
- Integrated experience
- Server controls content
- Works with all clients (they just see URLs)

**Cons:**
- Storage costs
- Moderation burden
- Server complexity

#### Approach 3: libp2p / IPFS (Decentralized)

Your interesting suggestion - use content-addressed storage.

```
┌────────┐                      ┌────────┐
│ Client │ ─── Add to IPFS ───►│  IPFS  │
│   A    │ ◄── CID: Qm... ──── │Network │
└────────┘                      └────────┘
     │
     │ PRIVMSG #channel :ipfs://Qm.../photo.jpg
     ▼
┌────────┐                      ┌────────┐
│ Client │ ─── Fetch CID ─────►│  IPFS  │
│   B    │ ◄── Image data ──── │Network │
└────────┘                      └────────┘
```

**Pros:**
- Decentralized - no single point of failure
- Content-addressed - immutable, verifiable
- Peer-to-peer - reduces server bandwidth
- Aligns with IRC's federated philosophy

**Cons:**
- Adds significant dependency (libp2p/IPFS)
- Client complexity
- Content availability depends on peers
- IPFS gateway needed for non-IPFS clients
- NAT traversal challenges

**Hybrid Implementation:**
```rust
pub struct MediaSharing {
    // Local IPFS node
    ipfs: IpfsClient,

    // Fallback HTTP gateway for URL generation
    gateway: String,  // e.g., "https://ipfs.io/ipfs/"
}

impl MediaSharing {
    pub async fn share_file(&self, path: &Path) -> Result<String, Error> {
        // Add to IPFS
        let cid = self.ipfs.add_file(path).await?;

        // Pin locally
        self.ipfs.pin(&cid).await?;

        // Return gateway URL for compatibility
        Ok(format!("{}{}", self.gateway, cid))
    }

    pub async fn fetch_media(&self, url: &str) -> Result<Bytes, Error> {
        if let Some(cid) = parse_ipfs_url(url) {
            // Fetch via IPFS
            self.ipfs.cat(&cid).await
        } else {
            // Fallback to HTTP
            reqwest::get(url).await?.bytes().await
        }
    }
}
```

### Recommendation: Layered Approach

**Phase 1 (Core):** URL detection and preview
- Client-side only
- No server changes
- Works everywhere

**Phase 2 (Server Upload):** FILEHOST implementation
- Simple HTTP upload endpoint
- Server stores files
- Returns shareable URLs

**Phase 3 (Optional/Experimental):** IPFS integration
- For users who want decentralization
- Client-side feature
- Falls back to gateway URLs

## Reactions and Emoji

Discord-style reactions on messages.

### The Challenge

IRC messages don't have stable IDs. You can't say "react to message X" because there's no X.

### IRCv3 Solution: Message IDs + TAGMSG

IRCv3 adds:
- `msgid` tag - unique ID per message
- `TAGMSG` - send metadata without text
- `+react` client tag (proposed)

```
Server: @msgid=abc123 :alice PRIVMSG #channel :Hello!
Client: @+react=👍;msgid=abc123 TAGMSG #channel
```

**Pros:**
- Standardized approach
- Works across compliant clients

**Cons:**
- Requires IRCv3 support
- Non-IRCv3 clients see nothing
- Not widely implemented yet

### Simpler Alternative: Convention

Just type reactions as text:

```
<alice> Check out my new project!
<bob> 👍
<charlie> 🎉
```

Many communities already do this.

## Discord-Like Feel: What's Achievable?

| Feature | IRC Equivalent | Gap |
|---------|---------------|-----|
| Rich text | IRC codes + markdown translation | Small |
| Inline images | URL preview | Small |
| File upload | FILEHOST / IPFS | Medium |
| Reactions | Text emoji / IRCv3 | Medium |
| Threads | Channels / `+draft/reply` | Large |
| Voice/Video | None (out of scope) | N/A |

### UI/UX Improvements (Client-Side)

Make it feel modern without changing the protocol:

1. **Message grouping** - Collapse consecutive messages from same user
2. **Smart timestamps** - "2 minutes ago" not "14:32:01"
3. **User avatars** - Generate from nick or fetch via Gravatar
4. **Typing indicators** - IRCv3 `+typing` tag
5. **Read markers** - Track locally, sync via account
6. **Link previews** - Fetch OpenGraph metadata
7. **Emoji picker** - Unicode emoji, custom shortcodes

## Summary: Proposed Extensions

| Extension | Priority | Approach |
|-----------|----------|----------|
| Markdown input | High | Translate to IRC codes |
| URL previews | High | Client-side fetch |
| File upload | Medium | FILEHOST endpoint |
| IRC code rendering | High | Parse and style |
| Emoji | Medium | Unicode + picker |
| IPFS sharing | Low | Optional client feature |
| Reactions | Low | IRCv3 or text convention |

## Configuration

```toml
[formatting]
# Enable markdown-to-IRC translation
markdown_enabled = true
# Render incoming IRC formatting codes
render_irc_codes = true

[media]
# Enable URL preview fetching
url_previews = true
# Max preview image size (KB)
max_preview_size = 500
# Allowed preview domains (empty = all)
preview_domains = []

[upload]
# Enable file upload (server)
enabled = true
max_size_mb = 10
allowed_types = ["image/*", "text/*", "application/pdf"]
retention_days = 30

[experimental]
# Enable IPFS integration
ipfs_enabled = false
ipfs_gateway = "https://ipfs.io/ipfs/"
```

## References

- [IRC Formatting](https://modern.ircdocs.horse/formatting) - Control codes spec
- [IRCv3 Message IDs](https://ircv3.net/specs/extensions/message-ids) - For reactions
- [FILEHOST Discussion](https://github.com/ircv3/ircv3-ideas/issues/100)
- [Image Messages Discussion](https://github.com/ircv3/ircv3-specifications/issues/273)
- [libp2p](https://libp2p.io/) / [IPFS](https://ipfs.io/)
