//! Main application and event loop.

use std::io;
use std::time::Duration;

use crossterm::event::{self, Event as TermEvent, EventStream};
use futures::StreamExt;
use ratatui::backend::CrosstermBackend;
use ratatui::Terminal;

use irc_client_lib::{Client, ClientConfig, Event as IrcEvent};

use crate::completion::{find_completion_word, format_nick_completion, get_candidates, CompletionContext};
use crate::handler::command::{command_help, parse_command, Command};
use crate::handler::input::{handle_key_event, KeyAction};
use crate::state::{BufferKind, BufferList, DisplayMessage};
use crate::style::Theme;
use crate::ui::input::InputState;
use crate::ui::layout::{draw_layout, LayoutConfig};

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

    /// UI theme.
    theme: Theme,

    /// Layout configuration.
    layout: LayoutConfig,

    /// Reconnection state.
    reconnect_state: ReconnectState,
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
        let nick = config.nicknames.first().cloned().unwrap_or_else(|| "user".into());
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
        }
    }

    /// Run the application.
    pub async fn run(
        &mut self,
        terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    ) -> io::Result<()> {
        // Add initial server message
        self.buffers.add_message(
            "Server",
            DisplayMessage::server("Connecting..."),
            true,
        );

        // Connect to IRC
        if let Err(e) = self.client.connect().await {
            self.buffers.add_message(
                "Server",
                DisplayMessage::error(format!("Connection failed: {}", e)),
                true,
            );
            // Still run the UI so user can see the error
        } else {
            self.connected = true;
            self.nick = self.client.nick().await;
        }

        // Subscribe to IRC events
        let mut irc_events = self.client.subscribe();

        // Terminal event stream
        let mut term_events = EventStream::new();

        // Main event loop
        loop {
            // Draw UI
            terminal.draw(|frame| {
                draw_layout(
                    frame,
                    &self.buffers,
                    &self.input,
                    &self.nick,
                    self.connected,
                    &self.theme,
                    &self.layout,
                );
            })?;

            // Check if we need to reconnect
            if self.should_attempt_reconnect() {
                self.try_reconnect().await;
                // Re-subscribe to new client's events
                irc_events = self.client.subscribe();
            }

            // Wait for events
            tokio::select! {
                // Terminal events
                Some(Ok(event)) = term_events.next() => {
                    if let TermEvent::Key(key) = event {
                        self.handle_key(key).await;
                    }
                }

                // IRC events
                Ok(event) = irc_events.recv() => {
                    self.handle_irc_event(event).await;
                }

                // Periodic tick for reconnection checks
                _ = tokio::time::sleep(Duration::from_millis(100)) => {
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
        match handle_key_event(key, &mut self.input) {
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
            }

            KeyAction::PrevBuffer => {
                self.buffers.prev();
            }

            KeyAction::ScrollUp(lines) => {
                self.buffers.active_mut().scroll_up(lines);
            }

            KeyAction::ScrollDown(lines) => {
                self.buffers.active_mut().scroll_down(lines);
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

    /// Get channel members for current buffer.
    fn get_channel_members(&self) -> Vec<String> {
        let active = self.buffers.active();
        if !active.is_channel() {
            return Vec::new();
        }

        // Get members from client state
        let state = self.client.state();
        if let Ok(state) = state.try_read() {
            if let Some(channel) = state.channel(&active.name) {
                return channel.member_nicks().map(String::from).collect();
            }
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
                DisplayMessage::error("Maximum reconnect attempts reached. Use /reconnect to try again."),
                true,
            );
            self.reconnect_state.reconnecting = false;
            return;
        }

        self.reconnect_state.attempts += 1;
        self.reconnect_state.reconnecting = true;

        let delay = self.reconnect_state.current_delay;
        self.reconnect_state.next_attempt = Some(
            std::time::Instant::now() + Duration::from_secs(delay),
        );

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

                // Add to our buffer if echo-message is not enabled
                self.buffers.add_message(
                    &target,
                    DisplayMessage::privmsg(&self.nick, &text).echo(),
                    self.buffers.active_name().eq_ignore_ascii_case(&target),
                );

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

                    // Add to our buffer
                    self.buffers.add_message(
                        &target,
                        DisplayMessage::action(&self.nick, &text).echo(),
                        true,
                    );
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
                    self.buffers.add_message(
                        "Server",
                        DisplayMessage::server(line),
                        true,
                    );
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

            Command::Message { text } => {
                let target = self.buffers.active_name().to_string();
                if target != "Server" {
                    let _ = self.client.privmsg(&target, &text).await;

                    // Add to our buffer
                    self.buffers.add_message(
                        &target,
                        DisplayMessage::privmsg(&self.nick, &text).echo(),
                        true,
                    );
                }
            }
        }
    }

    /// Handle an IRC event.
    async fn handle_irc_event(&mut self, event: IrcEvent) {
        match event {
            IrcEvent::Connected { nick, server, welcome } => {
                self.connected = true;
                self.nick = nick;
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

            IrcEvent::Privmsg { source, target, message, meta } => {
                // Determine which buffer to add to
                let buffer_target = if target.eq_ignore_ascii_case(&self.nick) {
                    // PM to us - use source nick
                    source.split('!').next().unwrap_or(&source).to_string()
                } else {
                    target.clone()
                };

                let nick = source.split('!').next().unwrap_or(&source);
                let mut msg = if let Some(time) = meta.time {
                    DisplayMessage::with_time(time, crate::state::MessageKind::Privmsg {
                        nick: nick.to_string(),
                        text: message,
                    })
                } else {
                    DisplayMessage::privmsg(nick, message)
                };

                if let Some(msgid) = meta.msgid {
                    msg = msg.with_msgid(msgid);
                }

                let is_active = self.buffers.active_name().eq_ignore_ascii_case(&buffer_target);
                self.buffers.add_message(&buffer_target, msg, is_active);
            }

            IrcEvent::Notice { source, target, message, .. } => {
                let buffer_target = if target.starts_with('#') || target.starts_with('&') {
                    target
                } else {
                    "Server".to_string()
                };

                let msg = DisplayMessage::notice(source, message);
                let is_active = self.buffers.active_name().eq_ignore_ascii_case(&buffer_target);
                self.buffers.add_message(&buffer_target, msg, is_active);
            }

            IrcEvent::Action { source, target, action, meta } => {
                let buffer_target = if target.eq_ignore_ascii_case(&self.nick) {
                    source.split('!').next().unwrap_or(&source).to_string()
                } else {
                    target
                };

                let nick = source.split('!').next().unwrap_or(&source);
                let mut msg = if let Some(time) = meta.time {
                    DisplayMessage::with_time(time, crate::state::MessageKind::Action {
                        nick: nick.to_string(),
                        text: action,
                    })
                } else {
                    DisplayMessage::action(nick, action)
                };

                if let Some(msgid) = meta.msgid {
                    msg = msg.with_msgid(msgid);
                }

                let is_active = self.buffers.active_name().eq_ignore_ascii_case(&buffer_target);
                self.buffers.add_message(&buffer_target, msg, is_active);
            }

            IrcEvent::Join { nick, channel, userhost, .. } => {
                if nick.eq_ignore_ascii_case(&self.nick) {
                    // We joined
                    self.buffers.get_or_create(&channel, BufferKind::Channel);
                    self.buffers.switch_to(&channel);
                }

                let msg = DisplayMessage::join(&nick, userhost);
                let is_active = self.buffers.active_name().eq_ignore_ascii_case(&channel);
                self.buffers.add_message(&channel, msg, is_active);
            }

            IrcEvent::Part { nick, channel, message } => {
                if nick.eq_ignore_ascii_case(&self.nick) {
                    // We left
                    self.buffers.remove(&channel);
                } else {
                    let msg = DisplayMessage::part(&nick, message);
                    let is_active = self.buffers.active_name().eq_ignore_ascii_case(&channel);
                    self.buffers.add_message(&channel, msg, is_active);
                }
            }

            IrcEvent::Kick { nick, channel, kicker, reason } => {
                if nick.eq_ignore_ascii_case(&self.nick) {
                    // We were kicked
                    self.buffers.add_message(
                        &channel,
                        DisplayMessage::error(format!(
                            "You were kicked by {} {}",
                            kicker,
                            reason.as_deref().map(|r| format!("({})", r)).unwrap_or_default()
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

            IrcEvent::Topic { channel, topic, setter } => {
                if let Some(buffer) = self.buffers.get_mut(&channel) {
                    buffer.set_topic(topic.clone());
                }

                let msg = DisplayMessage::topic(setter, topic);
                let is_active = self.buffers.active_name().eq_ignore_ascii_case(&channel);
                self.buffers.add_message(&channel, msg, is_active);
            }

            IrcEvent::NickChange { old_nick: _, new_nick } => {
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

            IrcEvent::ChannelMode { channel, setter, modes } => {
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
                self.buffers.add_message(
                    "Server",
                    DisplayMessage::error(message),
                    true,
                );
            }

            IrcEvent::Batch { batch_type, target, messages } => {
                if batch_type == "chathistory" {
                    if let Some(target) = target {
                        // Add history separator
                        let sep = DisplayMessage::history_separator();
                        let is_active = self.buffers.active_name().eq_ignore_ascii_case(&target);

                        // Convert batch events to display messages
                        let display_msgs: Vec<DisplayMessage> = messages
                            .into_iter()
                            .filter_map(|e| match e {
                                IrcEvent::Privmsg { source, message, meta, .. } => {
                                    let nick = source.split('!').next().unwrap_or(&source);
                                    let mut msg = if let Some(time) = meta.time {
                                        DisplayMessage::with_time(time, crate::state::MessageKind::Privmsg {
                                            nick: nick.to_string(),
                                            text: message,
                                        })
                                    } else {
                                        DisplayMessage::privmsg(nick, message)
                                    };
                                    if let Some(msgid) = meta.msgid {
                                        msg = msg.with_msgid(msgid);
                                    }
                                    Some(msg)
                                }
                                IrcEvent::Action { source, action, meta, .. } => {
                                    let nick = source.split('!').next().unwrap_or(&source);
                                    let mut msg = if let Some(time) = meta.time {
                                        DisplayMessage::with_time(time, crate::state::MessageKind::Action {
                                            nick: nick.to_string(),
                                            text: action,
                                        })
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
            }

            IrcEvent::Ping { .. } => {
                // Handled automatically by client
            }

            _ => {
                // Other events - could log for debugging
            }
        }
    }
}
