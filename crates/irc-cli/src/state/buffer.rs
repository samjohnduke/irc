//! Buffer management for the TUI.
//!
//! Buffers are containers for messages - channels, private queries,
//! and the server buffer.

use std::collections::VecDeque;
use std::time::Instant;

use super::message::DisplayMessage;

/// Maximum messages to keep in a buffer.
const MAX_BUFFER_MESSAGES: usize = 1000;

/// Number of lines from bottom to consider "at bottom" for auto-scroll.
const AUTO_SCROLL_THRESHOLD: usize = 3;

/// Time window for aggregating join/part events (30 seconds).
const JOIN_PART_AGGREGATE_WINDOW_MS: u128 = 30_000;

/// A message buffer (channel, query, or server).
#[derive(Debug)]
pub struct Buffer {
    /// Buffer name (channel name, nick for query, or "Server").
    pub name: String,

    /// Buffer type.
    pub kind: BufferKind,

    /// Messages in this buffer.
    messages: VecDeque<DisplayMessage>,

    /// Current scroll position (lines from bottom, 0 = at bottom).
    pub scroll_offset: usize,

    /// Unread message count.
    pub unread_count: usize,

    /// Whether there are unread highlights (mentions).
    pub has_highlight: bool,

    /// Channel topic (if channel).
    pub topic: Option<String>,

    /// Number of new messages that arrived while scrolled up.
    /// Reset when scrolling to bottom.
    pub new_messages_while_scrolled: usize,

    /// Pending join events for aggregation.
    pending_joins: Vec<String>,

    /// Pending part/quit events for aggregation.
    pending_parts: Vec<(String, bool)>, // (nick, is_quit)

    /// Time when first pending event was added.
    pending_events_start: Option<Instant>,
}

/// Buffer type.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BufferKind {
    /// Server messages buffer.
    Server,
    /// IRC channel.
    Channel,
    /// Private query/conversation.
    Query,
}

impl Buffer {
    /// Create a new buffer.
    pub fn new(name: impl Into<String>, kind: BufferKind) -> Self {
        Self {
            name: name.into(),
            kind,
            messages: VecDeque::with_capacity(MAX_BUFFER_MESSAGES),
            scroll_offset: 0,
            unread_count: 0,
            has_highlight: false,
            topic: None,
            new_messages_while_scrolled: 0,
            pending_joins: Vec::new(),
            pending_parts: Vec::new(),
            pending_events_start: None,
        }
    }

    /// Create a server buffer.
    pub fn server() -> Self {
        Self::new("Server", BufferKind::Server)
    }

    /// Check if we're at or near the bottom (within threshold for auto-scroll).
    pub fn is_at_bottom(&self) -> bool {
        self.scroll_offset <= AUTO_SCROLL_THRESHOLD
    }

    /// Add a message to the buffer.
    pub fn add_message(&mut self, msg: DisplayMessage) {
        // Check for expired pending events first
        self.check_pending_events();

        // Handle join/part/quit aggregation for channel buffers
        if self.kind == BufferKind::Channel
            && let Some(nick) = msg.join_part_nick()
        {
            let nick = nick.to_string();
            if msg.is_join() {
                self.add_join(nick);
                return;
            } else {
                self.add_part(nick, msg.is_quit());
                return;
            }
        }

        // Non-join/part message: flush any pending events first
        self.flush_pending_events();

        self.add_message_internal(msg);
    }

    /// Add a message and increment unread count.
    pub fn add_message_unread(&mut self, msg: DisplayMessage, is_highlight: bool) {
        self.add_message(msg);
        self.unread_count += 1;
        if is_highlight {
            self.has_highlight = true;
        }
    }

    /// Insert messages at the beginning (for history).
    pub fn prepend_messages(&mut self, messages: impl IntoIterator<Item = DisplayMessage>) {
        for msg in messages {
            self.messages.push_front(msg);
        }

        // Trim old messages from the end if needed
        while self.messages.len() > MAX_BUFFER_MESSAGES {
            self.messages.pop_back();
        }
    }

    /// Clear unread status.
    pub fn mark_read(&mut self) {
        self.unread_count = 0;
        self.has_highlight = false;
    }

    /// Get all messages.
    pub fn messages(&self) -> impl Iterator<Item = &DisplayMessage> {
        self.messages.iter()
    }

    /// Clear all messages.
    pub fn clear(&mut self) {
        self.messages.clear();
        self.scroll_offset = 0;
    }

    /// Scroll up by a number of lines.
    pub fn scroll_up(&mut self, lines: usize) {
        let max_scroll = self.messages.len().saturating_sub(1);
        self.scroll_offset = (self.scroll_offset + lines).min(max_scroll);
    }

    /// Scroll down by a number of lines.
    pub fn scroll_down(&mut self, lines: usize) {
        self.scroll_offset = self.scroll_offset.saturating_sub(lines);
    }

    /// Scroll to bottom.
    pub fn scroll_to_bottom(&mut self) {
        self.scroll_offset = 0;
        self.new_messages_while_scrolled = 0;
    }

    /// Get display name for the buffer.
    #[allow(dead_code)]
    pub fn display_name(&self) -> &str {
        &self.name
    }

    /// Check if this is a channel buffer.
    pub fn is_channel(&self) -> bool {
        self.kind == BufferKind::Channel
    }

    /// Check if this is the server buffer.
    pub fn is_server(&self) -> bool {
        self.kind == BufferKind::Server
    }

    /// Set the topic.
    pub fn set_topic(&mut self, topic: Option<String>) {
        self.topic = topic;
    }

    /// Flush any pending join/part events as aggregated messages.
    pub fn flush_pending_events(&mut self) {
        if self.pending_joins.is_empty() && self.pending_parts.is_empty() {
            return;
        }

        // Flush joins
        if !self.pending_joins.is_empty() {
            let nicks = std::mem::take(&mut self.pending_joins);
            let msg = DisplayMessage::aggregated_join(nicks);
            self.add_message_internal(msg);
        }

        // Flush parts (separate quit and part)
        if !self.pending_parts.is_empty() {
            let events = std::mem::take(&mut self.pending_parts);

            // Group by quit vs part
            let (quits, parts): (Vec<_>, Vec<_>) =
                events.into_iter().partition(|(_, is_quit)| *is_quit);

            if !parts.is_empty() {
                let nicks: Vec<String> = parts.into_iter().map(|(nick, _)| nick).collect();
                let msg = DisplayMessage::aggregated_part(nicks, false);
                self.add_message_internal(msg);
            }

            if !quits.is_empty() {
                let nicks: Vec<String> = quits.into_iter().map(|(nick, _)| nick).collect();
                let msg = DisplayMessage::aggregated_part(nicks, true);
                self.add_message_internal(msg);
            }
        }

        self.pending_events_start = None;
    }

    /// Check if pending events window has expired and flush if needed.
    pub fn check_pending_events(&mut self) {
        if let Some(start) = self.pending_events_start
            && start.elapsed().as_millis() >= JOIN_PART_AGGREGATE_WINDOW_MS
        {
            self.flush_pending_events();
        }
    }

    /// Add a join event, aggregating if appropriate.
    pub fn add_join(&mut self, nick: String) {
        self.check_pending_events();

        if self.pending_events_start.is_none() {
            self.pending_events_start = Some(Instant::now());
        }

        self.pending_joins.push(nick);
    }

    /// Add a part event, aggregating if appropriate.
    pub fn add_part(&mut self, nick: String, is_quit: bool) {
        self.check_pending_events();

        if self.pending_events_start.is_none() {
            self.pending_events_start = Some(Instant::now());
        }

        self.pending_parts.push((nick, is_quit));
    }

    /// Internal method to add a message without join/part handling.
    fn add_message_internal(&mut self, msg: DisplayMessage) {
        // Check for duplicate by msgid
        if let Some(ref msgid) = msg.msgid
            && self
                .messages
                .iter()
                .any(|m| m.msgid.as_ref() == Some(msgid))
        {
            return;
        }

        // Track if we're scrolled up before adding
        let was_scrolled_up = self.scroll_offset > AUTO_SCROLL_THRESHOLD;

        // If we're scrolled, keep the scroll position stable
        if self.scroll_offset > 0 {
            self.scroll_offset += 1;
        }

        // Track new messages while scrolled up
        if was_scrolled_up {
            self.new_messages_while_scrolled += 1;
        }

        self.messages.push_back(msg);

        // Trim old messages
        while self.messages.len() > MAX_BUFFER_MESSAGES {
            self.messages.pop_front();
            if self.scroll_offset > 0 {
                self.scroll_offset = self.scroll_offset.saturating_sub(1);
            }
        }
    }
}

/// Collection of buffers with active buffer tracking.
#[derive(Debug)]
pub struct BufferList {
    /// All buffers.
    buffers: Vec<Buffer>,

    /// Index of active buffer.
    active_index: usize,
}

impl Default for BufferList {
    fn default() -> Self {
        Self::new()
    }
}

impl BufferList {
    /// Create a new buffer list with a server buffer.
    pub fn new() -> Self {
        Self {
            buffers: vec![Buffer::server()],
            active_index: 0,
        }
    }

    /// Get the active buffer.
    pub fn active(&self) -> &Buffer {
        &self.buffers[self.active_index]
    }

    /// Get the active buffer mutably.
    pub fn active_mut(&mut self) -> &mut Buffer {
        &mut self.buffers[self.active_index]
    }

    /// Get the active buffer name.
    pub fn active_name(&self) -> &str {
        &self.buffers[self.active_index].name
    }

    /// Get a buffer by name mutably.
    pub fn get_mut(&mut self, name: &str) -> Option<&mut Buffer> {
        self.buffers
            .iter_mut()
            .find(|b| b.name.eq_ignore_ascii_case(name))
    }

    /// Get or create a buffer.
    pub fn get_or_create(&mut self, name: &str, kind: BufferKind) -> &mut Buffer {
        if let Some(idx) = self
            .buffers
            .iter()
            .position(|b| b.name.eq_ignore_ascii_case(name))
        {
            return &mut self.buffers[idx];
        }

        let buffer = Buffer::new(name, kind);
        self.buffers.push(buffer);
        self.buffers.last_mut().unwrap()
    }

    /// Add a message to a buffer (creating it if needed).
    pub fn add_message(&mut self, target: &str, msg: DisplayMessage, is_active: bool) {
        let kind = if target.starts_with('#') || target.starts_with('&') {
            BufferKind::Channel
        } else if target.eq_ignore_ascii_case("Server") {
            BufferKind::Server
        } else {
            BufferKind::Query
        };

        let buffer = self.get_or_create(target, kind);

        if is_active {
            buffer.add_message(msg);
        } else {
            // TODO: detect highlights
            buffer.add_message_unread(msg, false);
        }
    }

    /// Switch to a buffer by name.
    pub fn switch_to(&mut self, name: &str) -> bool {
        if let Some(idx) = self
            .buffers
            .iter()
            .position(|b| b.name.eq_ignore_ascii_case(name))
        {
            self.active_index = idx;
            self.buffers[idx].mark_read();
            true
        } else {
            false
        }
    }

    /// Switch to next buffer.
    pub fn next(&mut self) {
        self.active_index = (self.active_index + 1) % self.buffers.len();
        self.buffers[self.active_index].mark_read();
    }

    /// Switch to previous buffer.
    pub fn prev(&mut self) {
        if self.active_index == 0 {
            self.active_index = self.buffers.len() - 1;
        } else {
            self.active_index -= 1;
        }
        self.buffers[self.active_index].mark_read();
    }

    /// Remove a buffer by name.
    pub fn remove(&mut self, name: &str) {
        if let Some(idx) = self
            .buffers
            .iter()
            .position(|b| b.name.eq_ignore_ascii_case(name))
        {
            // Don't remove the server buffer
            if self.buffers[idx].is_server() {
                return;
            }

            self.buffers.remove(idx);

            // Adjust active index if needed
            if self.active_index >= self.buffers.len() {
                self.active_index = self.buffers.len() - 1;
            } else if self.active_index > idx {
                self.active_index -= 1;
            }
        }
    }

    /// Get all buffers.
    pub fn all(&self) -> &[Buffer] {
        &self.buffers
    }

    /// Get active index.
    pub fn active_index(&self) -> usize {
        self.active_index
    }
}
