# irc-cli

A terminal-based IRC client with a modern TUI interface built on `ratatui`. Provides a full-featured IRC experience in the terminal.

## Responsibilities

- Render a responsive terminal UI
- Handle keyboard input and commands
- Display channel/query buffers with scrollback
- Show user lists and channel info
- Support multiple server connections
- Provide a familiar IRC command interface (/, commands)

## UI Layout

```
┌─ irc-cli ─────────────────────────────────────────────────────────┐
│ [libera] #rust | 342 users | Topic: Pair your braces...          │
├───────────────────────────────────────────────────────────────────┤
│ 14:32 <alice> Has anyone tried the new async traits?             │
│ 14:32 <bob> Yeah, they're great for this use case                │
│ 14:33 <charlie> @alice check out the examples in tokio           │
│ 14:33 --> carol has joined #rust                                  │
│ 14:33 <alice> thanks! will do                                     │
│ 14:34 <-- dave has quit (Ping timeout)                           │
│                                                                   │
│                                                                   │
│                                                                   │
├─────────────────────────────────────────────────────────┬─────────┤
│ [#rust] [#linux] [#vim] (status)                        │ @alice  │
│                                                         │ @bob    │
│                                                         │  carol  │
│                                                         │  charlie│
├─────────────────────────────────────────────────────────┴─────────┤
│ > /msg alice hey, that tokio link?_                               │
└───────────────────────────────────────────────────────────────────┘
```

### Layout Components

1. **Title Bar**: Server name, current channel, user count, topic
2. **Message Area**: Scrollable message history with timestamps
3. **Buffer Tabs**: Switch between channels/queries/status
4. **Nick List**: Channel members (collapsible)
5. **Input Line**: Command/message input with prompt

## Key Bindings

### Navigation

| Key | Action |
|-----|--------|
| `Ctrl+N` / `Alt+→` | Next buffer |
| `Ctrl+P` / `Alt+←` | Previous buffer |
| `Alt+1-9` | Jump to buffer 1-9 |
| `Alt+0` | Jump to buffer 10 |
| `Page Up` | Page up in message history |
| `Page Down` | Page down in message history |
| `Home` | Scroll to top |
| `End` | Scroll to bottom |
| `Tab` | Cycle nick list focus / nick completion |

### Input

| Key | Action |
|-----|--------|
| `Enter` | Send message / execute command |
| `Ctrl+A` | Move cursor to start |
| `Ctrl+E` | Move cursor to end |
| `Ctrl+W` | Delete word backwards |
| `Ctrl+K` | Delete to end of line |
| `Ctrl+U` | Delete to start of line |
| `↑` / `↓` | Input history |
| `Ctrl+R` | Reverse search input history |

### Windows

| Key | Action |
|-----|--------|
| `Ctrl+L` | Toggle nick list visibility |
| `Ctrl+T` | Toggle buffer list visibility |
| `F1` | Help overlay |
| `Ctrl+Q` | Quit |

## Commands

All commands start with `/`. Unrecognized input is sent as a message.

### Connection

| Command | Description |
|---------|-------------|
| `/connect <server>` | Connect to a configured server |
| `/disconnect [message]` | Disconnect from current server |
| `/server <host> [port]` | Quick connect to a server |
| `/quit [message]` | Quit IRC (all servers) |

### Channels

| Command | Description |
|---------|-------------|
| `/join <channel> [key]` | Join a channel |
| `/part [channel] [message]` | Leave current or specified channel |
| `/topic [text]` | View or set topic |
| `/names` | Refresh nick list |
| `/kick <nick> [reason]` | Kick a user |
| `/invite <nick>` | Invite user to current channel |
| `/mode <modes>` | Set channel modes |
| `/list [pattern]` | List channels on server |

### Messaging

| Command | Description |
|---------|-------------|
| `/msg <target> <message>` | Send private message |
| `/notice <target> <message>` | Send notice |
| `/me <action>` | Send action to current buffer |
| `/query <nick>` | Open query buffer |
| `/ctcp <nick> <command>` | Send CTCP |

### User

| Command | Description |
|---------|-------------|
| `/nick <newnick>` | Change nickname |
| `/away [message]` | Set/unset away status |
| `/whois <nick>` | Query user info |
| `/who <mask>` | List matching users |

### UI

| Command | Description |
|---------|-------------|
| `/window <n>` | Switch to window n |
| `/close` | Close current buffer |
| `/clear` | Clear current buffer |
| `/set <key> [value]` | View/change settings |
| `/help [command]` | Show help |

### Raw

| Command | Description |
|---------|-------------|
| `/raw <command>` | Send raw IRC command |
| `/quote <command>` | Alias for /raw |

## Message Formatting

### Incoming Messages

```
14:32 <alice> Regular message
14:32 <@bob> Op message (prefix shown)
14:32 <+charlie> Voiced message
14:33 --> carol (carol@host) has joined #rust
14:33 <-- dave has left #rust (Goodbye)
14:33 <-- eve has quit (Ping timeout: 180 seconds)
14:34 *** alice is now known as alice_away
14:34 *** bob sets mode +o charlie
14:35 * alice waves hello
```

### Colors

- **Timestamps**: Dim gray
- **Nicknames**: Consistent color per nick (hash-based)
- **Own nick**: Highlighted when mentioned
- **Joins/Parts**: Dim green/red
- **Notices**: Dim magenta
- **Actions**: Italic cyan
- **Errors**: Red

## Configuration

```toml
# ~/.config/irc/cli.toml

[ui]
# Show timestamps
timestamps = true
timestamp_format = "%H:%M"

# Show join/part messages
show_joins = true
show_parts = true
show_quits = true

# Nick list position: "right", "left", "hidden"
nick_list = "right"
nick_list_width = 16

# Buffer tabs position: "top", "bottom", "hidden"
buffer_tabs = "bottom"

# Message scrollback limit per buffer
scrollback_lines = 10000

# Input history size
input_history_size = 500

[colors]
# Color theme: "default", "solarized-dark", "solarized-light", "nord"
theme = "default"

# Highlight words (regex patterns)
highlights = ["\\byournick\\b", "\\balert\\b"]
highlight_color = "yellow"

[notifications]
# Desktop notifications
enabled = true
on_mention = true
on_private = true
on_highlight = true

[logging]
# Log messages to files
enabled = false
path = "~/.local/share/irc/logs"
format = "%Y-%m-%d.log"
```

## Internal Architecture

```
┌───────────────────────────────────────────────────────────────┐
│                          irc-cli                              │
├───────────────────────────────────────────────────────────────┤
│  ┌─────────────┐     ┌─────────────┐     ┌─────────────┐      │
│  │    Input    │────▶│   Command   │────▶│   Client    │      │
│  │   Handler   │     │   Parser    │     │   Manager   │      │
│  └─────────────┘     └─────────────┘     └──────┬──────┘      │
│                                                  │            │
│         ┌────────────────────────────────────────┘            │
│         │                                                     │
│         ▼                                                     │
│  ┌─────────────┐     ┌─────────────┐     ┌─────────────┐      │
│  │    Event    │◀────│    App      │────▶│   Render    │      │
│  │    Loop     │     │   State     │     │   (TUI)     │      │
│  └─────────────┘     └─────────────┘     └─────────────┘      │
│                                                               │
└───────────────────────────────────────────────────────────────┘
```

### App State

```rust
pub struct App {
    /// IRC client manager
    client_manager: ClientManager,

    /// All buffers
    buffers: Vec<Buffer>,

    /// Currently active buffer index
    active_buffer: usize,

    /// Input line state
    input: InputState,

    /// Input history
    history: VecDeque<String>,

    /// UI state
    ui: UiState,

    /// Configuration
    config: CliConfig,

    /// Whether to quit
    should_quit: bool,
}

pub struct Buffer {
    /// Buffer type and identity
    pub kind: BufferKind,

    /// Server this buffer belongs to
    pub server: Option<ServerId>,

    /// Messages in this buffer
    pub messages: VecDeque<DisplayMessage>,

    /// Scroll position (from bottom)
    pub scroll: usize,

    /// Unread count
    pub unread: usize,

    /// Has unread highlights
    pub highlighted: bool,
}

pub enum BufferKind {
    /// Server status buffer
    Status { server: ServerId },

    /// Channel buffer
    Channel { name: String },

    /// Private message buffer
    Query { nick: String },
}

pub struct InputState {
    /// Current input text
    pub text: String,

    /// Cursor position
    pub cursor: usize,

    /// History browsing position
    pub history_index: Option<usize>,

    /// Nick completion state
    pub completion: Option<CompletionState>,
}
```

## Internal Structure

```
irc-cli/
├── Cargo.toml
└── src/
    ├── main.rs          # Entry point, argument parsing
    ├── app.rs           # App struct and event loop
    ├── input.rs         # Input handling, key bindings
    ├── command.rs       # Command parsing and execution
    ├── buffer.rs        # Buffer management
    ├── render.rs        # TUI rendering
    ├── widgets/
    │   ├── mod.rs
    │   ├── messages.rs  # Message list widget
    │   ├── input.rs     # Input line widget
    │   ├── nicklist.rs  # Nick list widget
    │   ├── tabs.rs      # Buffer tabs widget
    │   └── titlebar.rs  # Title bar widget
    ├── config.rs        # Configuration
    ├── colors.rs        # Color schemes
    └── completion.rs    # Nick/command completion
```

## Dependencies

```toml
[dependencies]
irc-client-lib = { path = "../irc-client-lib" }
tokio = { version = "1", features = ["full"] }
ratatui = { version = "0.29", features = ["crossterm"] }
serde = { version = "1", features = ["derive"] }
toml = "0.8"
directories = "5"
tracing = "0.1"
tracing-subscriber = "0.3"
clap = { version = "4", features = ["derive"] }
chrono = "0.4"
unicode-width = "0.2"
regex = "1"
```

## Event Loop

```rust
async fn run_app(mut app: App) -> Result<()> {
    let mut terminal = setup_terminal()?;

    loop {
        // Render
        terminal.draw(|frame| app.render(frame))?;

        // Handle events
        tokio::select! {
            // Terminal input
            Some(event) = read_crossterm_event() => {
                app.handle_input(event)?;
            }

            // IRC events
            Some(event) = app.client_manager.events().recv() => {
                app.handle_irc_event(event);
            }
        }

        if app.should_quit {
            break;
        }
    }

    restore_terminal(terminal)?;
    Ok(())
}
```

## Open Questions

1. **Mouse Support**: Enable click-to-focus buffers, nick list?
   - Recommendation: Yes, optional via config

2. **Split Windows**: Support horizontal/vertical splits?
   - Recommendation: Defer, single buffer with tabs is simpler

3. **Plugin System**: Allow Lua/custom scripts?
   - Recommendation: Defer to future version

4. **Sixel/Kitty Images**: Support inline images?
   - Recommendation: Defer, very niche

5. **Paste Detection**: Handle multi-line paste specially?
   - Recommendation: Yes, prompt to send as multiple lines or paste service
