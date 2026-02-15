//! Messages display widget.

use chrono::Duration;
use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Widget, Wrap},
};

use crate::state::{message::MessageKind, Buffer as IrcBuffer, DisplayMessage};
use crate::style::{format_timestamp, nick_color, Theme};

/// Time window for grouping consecutive messages from the same user (5 minutes).
const MESSAGE_GROUP_WINDOW_SECS: i64 = 300;

/// Widget for displaying messages in a buffer.
pub struct MessagesWidget<'a> {
    buffer: &'a IrcBuffer,
    theme: &'a Theme,
}

impl<'a> MessagesWidget<'a> {
    pub fn new(buffer: &'a IrcBuffer, theme: &'a Theme) -> Self {
        Self { buffer, theme }
    }

    /// Check if the current message should be grouped with the previous one.
    /// Returns the nick of the previous message if they should be grouped.
    fn should_group_with_previous(
        &self,
        msg: &DisplayMessage,
        prev_msg: Option<&DisplayMessage>,
    ) -> bool {
        let Some(prev) = prev_msg else {
            return false;
        };

        // Only group Privmsg messages (not actions, joins, etc.)
        let (MessageKind::Privmsg { nick: curr_nick, .. }, MessageKind::Privmsg { nick: prev_nick, .. }) =
            (&msg.kind, &prev.kind)
        else {
            return false;
        };

        // Same user?
        if !curr_nick.eq_ignore_ascii_case(prev_nick) {
            return false;
        }

        // Within time window?
        let time_diff = msg.time.signed_duration_since(prev.time);
        time_diff >= Duration::zero() && time_diff < Duration::seconds(MESSAGE_GROUP_WINDOW_SECS)
    }

    /// Format a message as a continuation (no timestamp/nick header).
    fn format_continuation(&self, text: &str) -> Line<'static> {
        // Indent to align with message text after "[HH:MM] <nick> "
        // Timestamp is 6 chars "[HH:MM] " + indent for alignment
        let indent = "        ";
        Line::from(vec![
            Span::styled(indent.to_string(), Style::default()),
            Span::styled(text.to_string(), self.theme.message_style()),
        ])
    }

    fn format_message(&self, msg: &DisplayMessage, is_continuation: bool) -> Line<'static> {
        // Handle continuation messages (grouped)
        if is_continuation {
            if let MessageKind::Privmsg { text, .. } = &msg.kind {
                return self.format_continuation(text);
            }
        }

        let time_str = format_timestamp(&msg.time);
        let time_span = Span::styled(
            format!("{} ", time_str),
            self.theme.timestamp_style(),
        );

        match &msg.kind {
            MessageKind::Privmsg { nick, text } => {
                Line::from(vec![
                    time_span,
                    Span::styled(
                        "<",
                        Style::default().fg(self.theme.muted),
                    ),
                    Span::styled(
                        nick.clone(),
                        Style::default().fg(nick_color(nick)).add_modifier(Modifier::BOLD),
                    ),
                    Span::styled(
                        "> ",
                        Style::default().fg(self.theme.muted),
                    ),
                    Span::styled(text.clone(), self.theme.message_style()),
                ])
            }

            MessageKind::Action { nick, text } => {
                Line::from(vec![
                    time_span,
                    Span::styled("* ", Style::default().fg(self.theme.action)),
                    Span::styled(
                        nick.clone(),
                        Style::default().fg(nick_color(nick)).add_modifier(Modifier::BOLD),
                    ),
                    Span::styled(format!(" {}", text), self.theme.action_style()),
                ])
            }

            MessageKind::Notice { source, text } => {
                let source_str = source.as_deref().unwrap_or("*");
                Line::from(vec![
                    time_span,
                    Span::styled(
                        format!("-{}- ", source_str),
                        Style::default().fg(Color::Rgb(200, 180, 100)),
                    ),
                    Span::styled(text.clone(), self.theme.message_style()),
                ])
            }

            MessageKind::Join { nick, userhost } => {
                let host_str = userhost
                    .as_ref()
                    .map(|h| format!(" ({})", h))
                    .unwrap_or_default();
                Line::from(vec![
                    time_span,
                    Span::styled("→ ", Style::default().fg(Color::Rgb(100, 200, 100))),
                    Span::styled(
                        nick.clone(),
                        Style::default().fg(nick_color(nick)).add_modifier(Modifier::BOLD),
                    ),
                    Span::styled(
                        format!(" joined{}", host_str),
                        Style::default().fg(Color::Rgb(100, 160, 100)),
                    ),
                ])
            }

            MessageKind::Part { nick, message } => {
                let msg_str = message
                    .as_ref()
                    .map(|m| format!(" ({})", m))
                    .unwrap_or_default();
                Line::from(vec![
                    time_span,
                    Span::styled("← ", Style::default().fg(Color::Rgb(200, 100, 100))),
                    Span::styled(
                        nick.clone(),
                        Style::default().fg(nick_color(nick)),
                    ),
                    Span::styled(
                        format!(" left{}", msg_str),
                        Style::default().fg(Color::Rgb(160, 100, 100)),
                    ),
                ])
            }

            MessageKind::Quit { nick, message } => {
                let msg_str = message
                    .as_ref()
                    .map(|m| format!(" ({})", m))
                    .unwrap_or_default();
                Line::from(vec![
                    time_span,
                    Span::styled("← ", Style::default().fg(Color::Rgb(200, 100, 100))),
                    Span::styled(
                        nick.clone(),
                        Style::default().fg(nick_color(nick)),
                    ),
                    Span::styled(
                        format!(" quit{}", msg_str),
                        Style::default().fg(Color::Rgb(160, 100, 100)),
                    ),
                ])
            }

            MessageKind::Kick { nick, kicker, reason } => {
                let reason_str = reason
                    .as_ref()
                    .map(|r| format!(" ({})", r))
                    .unwrap_or_default();
                Line::from(vec![
                    time_span,
                    Span::styled("✕ ", self.theme.error_style()),
                    Span::styled(
                        nick.clone(),
                        Style::default().fg(nick_color(nick)),
                    ),
                    Span::styled(" was kicked by ", Style::default().fg(Color::Rgb(180, 120, 120))),
                    Span::styled(
                        kicker.clone(),
                        Style::default().fg(nick_color(kicker)),
                    ),
                    Span::styled(reason_str, Style::default().fg(Color::Rgb(180, 120, 120))),
                ])
            }

            MessageKind::Nick { old_nick, new_nick } => {
                Line::from(vec![
                    time_span,
                    Span::styled("• ", self.theme.muted_style()),
                    Span::styled(
                        old_nick.clone(),
                        Style::default().fg(nick_color(old_nick)),
                    ),
                    Span::styled(" is now known as ", self.theme.muted_style()),
                    Span::styled(
                        new_nick.clone(),
                        Style::default().fg(nick_color(new_nick)).add_modifier(Modifier::BOLD),
                    ),
                ])
            }

            MessageKind::Topic { setter, topic } => {
                let setter_str = setter.as_deref().unwrap_or("Someone");
                let topic_str = topic.as_deref().unwrap_or("(cleared)");
                Line::from(vec![
                    time_span,
                    Span::styled("★ ", Style::default().fg(Color::Rgb(255, 200, 100))),
                    Span::styled(
                        setter_str.to_string(),
                        Style::default().fg(nick_color(setter_str)),
                    ),
                    Span::styled(" set topic: ", Style::default().fg(self.theme.muted)),
                    Span::styled(
                        topic_str.to_string(),
                        Style::default()
                            .fg(Color::Rgb(220, 220, 230))
                            .add_modifier(Modifier::ITALIC),
                    ),
                ])
            }

            MessageKind::Mode { setter, modes } => {
                Line::from(vec![
                    time_span,
                    Span::styled("⚙ ", Style::default().fg(Color::Rgb(150, 150, 200))),
                    Span::styled(
                        setter.clone(),
                        Style::default().fg(nick_color(setter)),
                    ),
                    Span::styled(
                        format!(" sets mode: {}", modes),
                        Style::default().fg(Color::Rgb(150, 150, 200)),
                    ),
                ])
            }

            MessageKind::Server { text } => {
                Line::from(vec![
                    time_span,
                    Span::styled("• ", Style::default().fg(self.theme.server)),
                    Span::styled(text.clone(), self.theme.server_style()),
                ])
            }

            MessageKind::Error { text } => {
                Line::from(vec![
                    time_span,
                    Span::styled("✕ ", self.theme.error_style()),
                    Span::styled(text.clone(), self.theme.error_style()),
                ])
            }

            MessageKind::HistorySeparator => {
                Line::from(vec![
                    Span::styled(
                        "───────────────────── History ─────────────────────",
                        Style::default().fg(Color::Rgb(80, 85, 100)),
                    ),
                ])
            }

            MessageKind::AggregatedJoin { nicks } => {
                let count = nicks.len();
                let nicks_str = if count <= 3 {
                    nicks.join(", ")
                } else {
                    format!("{} and {} others", nicks[..3].join(", "), count - 3)
                };
                Line::from(vec![
                    time_span,
                    Span::styled("→ ", Style::default().fg(Color::Rgb(100, 200, 100))),
                    Span::styled(
                        format!(
                            "{} user{} joined: ",
                            count,
                            if count == 1 { "" } else { "s" }
                        ),
                        Style::default().fg(Color::Rgb(100, 160, 100)),
                    ),
                    Span::styled(
                        nicks_str,
                        Style::default().fg(Color::Rgb(140, 200, 140)),
                    ),
                ])
            }

            MessageKind::AggregatedPart { nicks, is_quit } => {
                let count = nicks.len();
                let nicks_str = if count <= 3 {
                    nicks.join(", ")
                } else {
                    format!("{} and {} others", nicks[..3].join(", "), count - 3)
                };
                let action = if *is_quit { "quit" } else { "left" };
                Line::from(vec![
                    time_span,
                    Span::styled("← ", Style::default().fg(Color::Rgb(200, 100, 100))),
                    Span::styled(
                        format!(
                            "{} user{} {}: ",
                            count,
                            if count == 1 { "" } else { "s" },
                            action
                        ),
                        Style::default().fg(Color::Rgb(160, 100, 100)),
                    ),
                    Span::styled(
                        nicks_str,
                        Style::default().fg(Color::Rgb(180, 120, 120)),
                    ),
                ])
            }
        }
    }
}

impl Widget for MessagesWidget<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        // Main message area background
        let bg = self.theme.bg;

        // Fill background
        for y in area.y..area.y + area.height {
            for x in area.x..area.x + area.width {
                buf[(x, y)].set_bg(bg);
            }
        }

        let messages: Vec<_> = self.buffer.messages().collect();

        // Calculate visible range with scroll
        let visible_height = area.height.saturating_sub(1) as usize; // Account for top border
        let total = messages.len();
        let scroll_offset = self.buffer.scroll_offset.min(total.saturating_sub(visible_height));

        let start = total.saturating_sub(visible_height + scroll_offset);
        let end = total.saturating_sub(scroll_offset);

        // Format messages with grouping
        let visible_messages = &messages[start..end];
        let lines: Vec<Line> = visible_messages
            .iter()
            .enumerate()
            .map(|(i, msg)| {
                // Get the previous message in the visible range, or from before if this is first visible
                let prev_msg = if i > 0 {
                    Some(visible_messages[i - 1])
                } else if start > 0 {
                    // First visible message - check against message before visible range
                    Some(messages[start - 1])
                } else {
                    None
                };
                let is_continuation = self.should_group_with_previous(msg, prev_msg);
                self.format_message(msg, is_continuation)
            })
            .collect();

        // Title with buffer name
        let title = format!(" {} ", self.buffer.name);

        // Scroll indicator in title if scrolled
        let title_with_scroll = if self.buffer.scroll_offset > 0 {
            format!(" {} [↑{}] ", self.buffer.name, self.buffer.scroll_offset)
        } else {
            title
        };

        let title_style = if self.buffer.scroll_offset > 0 {
            Style::default()
                .fg(Color::Rgb(255, 200, 100))
                .add_modifier(Modifier::BOLD)
        } else {
            self.theme.title_style()
        };

        let block = Block::default()
            .borders(Borders::NONE)
            .title(Span::styled(title_with_scroll, title_style))
            .title_position(ratatui::widgets::block::Position::Top)
            .style(Style::default().bg(bg));

        let paragraph = Paragraph::new(lines)
            .block(block)
            .wrap(Wrap { trim: false });

        Widget::render(paragraph, area, buf);

        // Draw a subtle top border line manually
        let border_y = area.y;
        for x in area.x..area.x + area.width {
            buf[(x, border_y)].set_char('─');
            buf[(x, border_y)].set_fg(self.theme.border);
            buf[(x, border_y)].set_bg(bg);
        }

        // Draw "N new messages" indicator when scrolled up and have new messages
        if self.buffer.new_messages_while_scrolled > 0 && !self.buffer.is_at_bottom() {
            let new_count = self.buffer.new_messages_while_scrolled;
            let indicator_text = if new_count == 1 {
                " ↓ 1 new message ".to_string()
            } else {
                format!(" ↓ {} new messages ", new_count)
            };

            let indicator_width = indicator_text.len() as u16;
            let indicator_x = area.x + (area.width.saturating_sub(indicator_width)) / 2;
            let indicator_y = area.y + area.height.saturating_sub(1);

            // Only draw if we have space
            if indicator_y > area.y && indicator_x + indicator_width <= area.x + area.width {
                let indicator_style = Style::default()
                    .fg(Color::Rgb(255, 255, 255))
                    .bg(Color::Rgb(80, 100, 140))
                    .add_modifier(Modifier::BOLD);

                for (i, c) in indicator_text.chars().enumerate() {
                    let x = indicator_x + i as u16;
                    if x < area.x + area.width {
                        buf[(x, indicator_y)].set_char(c);
                        buf[(x, indicator_y)].set_style(indicator_style);
                    }
                }
            }
        }
    }
}
