//! Buffer management for the TUI.
//!
//! Buffers are containers for messages - channels, private queries,
//! and the server buffer.

use std::collections::VecDeque;

use super::message::DisplayMessage;

/// Maximum messages to keep in a buffer.
const MAX_BUFFER_MESSAGES: usize = 1000;

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
        }
    }

    /// Create a server buffer.
    pub fn server() -> Self {
        Self::new("Server", BufferKind::Server)
    }


    /// Add a message to the buffer.
    pub fn add_message(&mut self, msg: DisplayMessage) {
        // Check for duplicate by msgid
        if let Some(ref msgid) = msg.msgid {
            if self.messages.iter().any(|m| m.msgid.as_ref() == Some(msgid)) {
                return;
            }
        }

        // If we're scrolled, keep the scroll position stable
        if self.scroll_offset > 0 {
            self.scroll_offset += 1;
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
    }

    /// Get display name for the buffer.
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
        self.buffers.iter_mut().find(|b| b.name.eq_ignore_ascii_case(name))
    }

    /// Get or create a buffer.
    pub fn get_or_create(&mut self, name: &str, kind: BufferKind) -> &mut Buffer {
        if let Some(idx) = self.buffers.iter().position(|b| b.name.eq_ignore_ascii_case(name)) {
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
        if let Some(idx) = self.buffers.iter().position(|b| b.name.eq_ignore_ascii_case(name)) {
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
        if let Some(idx) = self.buffers.iter().position(|b| b.name.eq_ignore_ascii_case(name)) {
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
