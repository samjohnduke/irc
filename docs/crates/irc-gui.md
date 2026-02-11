# irc-gui

A modern desktop GUI client for IRC built with the `iced` framework. Provides a polished, native-feeling experience with modern chat app conventions.

## Responsibilities

- Render a modern, responsive desktop UI
- Handle mouse and keyboard interaction
- Display conversations with rich formatting
- Support multiple servers and channels with easy navigation
- System tray integration and notifications
- Cross-platform support (Linux, macOS, Windows)

## UI Design

### Main Window Layout

```
┌─────────────────────────────────────────────────────────────────────────┐
│  IRC Client                                              [_] [□] [×]    │
├─────────────────────────────────────────────────────────────────────────┤
│┌───────────┐┌─────────────────────────────────────────────┐┌───────────┐│
││ Servers   ││ #rust @ Libera                              ││ Members   ││
││           ││─────────────────────────────────────────────││───────────││
││ ▼ Libera  ││ ┌─────────────────────────────────────────┐ ││ Ops (3)   ││
││   #rust  •││ │ alice                         2:32 PM   │ ││  @alice   ││
││   #linux  ││ │ Has anyone tried the new async traits?  │ ││  @bob     ││
││   @bob    ││ └─────────────────────────────────────────┘ ││  @mod     ││
││           ││ ┌─────────────────────────────────────────┐ ││           ││
││ ▼ OFTC    ││ │ bob                           2:32 PM   │ ││ Voice (2) ││
││   #debian ││ │ Yeah, they're great for this use case   │ ││  +carol   ││
││           ││ └─────────────────────────────────────────┘ ││  +dave    ││
││           ││ ┌─────────────────────────────────────────┐ ││           ││
││           ││ │ ── carol joined ──           2:33 PM   │ ││ Users (47)││
││           ││ └─────────────────────────────────────────┘ ││  eve      ││
││           ││                                             ││  frank    ││
││           ││                                             ││  ...      ││
│├───────────┤│                                             │├───────────┤│
││ + Add     ││                                             ││ 52 online ││
│└───────────┘├─────────────────────────────────────────────┤└───────────┘│
│             │ Message #rust                               │             │
│             │ ┌─────────────────────────────────────────┐ │             │
│             │ │                                         │ │             │
│             │ └─────────────────────────────────────────┘ │             │
│             └─────────────────────────────────────────────┘             │
└─────────────────────────────────────────────────────────────────────────┘
```

### Components

1. **Server/Channel Sidebar**
   - Collapsible server groups
   - Channel list with unread indicators
   - Private message conversations
   - Status indicators (connected, away, etc.)
   - Context menus for actions

2. **Message Area**
   - Grouped messages by sender
   - Timestamps (hover or always visible)
   - Clickable URLs
   - Nick mentions highlighted
   - Message actions on hover (reply, copy, etc.)
   - Infinite scroll with lazy loading

3. **Member Sidebar**
   - Grouped by status (ops, voice, regular)
   - Online/away indicators
   - Context menu (PM, whois, kick, etc.)
   - Collapsible

4. **Input Area**
   - Multi-line input support
   - Typing indicator (IRCv3)
   - File drop for DCC (future)
   - Command autocomplete popup
   - Nick autocomplete with popup

5. **Title Bar**
   - Channel name and topic
   - Edit topic button (if op)
   - Channel settings button

## Message Rendering

### Message Bubbles

```
┌─────────────────────────────────────────────────┐
│ alice                                   2:32 PM │
│                                                 │
│ Has anyone tried the new async traits?          │
│                                                 │
│ I'm wondering if they'd work for this use case  │
│ where we need to abstract over different async  │
│ runtimes.                                       │
└─────────────────────────────────────────────────┘
```

### System Messages

```
        ── carol joined #rust ──           2:33 PM
        ── Topic changed by bob ──         2:34 PM
           "Welcome to #rust - Pair your braces"
```

### Actions

```
        * alice waves hello                2:35 PM
```

## Keyboard Shortcuts

| Key | Action |
|-----|--------|
| `Ctrl+N` | New server connection |
| `Ctrl+J` | Quick channel switch (fuzzy finder) |
| `Ctrl+K` | Quick channel switch (alternative) |
| `Ctrl+Tab` | Next channel |
| `Ctrl+Shift+Tab` | Previous channel |
| `Ctrl+1-9` | Switch to channel 1-9 |
| `Ctrl+W` | Close current tab |
| `Ctrl+,` | Open settings |
| `Ctrl+Q` | Quit |
| `Escape` | Close dialogs/popups |
| `Page Up/Down` | Scroll messages |
| `Home` | Scroll to oldest |
| `End` | Scroll to newest |

## Dialogs

### Server Connection Dialog

```
┌─ Connect to Server ───────────────────────────────┐
│                                                   │
│  Server Name     [Libera Chat                  ]  │
│                                                   │
│  Address         [irc.libera.chat              ]  │
│  Port            [6697    ] [✓] Use TLS           │
│                                                   │
│  ── Identity ──                                   │
│  Nickname        [mynick                       ]  │
│  Username        [mynick                       ]  │
│  Real Name       [My Real Name                 ]  │
│                                                   │
│  ── Auto-join ──                                  │
│  Channels        [#channel1, #channel2         ]  │
│                                                   │
│  [✓] Connect on startup                           │
│  [✓] Auto-reconnect                               │
│                                                   │
│                    [ Cancel ]  [ Connect ]        │
└───────────────────────────────────────────────────┘
```

### Settings Dialog

```
┌─ Settings ────────────────────────────────────────┐
│                                                   │
│  ┌─────────────┐                                  │
│  │ General     │  Theme          [Dark ▼]        │
│  │ Appearance  │                                  │
│  │ Notifications│ Font Size      [14px ▼]        │
│  │ Connections │                                  │
│  │ Advanced    │  [✓] Show timestamps             │
│  └─────────────┘  [✓] Show join/part messages     │
│                   [ ] Compact mode                │
│                                                   │
│                   Message grouping                │
│                   [5 minutes ▼]                   │
│                                                   │
│                    [ Cancel ]  [ Save ]           │
└───────────────────────────────────────────────────┘
```

## Theming

### Built-in Themes

- **Dark** (default): Dark background, light text
- **Light**: Light background, dark text
- **Nord**: Nord color palette
- **Solarized Dark/Light**: Solarized color palette
- **System**: Follow OS dark/light mode

### Theme Structure

```rust
pub struct Theme {
    /// Background colors
    pub background: Background,

    /// Text colors
    pub text: TextColors,

    /// Accent colors
    pub accent: AccentColors,

    /// Message colors
    pub messages: MessageColors,

    /// Sidebar colors
    pub sidebar: SidebarColors,

    /// Nick colors (for hash-based coloring)
    pub nick_palette: Vec<Color>,
}

pub struct Background {
    pub primary: Color,
    pub secondary: Color,
    pub tertiary: Color,
    pub hover: Color,
    pub selected: Color,
}

pub struct TextColors {
    pub primary: Color,
    pub secondary: Color,
    pub muted: Color,
    pub link: Color,
}
```

## Configuration

```toml
# ~/.config/irc/gui.toml

[window]
# Remember window size and position
remember_size = true
start_maximized = false
width = 1200
height = 800

[appearance]
theme = "dark"
font_family = "system"
font_size = 14
message_grouping_minutes = 5
show_timestamps = true
timestamp_format = "%H:%M"
compact_mode = false

[sidebar]
# Sidebar widths (pixels)
servers_width = 200
members_width = 180
show_member_count = true

[messages]
show_joins = true
show_parts = true
show_quits = true
show_nick_changes = true
show_mode_changes = false

[notifications]
enabled = true
sound = true
on_mention = true
on_private_message = true
on_highlight = true
# Keywords that trigger highlights
highlight_words = ["urgent", "alert"]

[tray]
enabled = true
minimize_to_tray = true
close_to_tray = false

[logging]
enabled = false
path = "~/.local/share/irc/logs"
```

## Internal Architecture

```
┌─────────────────────────────────────────────────────────────────┐
│                           irc-gui                               │
├─────────────────────────────────────────────────────────────────┤
│                                                                 │
│  ┌───────────────────────────────────────────────────────────┐  │
│  │                     Iced Application                      │  │
│  │  ┌─────────┐  ┌─────────┐  ┌─────────┐  ┌─────────┐       │  │
│  │  │ Sidebar │  │ Messages│  │ Members │  │  Input  │       │  │
│  │  │  View   │  │  View   │  │  View   │  │  View   │       │  │
│  │  └────┬────┘  └────┬────┘  └────┬────┘  └────┬────┘       │  │
│  │       │            │            │            │            │  │
│  │       └────────────┴────────────┴────────────┘            │  │
│  │                          │                                │  │
│  │                          ▼                                │  │
│  │                   ┌─────────────┐                         │  │
│  │                   │  App State  │                         │  │
│  │                   └──────┬──────┘                         │  │
│  └──────────────────────────┼────────────────────────────────┘  │
│                             │                                   │
│                             ▼                                   │
│                   ┌─────────────────┐                           │
│                   │  ClientManager  │◀── irc-client-lib         │
│                   └─────────────────┘                           │
│                                                                 │
└─────────────────────────────────────────────────────────────────┘
```

### Message Types (Iced)

```rust
#[derive(Debug, Clone)]
pub enum Message {
    // === IRC Events ===
    IrcEvent(irc_client_lib::Event),

    // === Navigation ===
    SelectServer(ServerId),
    SelectChannel(ServerId, String),
    SelectQuery(ServerId, String),

    // === Input ===
    InputChanged(String),
    InputSubmit,

    // === UI Actions ===
    ToggleMemberList,
    ToggleServerList,
    ScrollMessages(f32),

    // === Dialogs ===
    OpenConnectDialog,
    OpenSettings,
    OpenJoinDialog,
    CloseDialog,

    // === Connect Dialog ===
    ConnectDialogUpdate(ConnectDialogMessage),
    ConnectSubmit,

    // === Context Menu ===
    ShowContextMenu(ContextMenuKind, Point),
    ContextMenuAction(ContextMenuAction),
    HideContextMenu,

    // === Window ===
    WindowResized(u32, u32),
    CloseRequested,

    // === Notifications ===
    NotificationClicked(NotificationId),

    // === Tray ===
    TrayIconClicked,
    TrayMenuAction(TrayAction),
}
```

### App State

```rust
pub struct App {
    /// IRC client manager
    clients: ClientManager,

    /// UI state
    ui: UiState,

    /// Current view
    active_view: ActiveView,

    /// Dialogs
    dialog: Option<Dialog>,

    /// Configuration
    config: GuiConfig,

    /// Theme
    theme: Theme,

    /// Notification manager
    notifications: NotificationManager,
}

pub struct UiState {
    /// Server sidebar expanded state
    server_expanded: HashMap<ServerId, bool>,

    /// Member list visible
    members_visible: bool,

    /// Server list visible
    servers_visible: bool,

    /// Input text
    input_text: String,

    /// Message scroll position
    scroll_offset: f32,

    /// Context menu state
    context_menu: Option<ContextMenu>,
}

pub enum ActiveView {
    Server(ServerId),
    Channel { server: ServerId, channel: String },
    Query { server: ServerId, nick: String },
}

pub enum Dialog {
    Connect(ConnectDialogState),
    Settings(SettingsDialogState),
    JoinChannel(JoinDialogState),
    Confirm(ConfirmDialogState),
}
```

## Internal Structure

```
irc-gui/
├── Cargo.toml
└── src/
    ├── main.rs              # Entry point
    ├── app.rs               # Iced Application impl
    ├── message.rs           # Message enum
    ├── state.rs             # App state
    ├── views/
    │   ├── mod.rs
    │   ├── sidebar.rs       # Server/channel sidebar
    │   ├── messages.rs      # Message list view
    │   ├── members.rs       # Member list view
    │   ├── input.rs         # Message input
    │   └── titlebar.rs      # Channel title bar
    ├── dialogs/
    │   ├── mod.rs
    │   ├── connect.rs       # Server connection dialog
    │   ├── settings.rs      # Settings dialog
    │   └── join.rs          # Join channel dialog
    ├── widgets/
    │   ├── mod.rs
    │   ├── message_bubble.rs
    │   ├── nick_badge.rs
    │   └── context_menu.rs
    ├── theme.rs             # Theme definitions
    ├── config.rs            # Configuration
    └── notifications.rs     # Desktop notifications
```

## Dependencies

```toml
[dependencies]
irc-client-lib = { path = "../irc-client-lib" }
iced = { version = "0.13", features = ["tokio", "image"] }  # pin minor; iced API changes between releases
tokio = { version = "1", features = ["full"] }
serde = { version = "1", features = ["derive"] }
toml = "0.8"
directories = "5"
tracing = "0.1"
tracing-subscriber = "0.3"
chrono = "0.4"
notify-rust = "4"  # Cross-platform desktop notifications (Linux/macOS/Windows)
open = "5"         # Open URLs in browser
image = "0.25"     # For avatar/image support later
ksni = "0.2"       # Linux system tray (StatusNotifierItem over DBus)
```

## Platform Considerations

### Linux
- Use XDG directories for config/data
- DBus notifications via `notify-rust`
- System tray via `ksni` (StatusNotifierItem protocol)

### macOS
- Notifications via `notify-rust` (uses native macOS APIs)
- Menu bar integration (iced native support)
- Proper Cmd key bindings (Cmd+Q, Cmd+, for settings, etc.)

### Windows
- Notifications via `notify-rust` (uses Windows toast notifications)
- System tray via Windows API (iced native support)
- Proper window chrome

## Open Questions

1. **Framework Choice**: Stick with iced or consider egui/Tauri?
   - Recommendation: iced for native feel, reconsider if hitting limitations

2. **Image Support**: Inline image previews for URLs?
   - Recommendation: Defer, add URL preview cards first

3. **Rich Text**: Support mIRC color codes?
   - Recommendation: Basic support (bold, underline, colors)

4. **Spell Check**: Integrate spell checking?
   - Recommendation: Platform-native if available

5. **Accessibility**: Screen reader support?
   - Recommendation: Yes, iced has some support, prioritize

6. **Update Mechanism**: Auto-update?
   - Recommendation: Defer, manual updates initially
