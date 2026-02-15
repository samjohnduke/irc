//! Main application and event loop.

use std::io;
use std::time::Duration;

use crossterm::event::{self, Event as TermEvent, EventStream};
use futures::StreamExt;
use ratatui::Terminal;
use ratatui::backend::CrosstermBackend;
use tracing::{debug, info, trace, warn};

use irc_client_lib::{Client, ClientConfig, Event as IrcEvent};

use crate::completion::{
    CompletionContext, find_completion_word, format_nick_completion, get_candidates,
};
use crate::config::UiConfig;
use crate::handler::command::{Command, command_help, parse_command};
use crate::handler::input::{InputMode, KeyAction, VimState, handle_key_event_with_mode};
use crate::state::{BufferKind, BufferList, DisplayMessage};
use crate::style::Theme;
use crate::ui::channel_list::{ChannelEntry, ChannelListState, ChannelListWidget};
use crate::ui::help::HelpWidget;
use crate::ui::input::InputState;
use crate::ui::layout::LayoutConfig;
use crate::ui::layout_modern::{
    CommandPaletteRenderState, SearchRenderState, UserFilterRenderState, draw_modern_layout,
};
use crate::ui::splash::{ConnectionPhase, LogEntry, SplashWidget};
use crate::ui::userlist::ChannelUser;

/// Application state for the splash screen.
#[derive(Debug, Clone)]
pub struct SplashState {
    /// Current connection phase.
    pub phase: ConnectionPhase,
    /// Connection log entries.
    pub log: Vec<LogEntry>,
    /// Animation frame counter.
    pub frame: usize,
}

impl SplashState {
    pub fn new() -> Self {
        Self {
            phase: ConnectionPhase::Starting,
            log: Vec::new(),
            frame: 0,
        }
    }

    pub fn log_info(&mut self, msg: impl Into<String>) {
        self.log.push(LogEntry::info(msg));
    }

    pub fn log_success(&mut self, msg: impl Into<String>) {
        self.log.push(LogEntry::success(msg));
    }

    pub fn log_error(&mut self, msg: impl Into<String>) {
        self.log.push(LogEntry::error(msg));
    }
}

/// Main application state.
pub struct App {
    /// IRC client.
    client: Client,

    /// Client configuration (for reconnection).
    config: ClientConfig,

    /// Buffer list.
    buffers: BufferList,

    /// Input line state.
    input: InputState,

    /// Current nickname.
    nick: String,

    /// Whether we're connected.
    connected: bool,

    /// Should quit.
    should_quit: bool,

    /// UI theme (used by modals).
    theme: Theme,

    /// Layout configuration.
    #[allow(dead_code)]
    layout: LayoutConfig,

    /// Reconnection state.
    reconnect_state: ReconnectState,

    /// Splash screen state (shown during initial connection).
    splash: Option<SplashState>,

    /// Whether to show the help overlay.
    show_help: bool,

    /// Channel list modal state.
    channel_list: ChannelListState,

    /// UI configuration.
    ui_config: UiConfig,

    /// Current input mode (for vim-style navigation).
    input_mode: InputMode,

    /// Vim state (for multi-key commands like gg).
    vim_state: VimState,

    /// Search state.
    search: SearchState,

    /// Whether to show the sidebar (user list + topic).
    show_sidebar: bool,

    /// User list filter state.
    user_filter: UserFilterState,

    /// Command palette state.
    command_palette: CommandPalette,
}

/// User list filter state for sidebar.
#[derive(Debug, Default)]
pub struct UserFilterState {
    /// Whether filter input is active.
    pub active: bool,
    /// Current filter text.
    pub filter: String,
}

/// Command palette action.
#[derive(Debug, Clone)]
pub enum PaletteAction {
    /// Show server capabilities.
    ShowCapabilities,
    /// Show server info (MOTD, version).
    ShowServerInfo,
    /// Join a channel (prompts for input).
    JoinChannel,
    /// Change nickname (prompts for input).
    ChangeNick,
    /// Set away status.
    SetAway,
    /// Clear away status.
    ClearAway,
    /// Toggle sidebar.
    ToggleSidebar,
    /// Toggle join/part messages.
    ToggleJoinPart,
    /// Disconnect from server.
    Disconnect,
    /// Reconnect to server.
    Reconnect,
    /// Show keyboard shortcuts.
    ShowKeybindings,
}

impl PaletteAction {
    fn label(&self) -> &'static str {
        match self {
            Self::ShowCapabilities => "Show Server Capabilities",
            Self::ShowServerInfo => "Show Server Info (MOTD)",
            Self::JoinChannel => "Join Channel...",
            Self::ChangeNick => "Change Nickname...",
            Self::SetAway => "Set Away Status...",
            Self::ClearAway => "Clear Away Status",
            Self::ToggleSidebar => "Toggle Sidebar",
            Self::ToggleJoinPart => "Toggle Join/Part Messages",
            Self::Disconnect => "Disconnect",
            Self::Reconnect => "Reconnect",
            Self::ShowKeybindings => "Show Keyboard Shortcuts",
        }
    }

    fn all() -> Vec<Self> {
        vec![
            Self::ShowCapabilities,
            Self::ShowServerInfo,
            Self::JoinChannel,
            Self::ChangeNick,
            Self::SetAway,
            Self::ClearAway,
            Self::ToggleSidebar,
            Self::ToggleJoinPart,
            Self::Disconnect,
            Self::Reconnect,
            Self::ShowKeybindings,
        ]
    }
}

/// Command palette state.
#[derive(Debug, Default)]
pub struct CommandPalette {
    /// Whether the palette is visible.
    pub visible: bool,
    /// Current filter/search text.
    pub filter: String,
    /// Selected index.
    pub selected: usize,
    /// Filtered actions.
    pub filtered_actions: Vec<PaletteAction>,
    /// Info display content (for inline results).
    pub info_content: Vec<String>,
}

impl CommandPalette {
    pub fn open(&mut self) {
        self.visible = true;
        self.filter.clear();
        self.selected = 0;
        self.info_content.clear();
        self.update_filtered();
    }

    pub fn close(&mut self) {
        self.visible = false;
        self.filter.clear();
        self.info_content.clear();
    }

    pub fn update_filtered(&mut self) {
        let filter_lower = self.filter.to_lowercase();
        self.filtered_actions = PaletteAction::all()
            .into_iter()
            .filter(|a| filter_lower.is_empty() || a.label().to_lowercase().contains(&filter_lower))
            .collect();
        // Clamp selection
        if self.selected >= self.filtered_actions.len() {
            self.selected = self.filtered_actions.len().saturating_sub(1);
        }
    }

    pub fn select_next(&mut self) {
        if !self.filtered_actions.is_empty() {
            self.selected = (self.selected + 1) % self.filtered_actions.len();
        }
    }

    pub fn select_prev(&mut self) {
        if !self.filtered_actions.is_empty() {
            self.selected = self
                .selected
                .checked_sub(1)
                .unwrap_or(self.filtered_actions.len() - 1);
        }
    }

    pub fn selected_action(&self) -> Option<PaletteAction> {
        self.filtered_actions.get(self.selected).cloned()
    }

    pub fn set_info(&mut self, lines: Vec<String>) {
        self.info_content = lines;
    }
}

/// Search state for in-buffer search.
#[derive(Debug, Default)]
pub struct SearchState {
    /// Search query.
    pub query: String,
    /// Whether search is active.
    pub active: bool,
    /// Current match index.
    pub current_match: usize,
    /// Total number of matches.
    pub total_matches: usize,
    /// Indices of matching messages in the buffer.
    pub match_indices: Vec<usize>,
}

/// Reconnection state tracking.
#[derive(Debug, Clone)]
struct ReconnectState {
    /// Number of reconnect attempts.
    attempts: u32,
    /// Current delay in seconds.
    current_delay: u64,
    /// Whether we're actively trying to reconnect.
    reconnecting: bool,
    /// Time of next reconnect attempt (if reconnecting).
    next_attempt: Option<std::time::Instant>,
}

impl App {
    /// Create a new application with the given config.
    pub fn new(config: ClientConfig) -> Self {
        Self::with_ui_config(config, UiConfig::default())
    }

    /// Create a new application with the given client and UI configs.
    pub fn with_ui_config(config: ClientConfig, ui_config: UiConfig) -> Self {
        let nick = config
            .nicknames
            .first()
            .cloned()
            .unwrap_or_else(|| "user".into());
        let reconnect_delay = config.reconnect_delay;

        Self {
            client: Client::new(config.clone()),
            config,
            buffers: BufferList::new(),
            input: InputState::new(),
            nick,
            connected: false,
            should_quit: false,
            theme: Theme::default(),
            layout: LayoutConfig::default(),
            reconnect_state: ReconnectState {
                attempts: 0,
                current_delay: reconnect_delay,
                reconnecting: false,
                next_attempt: None,
            },
            splash: Some(SplashState::new()),
            show_help: false,
            channel_list: ChannelListState::new(),
            ui_config,
            input_mode: InputMode::Insert,
            vim_state: VimState::default(),
            search: SearchState::default(),
            show_sidebar: true,
            user_filter: UserFilterState::default(),
            command_palette: CommandPalette::default(),
        }
    }

    /// Check if echo-message capability is enabled.
    /// When enabled, the server echoes our messages back, so we shouldn't add them locally.
    fn echo_message_enabled(&self) -> bool {
        if let Ok(state) = self.client.state().try_read() {
            state.caps().echo_message_enabled()
        } else {
            false
        }
    }

    /// Run the application.
    pub async fn run(
        &mut self,
        terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    ) -> io::Result<()> {
        info!("Starting IRC client application");
        debug!("Splash screen active: {}", self.splash.is_some());

        // Subscribe to events BEFORE connecting
        let mut irc_events = self.client.subscribe();
        debug!("Subscribed to client events");

        // Terminal event stream
        let mut term_events = EventStream::new();

        // Update splash state
        if let Some(ref mut splash) = self.splash {
            splash.log_info(format!(
                "Connecting to {}:{}...",
                self.config.server, self.config.port
            ));
            splash.phase = ConnectionPhase::Connecting;
        }

        // Spawn the connection task
        debug!("Taking client for connection task");
        let mut client = std::mem::take(&mut self.client);
        let (connect_tx, mut connect_rx) =
            tokio::sync::oneshot::channel::<Result<Client, (Client, String)>>();

        debug!("Spawning connection task");
        tokio::spawn(async move {
            debug!("Connection task started");
            match client.connect().await {
                Ok(()) => {
                    debug!("Connection task: connect() succeeded");
                    let _ = connect_tx.send(Ok(client));
                }
                Err(e) => {
                    debug!("Connection task: connect() failed: {}", e);
                    let _ = connect_tx.send(Err((client, e.to_string())));
                }
            }
        });

        // Track if we've received the client back
        let mut waiting_for_connect = true;

        debug!(
            "Entering main event loop, splash active: {}",
            self.splash.is_some()
        );

        // Main event loop
        loop {
            // Draw UI
            trace!("Drawing frame, splash active: {}", self.splash.is_some());
            terminal.draw(|frame| {
                if let Some(ref splash) = self.splash {
                    // Draw splash screen
                    let widget = SplashWidget::new(
                        &self.config.server,
                        self.config.port,
                        &splash.phase,
                        &splash.log,
                        splash.frame,
                        &self.theme,
                    );
                    frame.render_widget(widget, frame.area());
                } else {
                    // Get channel users for user list
                    let channel_users = self.get_channel_users();

                    // Build search state for rendering
                    let search_state = if self.search.active {
                        Some(SearchRenderState {
                            active: true,
                            query: self.search.query.clone(),
                            current_match: self.search.current_match,
                            total_matches: self.search.total_matches,
                        })
                    } else {
                        None
                    };

                    // Build user filter render state
                    let user_filter_state =
                        if self.user_filter.active || !self.user_filter.filter.is_empty() {
                            Some(UserFilterRenderState {
                                active: self.user_filter.active,
                                filter: self.user_filter.filter.clone(),
                            })
                        } else {
                            None
                        };

                    // Build command palette render state
                    let palette_state = if self.command_palette.visible {
                        Some(CommandPaletteRenderState {
                            filter: self.command_palette.filter.clone(),
                            selected: self.command_palette.selected,
                            items: self
                                .command_palette
                                .filtered_actions
                                .iter()
                                .enumerate()
                                .map(|(i, a)| {
                                    (a.label().to_string(), i == self.command_palette.selected)
                                })
                                .collect(),
                            info_content: self.command_palette.info_content.clone(),
                        })
                    } else {
                        None
                    };

                    // Draw modern minimal layout
                    draw_modern_layout(
                        frame,
                        &self.buffers,
                        &self.input,
                        &self.nick,
                        self.connected,
                        &channel_users,
                        self.input_mode,
                        search_state.as_ref(),
                        self.ui_config.hide_joinpart,
                        self.show_sidebar,
                        user_filter_state.as_ref(),
                        palette_state.as_ref(),
                    );

                    // Draw channel list modal if active
                    if self.channel_list.visible {
                        frame.render_widget(
                            ChannelListWidget::new(&self.channel_list, &self.theme),
                            frame.area(),
                        );
                    }

                    // Draw help overlay if active
                    if self.show_help {
                        frame.render_widget(HelpWidget::new(), frame.area());
                    }
                }
            })?;

            // Check if we need to reconnect
            if self.splash.is_none() && self.should_attempt_reconnect() {
                self.try_reconnect().await;
                // Re-subscribe to new client's events
                irc_events = self.client.subscribe();
            }

            // Wait for events with a short timeout for animation
            tokio::select! {
                // Check for connect completion
                result = &mut connect_rx, if waiting_for_connect => {
                    debug!("Received connect_rx result");
                    waiting_for_connect = false;
                    match result {
                        Ok(Ok(client)) => {
                            debug!("connect_rx: success");
                            self.client = client;
                            // Don't re-subscribe - the original subscription is still valid
                            // and has the Connected event buffered
                        }
                        Ok(Err((client, error))) => {
                            debug!("connect_rx: error: {}", error);
                            self.client = client;
                            irc_events = self.client.subscribe();
                            if let Some(ref mut splash) = self.splash {
                                splash.phase = ConnectionPhase::Failed(error.clone());
                                splash.log_error(&error);
                            }
                            // Transition to main UI so user can /reconnect
                            debug!("Dismissing splash screen due to error");
                            self.splash = None;
                            self.buffers.add_message(
                                "Server",
                                DisplayMessage::error(format!("Connection failed: {}", error)),
                                true,
                            );
                        }
                        Err(_) => {
                            warn!("connect_rx channel closed unexpectedly");
                        }
                    }
                }

                // Terminal events
                Some(Ok(event)) = term_events.next() => {
                    if let TermEvent::Key(key) = event {
                        // Allow quit during splash
                        if self.splash.is_some() {
                            if key.code == crossterm::event::KeyCode::Char('c')
                                && key.modifiers.contains(crossterm::event::KeyModifiers::CONTROL)
                            {
                                self.should_quit = true;
                            }
                            // Ignore other keys during splash
                        } else {
                            self.handle_key(key).await;
                        }
                    }
                }

                // IRC events
                Ok(event) = irc_events.recv() => {
                    debug!("Received IRC event: {:?}", std::mem::discriminant(&event));
                    self.handle_irc_event(event).await;
                }

                // Animation tick (50ms = 20fps)
                _ = tokio::time::sleep(Duration::from_millis(50)) => {
                    if let Some(ref mut splash) = self.splash {
                        splash.frame = splash.frame.wrapping_add(1);
                    }
                    if self.should_quit {
                        break;
                    }
                }
            }

            if self.should_quit {
                break;
            }
        }

        Ok(())
    }

    /// Handle a keyboard event.
    async fn handle_key(&mut self, key: event::KeyEvent) {
        use crossterm::event::KeyCode;

        // Handle channel list modal if open
        if self.channel_list.visible {
            match key.code {
                KeyCode::Esc => {
                    self.channel_list.close();
                }
                KeyCode::Up => {
                    self.channel_list.select_prev();
                }
                KeyCode::Down => {
                    self.channel_list.select_next();
                }
                KeyCode::Enter => {
                    let filtered = self.channel_list.filtered_channels();
                    if filtered.is_empty() || self.channel_list.channels.is_empty() {
                        // No results - send a new search
                        self.send_list_search().await;
                    } else if let Some(channel) = self.channel_list.selected_channel() {
                        // Join selected channel
                        let channel_name = channel.name.clone();
                        self.channel_list.close();
                        let _ = self.client.join(&channel_name).await;
                    }
                }
                KeyCode::Tab => {
                    // Tab triggers a new server search with current filter
                    self.send_list_search().await;
                }
                KeyCode::Backspace => {
                    self.channel_list.filter_backspace();
                }
                KeyCode::Char('u') if key.modifiers.contains(event::KeyModifiers::CONTROL) => {
                    self.channel_list.filter_clear();
                }
                KeyCode::Char(c) => {
                    self.channel_list.filter_insert(c);
                }
                _ => {}
            }
            return;
        }

        // Handle user filter input when active
        if self.user_filter.active {
            match key.code {
                KeyCode::Esc | KeyCode::Enter => {
                    self.user_filter.active = false;
                    // Keep the filter text so results stay filtered
                }
                KeyCode::Backspace => {
                    self.user_filter.filter.pop();
                }
                KeyCode::Char('u') if key.modifiers.contains(event::KeyModifiers::CONTROL) => {
                    self.user_filter.filter.clear();
                }
                KeyCode::Char(c)
                    if !key.modifiers.contains(event::KeyModifiers::CONTROL)
                        && !key.modifiers.contains(event::KeyModifiers::ALT) =>
                {
                    self.user_filter.filter.push(c);
                }
                _ => {}
            }
            return;
        }

        // Handle command palette when open
        if self.command_palette.visible {
            // If showing info content, Backspace or Esc goes back to list
            if !self.command_palette.info_content.is_empty() {
                match key.code {
                    KeyCode::Esc | KeyCode::Backspace => {
                        self.command_palette.info_content.clear();
                    }
                    _ => {}
                }
                return;
            }

            match key.code {
                KeyCode::Esc => {
                    self.command_palette.close();
                }
                KeyCode::Up => {
                    self.command_palette.select_prev();
                }
                KeyCode::Down => {
                    self.command_palette.select_next();
                }
                KeyCode::Enter => {
                    if let Some(action) = self.command_palette.selected_action() {
                        self.execute_palette_action(action).await;
                    }
                }
                KeyCode::Backspace => {
                    self.command_palette.filter.pop();
                    self.command_palette.update_filtered();
                }
                KeyCode::Char('u') if key.modifiers.contains(event::KeyModifiers::CONTROL) => {
                    self.command_palette.filter.clear();
                    self.command_palette.update_filtered();
                }
                KeyCode::Char(c)
                    if !key.modifiers.contains(event::KeyModifiers::CONTROL)
                        && !key.modifiers.contains(event::KeyModifiers::ALT) =>
                {
                    self.command_palette.filter.push(c);
                    self.command_palette.update_filtered();
                }
                _ => {}
            }
            return;
        }

        // Special handling for mode switching with Esc
        if key.code == KeyCode::Esc {
            if self.show_help {
                self.show_help = false;
                return;
            }
            if self.search.active {
                self.search.active = false;
                self.input_mode = InputMode::Insert;
                return;
            }
            // In insert mode with empty input and vim_mode enabled -> switch to normal
            if self.ui_config.vim_mode
                && self.input_mode == InputMode::Insert
                && self.input.text.is_empty()
            {
                self.input_mode = InputMode::Normal;
                return;
            }
            // In normal mode -> switch back to insert
            if self.input_mode == InputMode::Normal {
                self.input_mode = InputMode::Insert;
                return;
            }
        }

        // Handle 'i' to return to insert mode from normal mode
        if self.input_mode == InputMode::Normal && key.code == KeyCode::Char('i') {
            self.input_mode = InputMode::Insert;
            return;
        }

        let action =
            handle_key_event_with_mode(key, &mut self.input, self.input_mode, &mut self.vim_state);

        match action {
            KeyAction::None => {}

            KeyAction::Submit(text) => {
                self.handle_input(text).await;
            }

            KeyAction::Quit => {
                let _ = self.client.quit(Some("Leaving")).await;
                self.should_quit = true;
            }

            KeyAction::NextBuffer => {
                self.buffers.next();
                self.search.active = false; // Clear search when switching buffers
            }

            KeyAction::PrevBuffer => {
                self.buffers.prev();
                self.search.active = false;
            }

            KeyAction::JumpToBuffer(index) => {
                if let Some(name) = self.buffers.all().get(index).map(|b| b.name.clone()) {
                    self.buffers.switch_to(&name);
                    self.search.active = false;
                }
            }

            KeyAction::ScrollUp(lines) => {
                self.buffers.active_mut().scroll_up(lines);
            }

            KeyAction::ScrollDown(lines) => {
                self.buffers.active_mut().scroll_down(lines);
            }

            KeyAction::ScrollTop => {
                self.buffers.active_mut().scroll_up(usize::MAX);
            }

            KeyAction::ScrollBottom => {
                self.buffers.active_mut().scroll_to_bottom();
            }

            KeyAction::TabComplete => {
                self.handle_tab_complete(false);
            }

            KeyAction::TabCompleteReverse => {
                self.handle_tab_complete(true);
            }

            KeyAction::ToggleHelp => {
                self.show_help = !self.show_help;
            }

            KeyAction::ToggleSidebar => {
                self.show_sidebar = !self.show_sidebar;
            }

            KeyAction::ToggleUserFilter => {
                if self.show_sidebar {
                    self.user_filter.active = !self.user_filter.active;
                    if !self.user_filter.active {
                        // Clear filter when deactivating
                        self.user_filter.filter.clear();
                    }
                }
            }

            KeyAction::OpenCommandPalette => {
                self.command_palette.open();
            }

            KeyAction::CloseHelp => {
                if self.show_help {
                    self.show_help = false;
                }
            }

            KeyAction::CloseBuffer => {
                let name = self.buffers.active_name().to_string();
                if name.starts_with('#') || name.starts_with('&') {
                    let _ = self.client.part(&name, None).await;
                }
                self.buffers.remove(&name);
            }

            KeyAction::EnterSearch => {
                self.search.active = true;
                self.search.query.clear();
                self.input_mode = InputMode::Search;
            }

            KeyAction::ExitSearch => {
                self.search.active = false;
                self.input_mode = if self.ui_config.vim_mode {
                    InputMode::Normal
                } else {
                    InputMode::Insert
                };
            }

            KeyAction::SearchNext => {
                if self.search.active {
                    // Update query from input
                    self.search.query = self.input.text.clone();
                    self.perform_search();
                    self.search_next();
                }
            }

            KeyAction::SearchPrev => {
                if self.search.active {
                    self.search.query = self.input.text.clone();
                    self.perform_search();
                    self.search_prev();
                }
            }
        }
    }

    /// Perform search in current buffer.
    fn perform_search(&mut self) {
        if self.search.query.is_empty() {
            self.search.match_indices.clear();
            self.search.total_matches = 0;
            return;
        }

        let query_lower = self.search.query.to_lowercase();
        let messages: Vec<_> = self.buffers.active().messages().collect();

        self.search.match_indices = messages
            .iter()
            .enumerate()
            .filter(|(_, msg)| {
                // Check if message contains search term
                match &msg.kind {
                    crate::state::MessageKind::Privmsg { text, nick, .. } => {
                        text.to_lowercase().contains(&query_lower)
                            || nick.to_lowercase().contains(&query_lower)
                    }
                    crate::state::MessageKind::Action { text, nick, .. } => {
                        text.to_lowercase().contains(&query_lower)
                            || nick.to_lowercase().contains(&query_lower)
                    }
                    crate::state::MessageKind::Notice { text, .. } => {
                        text.to_lowercase().contains(&query_lower)
                    }
                    crate::state::MessageKind::Server { text } => {
                        text.to_lowercase().contains(&query_lower)
                    }
                    _ => false,
                }
            })
            .map(|(i, _)| i)
            .collect();

        self.search.total_matches = self.search.match_indices.len();
        self.search.current_match = 0;
    }

    /// Navigate to next search match.
    fn search_next(&mut self) {
        if self.search.match_indices.is_empty() {
            return;
        }

        if self.search.current_match < self.search.total_matches.saturating_sub(1) {
            self.search.current_match += 1;
        } else {
            self.search.current_match = 0; // Wrap around
        }

        self.scroll_to_match();
    }

    /// Navigate to previous search match.
    fn search_prev(&mut self) {
        if self.search.match_indices.is_empty() {
            return;
        }

        if self.search.current_match > 0 {
            self.search.current_match -= 1;
        } else {
            self.search.current_match = self.search.total_matches.saturating_sub(1); // Wrap around
        }

        self.scroll_to_match();
    }

    /// Scroll to show current search match.
    fn scroll_to_match(&mut self) {
        if let Some(&msg_index) = self.search.match_indices.get(self.search.current_match) {
            let total_messages = self.buffers.active().messages().count();
            // Calculate scroll offset to show the match
            let offset = total_messages.saturating_sub(msg_index + 1);
            self.buffers.active_mut().scroll_offset = offset;
        }
    }

    /// Handle tab completion.
    fn handle_tab_complete(&mut self, reverse: bool) {
        // If completion is already active, cycle through candidates
        if self.input.completion.is_active() {
            let start_pos = self.input.completion.start_pos();
            let completion = if reverse {
                self.input.completion.prev()
            } else {
                self.input.completion.next()
            };

            if let Some(completion) = completion {
                let completion = completion.to_string();
                self.input.apply_completion(&completion, start_pos);
            }
            return;
        }

        // Start new completion
        let (word_start, prefix) = find_completion_word(&self.input.text, self.input.cursor);

        if prefix.is_empty() {
            return;
        }

        // Get completion context
        let members = self.get_channel_members();
        let buffer_names = self.get_buffer_names();

        let context = CompletionContext {
            members: &members,
            buffers: &buffer_names,
        };

        let candidates = get_candidates(prefix, &context);

        if candidates.is_empty() {
            return;
        }

        // Format candidates (add suffix for nicks)
        let at_start = word_start == 0;
        let formatted_candidates: Vec<String> = candidates
            .into_iter()
            .map(|c| {
                if c.starts_with('/') || c.starts_with('#') || c.starts_with('&') {
                    format!("{} ", c)
                } else {
                    format_nick_completion(&c, at_start)
                }
            })
            .collect();

        // Start completion
        self.input.completion.start(
            prefix.to_string(),
            word_start,
            self.input.cursor,
            formatted_candidates,
        );

        // Apply first completion
        if let Some(completion) = self.input.completion.current() {
            let completion = completion.to_string();
            self.input.apply_completion(&completion, word_start);
        }
    }

    /// Get channel members for current buffer (for completion).
    fn get_channel_members(&self) -> Vec<String> {
        let active = self.buffers.active();
        if !active.is_channel() {
            return Vec::new();
        }

        // Get members from client state
        let state = self.client.state();
        if let Ok(state) = state.try_read()
            && let Some(channel) = state.channel(&active.name)
        {
            return channel.member_nicks().map(String::from).collect();
        }

        Vec::new()
    }

    /// Send a LIST command with the current filter.
    async fn send_list_search(&mut self) {
        let filter = self.channel_list.get_search_filter();

        // Start new search (clears existing results)
        self.channel_list.start_search();

        // Build and send LIST command
        let list_cmd = irc_proto::Message::new(irc_proto::Command::List {
            channels: filter.map(|f| vec![f]),
        });

        let _ = self.client.send_raw(list_cmd).await;
    }

    /// Get channel users with status for the user list widget.
    fn get_channel_users(&self) -> Vec<ChannelUser> {
        use crate::ui::userlist::UserStatus;

        let active = self.buffers.active();
        if !active.is_channel() {
            return Vec::new();
        }

        let state = self.client.state();
        if let Ok(state) = state.try_read()
            && let Some(channel) = state.channel(&active.name)
        {
            return channel
                .members
                .iter()
                .map(|(nick, info)| {
                    // Determine the highest status from prefixes
                    let status = if info.prefixes.contains('~') {
                        UserStatus::Owner
                    } else if info.prefixes.contains('&') {
                        UserStatus::Admin
                    } else if info.prefixes.contains('@') {
                        UserStatus::Op
                    } else if info.prefixes.contains('%') {
                        UserStatus::HalfOp
                    } else if info.prefixes.contains('+') {
                        UserStatus::Voice
                    } else {
                        UserStatus::Normal
                    };

                    ChannelUser {
                        nick: nick.to_string(),
                        status,
                        away: false, // TODO: track away status
                    }
                })
                .collect();
        }

        Vec::new()
    }

    /// Get all buffer names.
    fn get_buffer_names(&self) -> Vec<String> {
        self.buffers
            .all()
            .iter()
            .filter(|b| b.is_channel())
            .map(|b| b.name.clone())
            .collect()
    }

    /// Start the reconnection process.
    fn start_reconnect(&mut self) {
        // Check if we've exceeded max attempts
        if self.config.reconnect_max_attempts > 0
            && self.reconnect_state.attempts >= self.config.reconnect_max_attempts
        {
            self.buffers.add_message(
                "Server",
                DisplayMessage::error(
                    "Maximum reconnect attempts reached. Use /reconnect to try again.",
                ),
                true,
            );
            self.reconnect_state.reconnecting = false;
            return;
        }

        self.reconnect_state.attempts += 1;
        self.reconnect_state.reconnecting = true;

        let delay = self.reconnect_state.current_delay;
        self.reconnect_state.next_attempt =
            Some(std::time::Instant::now() + Duration::from_secs(delay));

        // Increase delay for next attempt (exponential backoff)
        self.reconnect_state.current_delay = (delay * 2).min(self.config.reconnect_max_delay);

        self.buffers.add_message(
            "Server",
            DisplayMessage::server(format!(
                "Reconnecting in {} seconds (attempt {})...",
                delay, self.reconnect_state.attempts
            )),
            true,
        );
    }

    /// Attempt to reconnect.
    async fn try_reconnect(&mut self) {
        self.reconnect_state.next_attempt = None;

        // Collect channels to rejoin before reconnecting
        let channels_to_rejoin: Vec<String> = self
            .buffers
            .all()
            .iter()
            .filter(|b| b.is_channel())
            .map(|b| b.name.clone())
            .collect();

        self.buffers.add_message(
            "Server",
            DisplayMessage::server("Attempting to reconnect..."),
            true,
        );

        // Create a new client with the same config
        self.client = Client::new(self.config.clone());

        match self.client.connect().await {
            Ok(()) => {
                self.connected = true;
                self.nick = self.client.nick().await;
                self.reconnect_state.reconnecting = false;
                self.reconnect_state.attempts = 0;
                self.reconnect_state.current_delay = self.config.reconnect_delay;

                self.buffers.add_message(
                    "Server",
                    DisplayMessage::server("Reconnected successfully!"),
                    true,
                );

                // Rejoin channels that were open
                if !channels_to_rejoin.is_empty() {
                    self.buffers.add_message(
                        "Server",
                        DisplayMessage::server(format!(
                            "Rejoining {} channel(s)...",
                            channels_to_rejoin.len()
                        )),
                        true,
                    );

                    for channel in channels_to_rejoin {
                        if let Err(e) = self.client.join(&channel).await {
                            self.buffers.add_message(
                                "Server",
                                DisplayMessage::error(format!(
                                    "Failed to rejoin {}: {}",
                                    channel, e
                                )),
                                true,
                            );
                        }
                    }
                }
            }
            Err(e) => {
                self.buffers.add_message(
                    "Server",
                    DisplayMessage::error(format!("Reconnect failed: {}", e)),
                    true,
                );

                // Schedule next attempt
                if self.config.reconnect {
                    self.start_reconnect();
                }
            }
        }
    }

    /// Check if it's time to reconnect.
    fn should_attempt_reconnect(&self) -> bool {
        if !self.reconnect_state.reconnecting {
            return false;
        }

        if let Some(next_attempt) = self.reconnect_state.next_attempt {
            std::time::Instant::now() >= next_attempt
        } else {
            false
        }
    }

    /// Execute a command palette action.
    async fn execute_palette_action(&mut self, action: PaletteAction) {
        match action {
            PaletteAction::ShowCapabilities => {
                // Get capabilities from client
                let caps = self.client.capabilities().await;
                let mut lines = vec!["Server Capabilities:".to_string()];
                if caps.is_empty() {
                    lines.push("  (none negotiated)".to_string());
                } else {
                    for cap in caps {
                        lines.push(format!("  • {}", cap));
                    }
                }
                self.command_palette.set_info(lines);
            }

            PaletteAction::ShowServerInfo => {
                let mut lines = vec!["Server Information:".to_string()];
                lines.push(format!("  Server: {}", self.config.server));
                lines.push(format!("  Port: {}", self.config.port));
                lines.push(format!(
                    "  TLS: {}",
                    if self.config.tls { "yes" } else { "no" }
                ));
                lines.push(format!(
                    "  Connected: {}",
                    if self.connected { "yes" } else { "no" }
                ));
                lines.push(format!("  Nick: {}", self.nick));
                self.command_palette.set_info(lines);
            }

            PaletteAction::JoinChannel => {
                // Close palette and let user type /join
                self.command_palette.close();
                self.input.set_text("/join ");
            }

            PaletteAction::ChangeNick => {
                self.command_palette.close();
                self.input.set_text("/nick ");
            }

            PaletteAction::SetAway => {
                self.command_palette.close();
                self.input.set_text("/away ");
            }

            PaletteAction::ClearAway => {
                self.command_palette.close();
                let _ = self.client.away(None).await;
                self.buffers.add_message(
                    "Server",
                    DisplayMessage::server("Away status cleared"),
                    true,
                );
            }

            PaletteAction::ToggleSidebar => {
                self.command_palette.close();
                self.show_sidebar = !self.show_sidebar;
            }

            PaletteAction::ToggleJoinPart => {
                self.command_palette.close();
                self.ui_config.hide_joinpart = !self.ui_config.hide_joinpart;
                let status = if self.ui_config.hide_joinpart {
                    "hidden"
                } else {
                    "visible"
                };
                self.buffers.add_message(
                    "Server",
                    DisplayMessage::server(format!("Join/part messages are now {}", status)),
                    true,
                );
            }

            PaletteAction::Disconnect => {
                self.command_palette.close();
                let _ = self.client.quit(Some("Disconnecting")).await;
                self.connected = false;
            }

            PaletteAction::Reconnect => {
                self.command_palette.close();
                if !self.connected {
                    self.reconnect_state.attempts = 0;
                    self.reconnect_state.reconnecting = true;
                    self.reconnect_state.next_attempt = Some(std::time::Instant::now());
                }
            }

            PaletteAction::ShowKeybindings => {
                let lines = vec![
                    "Keyboard Shortcuts:".to_string(),
                    "".to_string(),
                    "  Ctrl+K      Command palette".to_string(),
                    "  Ctrl+N/P    Next/prev buffer".to_string(),
                    "  Alt+1-9     Jump to buffer".to_string(),
                    "  Alt+S       Toggle sidebar".to_string(),
                    "  Alt+F       Filter users".to_string(),
                    "  Ctrl+F      Search messages".to_string(),
                    "  PgUp/PgDn   Scroll messages".to_string(),
                    "  Tab         Nick completion".to_string(),
                    "  F1          Help".to_string(),
                ];
                self.command_palette.set_info(lines);
            }
        }
    }

    /// Handle user input (command or message).
    async fn handle_input(&mut self, text: String) {
        let command = parse_command(&text);

        match command {
            Command::Join { channel, key } => {
                let channel = if !channel.starts_with('#') && !channel.starts_with('&') {
                    format!("#{}", channel)
                } else {
                    channel
                };

                if let Some(key) = key {
                    let _ = self.client.join_with_key(&channel, &key).await;
                } else {
                    let _ = self.client.join(&channel).await;
                }
            }

            Command::Part { message } => {
                let active = self.buffers.active_name().to_string();
                if active.starts_with('#') || active.starts_with('&') {
                    let _ = self.client.part(&active, message.as_deref()).await;
                }
            }

            Command::Msg { target, text } => {
                let _ = self.client.privmsg(&target, &text).await;

                // Add to our buffer only if echo-message is not enabled
                // (otherwise server will echo it back to us)
                if !self.echo_message_enabled() {
                    self.buffers.add_message(
                        &target,
                        DisplayMessage::privmsg(&self.nick, &text).echo(),
                        self.buffers.active_name().eq_ignore_ascii_case(&target),
                    );
                }

                // Switch to that buffer
                if !self.buffers.switch_to(&target) {
                    self.buffers.get_or_create(&target, BufferKind::Query);
                    self.buffers.switch_to(&target);
                }
            }

            Command::Me { text } => {
                let target = self.buffers.active_name().to_string();
                if target != "Server" {
                    let _ = self.client.action(&target, &text).await;

                    // Add to our buffer only if echo-message is not enabled
                    if !self.echo_message_enabled() {
                        self.buffers.add_message(
                            &target,
                            DisplayMessage::action(&self.nick, &text).echo(),
                            true,
                        );
                    }
                }
            }

            Command::Nick { nick } => {
                let _ = self.client.change_nick(&nick).await;
            }

            Command::Quit { message } => {
                let _ = self.client.quit(message.as_deref()).await;
                self.should_quit = true;
            }

            Command::Topic { text } => {
                let active = self.buffers.active_name().to_string();
                if active.starts_with('#') || active.starts_with('&') {
                    let _ = self.client.topic(&active, text.as_deref()).await;
                }
            }

            Command::Kick { nick, reason } => {
                let active = self.buffers.active_name().to_string();
                if active.starts_with('#') || active.starts_with('&') {
                    let _ = self.client.kick(&active, &nick, reason.as_deref()).await;
                }
            }

            Command::Invite { nick } => {
                let active = self.buffers.active_name().to_string();
                if active.starts_with('#') || active.starts_with('&') {
                    let _ = self.client.invite(&nick, &active).await;
                }
            }

            Command::Away { message } => {
                let _ = self.client.away(message.as_deref()).await;
            }

            Command::Clear => {
                self.buffers.active_mut().clear();
            }

            Command::JoinPart => {
                self.ui_config.hide_joinpart = !self.ui_config.hide_joinpart;
                let status = if self.ui_config.hide_joinpart {
                    "hidden"
                } else {
                    "visible"
                };
                self.buffers.add_message(
                    "Server",
                    DisplayMessage::server(format!("Join/part messages are now {}", status)),
                    true,
                );
            }

            Command::Close => {
                let name = self.buffers.active_name().to_string();
                if name.starts_with('#') || name.starts_with('&') {
                    // Part the channel first
                    let _ = self.client.part(&name, None).await;
                }
                self.buffers.remove(&name);
            }

            Command::Query { nick } => {
                self.buffers.get_or_create(&nick, BufferKind::Query);
                self.buffers.switch_to(&nick);
            }

            Command::History { count } => {
                let target = self.buffers.active_name().to_string();
                let _ = self.client.chathistory(&target, count.unwrap_or(100)).await;
            }

            Command::Raw { text } => {
                // Parse and send raw IRC command
                if let Ok(msg) = irc_proto::Message::parse_str(&text) {
                    let _ = self.client.send_raw(msg).await;
                } else {
                    self.buffers.add_message(
                        "Server",
                        DisplayMessage::error("Invalid IRC command"),
                        self.buffers.active_name() == "Server",
                    );
                }
            }

            Command::Help { topic } => {
                let help_text = command_help(topic.as_deref());
                for line in help_text.lines() {
                    self.buffers
                        .add_message("Server", DisplayMessage::server(line), true);
                }
                self.buffers.switch_to("Server");
            }

            Command::Reconnect => {
                if self.connected {
                    self.buffers.add_message(
                        "Server",
                        DisplayMessage::server("Already connected. Use /disconnect first."),
                        true,
                    );
                } else {
                    // Reset reconnect state and try immediately
                    self.reconnect_state.attempts = 0;
                    self.reconnect_state.current_delay = self.config.reconnect_delay;
                    self.reconnect_state.reconnecting = false;
                    self.try_reconnect().await;
                }
            }

            Command::Disconnect => {
                if self.connected {
                    let _ = self.client.quit(Some("Disconnecting")).await;
                    self.connected = false;
                    self.reconnect_state.reconnecting = false;
                    self.buffers.add_message(
                        "Server",
                        DisplayMessage::server("Disconnected."),
                        true,
                    );
                } else {
                    // Stop any reconnection attempts
                    self.reconnect_state.reconnecting = false;
                    self.buffers.add_message(
                        "Server",
                        DisplayMessage::server("Not connected."),
                        true,
                    );
                }
            }

            Command::List { filter } => {
                // Warn about unfiltered LIST on large networks
                if filter.is_none() {
                    self.buffers.add_message(
                        "Server",
                        DisplayMessage::server(
                            "Note: Unfiltered /list on large networks may cause disconnection.",
                        ),
                        self.buffers.active_name() == "Server",
                    );
                    self.buffers.add_message(
                        "Server",
                        DisplayMessage::server(
                            "Consider using /list <pattern> (e.g., /list *rust*)",
                        ),
                        self.buffers.active_name() == "Server",
                    );
                }

                // Open the channel list modal
                self.channel_list.open();

                // If there's a filter, pre-populate it
                if let Some(ref f) = filter {
                    self.channel_list.set_filter(f.clone());
                }

                // Build LIST command (optionally with server-side filter)
                let list_cmd = irc_proto::Message::new(irc_proto::Command::List {
                    channels: filter.map(|f| vec![f]),
                });

                let _ = self.client.send_raw(list_cmd).await;
            }

            Command::Message { text } => {
                let target = self.buffers.active_name().to_string();
                if target != "Server" {
                    let _ = self.client.privmsg(&target, &text).await;

                    // Add to our buffer only if echo-message is not enabled
                    if !self.echo_message_enabled() {
                        self.buffers.add_message(
                            &target,
                            DisplayMessage::privmsg(&self.nick, &text).echo(),
                            true,
                        );
                    }
                }
            }
        }
    }

    /// Handle an IRC event.
    async fn handle_irc_event(&mut self, event: IrcEvent) {
        match event {
            IrcEvent::ConnectionProgress { phase, message } => {
                if let Some(ref mut splash) = self.splash {
                    // Update splash screen based on progress
                    match phase.as_str() {
                        "connecting" => {
                            splash.phase = ConnectionPhase::Connecting;
                            splash.log_info(&message);
                        }
                        "connected" => {
                            splash.log_success(&message);
                        }
                        "capabilities" => {
                            splash.phase = ConnectionPhase::Capabilities;
                            splash.log_info(&message);
                        }
                        "authenticating" => {
                            splash.phase = ConnectionPhase::Authenticating;
                            splash.log_info(&message);
                        }
                        "registering" => {
                            splash.phase = ConnectionPhase::Registering;
                            splash.log_info(&message);
                        }
                        "complete" => {
                            splash.phase = ConnectionPhase::Connected;
                            splash.log_success(&message);
                        }
                        "error" => {
                            splash.phase = ConnectionPhase::Failed(message.clone());
                            splash.log_error(&message);
                        }
                        _ => {
                            splash.log_info(&message);
                        }
                    }
                }
            }

            IrcEvent::Connected {
                nick,
                server,
                welcome,
            } => {
                info!("Connected event received: nick={}, server={}", nick, server);
                self.connected = true;
                self.nick = nick.clone();

                // Transition from splash to main UI
                if self.splash.is_some() {
                    debug!("Dismissing splash screen after successful connection");
                    // Small delay to show "Connected!" message
                    tokio::time::sleep(Duration::from_millis(500)).await;
                    self.splash = None;
                }

                self.buffers.add_message(
                    "Server",
                    DisplayMessage::server(format!("Connected to {}", server)),
                    self.buffers.active_name() == "Server",
                );
                self.buffers.add_message(
                    "Server",
                    DisplayMessage::server(welcome),
                    self.buffers.active_name() == "Server",
                );
            }

            IrcEvent::Disconnected { reason, .. } => {
                self.connected = false;

                // Close channel list modal if open
                if self.channel_list.visible {
                    self.channel_list.close();
                }

                let msg = reason.clone().unwrap_or_else(|| "Connection closed".into());
                self.buffers.add_message(
                    "Server",
                    DisplayMessage::error(format!("Disconnected: {}", msg)),
                    true,
                );

                // Start reconnection if enabled
                if self.config.reconnect {
                    self.start_reconnect();
                }
            }

            IrcEvent::Privmsg {
                source,
                target,
                message,
                meta,
            } => {
                // Determine which buffer to add to
                let buffer_target = if target.eq_ignore_ascii_case(&self.nick) {
                    // PM to us - use source nick
                    source.split('!').next().unwrap_or(&source).to_string()
                } else {
                    target.clone()
                };

                let nick = source.split('!').next().unwrap_or(&source);
                let mut msg = if let Some(time) = meta.time {
                    DisplayMessage::with_time(
                        time,
                        crate::state::MessageKind::Privmsg {
                            nick: nick.to_string(),
                            text: message,
                        },
                    )
                } else {
                    DisplayMessage::privmsg(nick, message)
                };

                if let Some(msgid) = meta.msgid {
                    msg = msg.with_msgid(msgid);
                }

                let is_active = self
                    .buffers
                    .active_name()
                    .eq_ignore_ascii_case(&buffer_target);
                self.buffers.add_message(&buffer_target, msg, is_active);
            }

            IrcEvent::Notice {
                source,
                target,
                message,
                ..
            } => {
                let buffer_target = if target.starts_with('#') || target.starts_with('&') {
                    target
                } else {
                    "Server".to_string()
                };

                let msg = DisplayMessage::notice(source, message);
                let is_active = self
                    .buffers
                    .active_name()
                    .eq_ignore_ascii_case(&buffer_target);
                self.buffers.add_message(&buffer_target, msg, is_active);
            }

            IrcEvent::Action {
                source,
                target,
                action,
                meta,
            } => {
                let buffer_target = if target.eq_ignore_ascii_case(&self.nick) {
                    source.split('!').next().unwrap_or(&source).to_string()
                } else {
                    target
                };

                let nick = source.split('!').next().unwrap_or(&source);
                let mut msg = if let Some(time) = meta.time {
                    DisplayMessage::with_time(
                        time,
                        crate::state::MessageKind::Action {
                            nick: nick.to_string(),
                            text: action,
                        },
                    )
                } else {
                    DisplayMessage::action(nick, action)
                };

                if let Some(msgid) = meta.msgid {
                    msg = msg.with_msgid(msgid);
                }

                let is_active = self
                    .buffers
                    .active_name()
                    .eq_ignore_ascii_case(&buffer_target);
                self.buffers.add_message(&buffer_target, msg, is_active);
            }

            IrcEvent::Join {
                nick,
                channel,
                userhost,
                ..
            } => {
                if nick.eq_ignore_ascii_case(&self.nick) {
                    // We joined
                    self.buffers.get_or_create(&channel, BufferKind::Channel);
                    self.buffers.switch_to(&channel);
                }

                let msg = DisplayMessage::join(&nick, userhost);
                let is_active = self.buffers.active_name().eq_ignore_ascii_case(&channel);
                self.buffers.add_message(&channel, msg, is_active);
            }

            IrcEvent::Part {
                nick,
                channel,
                message,
            } => {
                if nick.eq_ignore_ascii_case(&self.nick) {
                    // We left
                    self.buffers.remove(&channel);
                } else {
                    let msg = DisplayMessage::part(&nick, message);
                    let is_active = self.buffers.active_name().eq_ignore_ascii_case(&channel);
                    self.buffers.add_message(&channel, msg, is_active);
                }
            }

            IrcEvent::Kick {
                nick,
                channel,
                kicker,
                reason,
            } => {
                if nick.eq_ignore_ascii_case(&self.nick) {
                    // We were kicked
                    self.buffers.add_message(
                        &channel,
                        DisplayMessage::error(format!(
                            "You were kicked by {} {}",
                            kicker,
                            reason
                                .as_deref()
                                .map(|r| format!("({})", r))
                                .unwrap_or_default()
                        )),
                        true,
                    );
                    self.buffers.remove(&channel);
                } else {
                    let msg = DisplayMessage::kick(&nick, &kicker, reason);
                    let is_active = self.buffers.active_name().eq_ignore_ascii_case(&channel);
                    self.buffers.add_message(&channel, msg, is_active);
                }
            }

            IrcEvent::Quit { nick, message } => {
                // Add to all channel buffers where user was visible
                let msg = DisplayMessage::quit(&nick, message);
                for buffer in self.buffers.all() {
                    if buffer.is_channel() {
                        // Note: we'd need to track channel membership to do this properly
                        // For now, we won't show quit messages
                    }
                }
                let _ = msg; // Suppress unused warning
            }

            IrcEvent::Topic {
                channel,
                topic,
                setter,
            } => {
                if let Some(buffer) = self.buffers.get_mut(&channel) {
                    buffer.set_topic(topic.clone());
                }

                let msg = DisplayMessage::topic(setter, topic);
                let is_active = self.buffers.active_name().eq_ignore_ascii_case(&channel);
                self.buffers.add_message(&channel, msg, is_active);
            }

            IrcEvent::NickChange {
                old_nick: _,
                new_nick,
            } => {
                // Our nick changed
                self.nick = new_nick.clone();
                self.buffers.add_message(
                    "Server",
                    DisplayMessage::server(format!("You are now known as {}", new_nick)),
                    self.buffers.active_name() == "Server",
                );
            }

            IrcEvent::Nick { old_nick, new_nick } => {
                // Someone else changed nick - add to relevant channels
                let msg = DisplayMessage::nick_change(&old_nick, &new_nick);
                let _ = msg; // Would need to track channel membership
            }

            IrcEvent::ChannelMode {
                channel,
                setter,
                modes,
            } => {
                let msg = DisplayMessage::mode(&setter, &modes);
                let is_active = self.buffers.active_name().eq_ignore_ascii_case(&channel);
                self.buffers.add_message(&channel, msg, is_active);
            }

            IrcEvent::Invite { inviter, channel } => {
                self.buffers.add_message(
                    "Server",
                    DisplayMessage::server(format!("{} invites you to {}", inviter, channel)),
                    true,
                );
            }

            IrcEvent::Motd { line } => {
                self.buffers.add_message(
                    "Server",
                    DisplayMessage::server(line),
                    self.buffers.active_name() == "Server",
                );
            }

            IrcEvent::ServerError { message } => {
                self.buffers
                    .add_message("Server", DisplayMessage::error(message), true);
            }

            IrcEvent::Batch {
                batch_type,
                target,
                messages,
            } => {
                if batch_type == "chathistory"
                    && let Some(target) = target
                {
                    // Add history separator
                    let sep = DisplayMessage::history_separator();
                    let is_active = self.buffers.active_name().eq_ignore_ascii_case(&target);

                    // Convert batch events to display messages
                    let display_msgs: Vec<DisplayMessage> = messages
                        .into_iter()
                        .filter_map(|e| match e {
                            IrcEvent::Privmsg {
                                source,
                                message,
                                meta,
                                ..
                            } => {
                                let nick = source.split('!').next().unwrap_or(&source);
                                let mut msg = if let Some(time) = meta.time {
                                    DisplayMessage::with_time(
                                        time,
                                        crate::state::MessageKind::Privmsg {
                                            nick: nick.to_string(),
                                            text: message,
                                        },
                                    )
                                } else {
                                    DisplayMessage::privmsg(nick, message)
                                };
                                if let Some(msgid) = meta.msgid {
                                    msg = msg.with_msgid(msgid);
                                }
                                Some(msg)
                            }
                            IrcEvent::Action {
                                source,
                                action,
                                meta,
                                ..
                            } => {
                                let nick = source.split('!').next().unwrap_or(&source);
                                let mut msg = if let Some(time) = meta.time {
                                    DisplayMessage::with_time(
                                        time,
                                        crate::state::MessageKind::Action {
                                            nick: nick.to_string(),
                                            text: action,
                                        },
                                    )
                                } else {
                                    DisplayMessage::action(nick, action)
                                };
                                if let Some(msgid) = meta.msgid {
                                    msg = msg.with_msgid(msgid);
                                }
                                Some(msg)
                            }
                            _ => None,
                        })
                        .collect();

                    // Prepend history to buffer
                    let buffer = self.buffers.get_or_create(&target, BufferKind::Channel);
                    buffer.prepend_messages(std::iter::once(sep).chain(display_msgs));

                    let _ = is_active; // Suppress unused warning
                }
            }

            IrcEvent::Ping { .. } => {
                // Handled automatically by client
            }

            IrcEvent::Numeric { code, params } => {
                // Handle specific numerics
                match code {
                    // RPL_LISTSTART (321) - List header
                    321 => {
                        // Channel list is starting - modal should already be open
                        trace!("LIST started");
                    }

                    // RPL_LIST (322) - Channel entry
                    // Format: 322 <target> <channel> <visible> :<topic>
                    // target is separate, so params = [channel, visible, topic]
                    322 => {
                        if params.len() >= 2 {
                            let channel_name = params[0].clone();
                            let users: u32 = params[1].parse().unwrap_or(0);
                            let topic = params.get(2).cloned().unwrap_or_default();

                            // Add to channel list modal
                            self.channel_list.add_channel(ChannelEntry::new(
                                channel_name,
                                users,
                                topic,
                            ));
                        }
                    }

                    // RPL_LISTEND (323) - End of list
                    323 => {
                        // Mark loading as complete
                        self.channel_list.finish_loading();
                        trace!(
                            "LIST complete: {} channels",
                            self.channel_list.channels.len()
                        );
                    }

                    // Other numerics - ignore or log
                    _ => {
                        trace!("Unhandled numeric {}: {:?}", code, params);
                    }
                }
            }

            _ => {
                // Other events - could log for debugging
            }
        }
    }
}
