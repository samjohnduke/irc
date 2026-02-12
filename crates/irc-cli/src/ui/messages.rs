//! Messages display widget.

use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::Style,
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Widget, Wrap},
};

use crate::state::{message::MessageKind, Buffer as IrcBuffer};
use crate::style::{format_timestamp, nick_color, Theme};

/// Widget for displaying messages in a buffer.
pub struct MessagesWidget<'a> {
    buffer: &'a IrcBuffer,
    theme: &'a Theme,
}

impl<'a> MessagesWidget<'a> {
    pub fn new(buffer: &'a IrcBuffer, theme: &'a Theme) -> Self {
        Self { buffer, theme }
    }

    fn format_message(&self, msg: &crate::state::DisplayMessage) -> Line<'static> {
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
                        format!("<{}> ", nick),
                        Style::default().fg(nick_color(nick)),
                    ),
                    Span::styled(text.clone(), self.theme.message_style()),
                ])
            }

            MessageKind::Action { nick, text } => {
                Line::from(vec![
                    time_span,
                    Span::styled(
                        format!("* {} {}", nick, text),
                        self.theme.action_style(),
                    ),
                ])
            }

            MessageKind::Notice { source, text } => {
                let source_str = source.as_deref().unwrap_or("*");
                Line::from(vec![
                    time_span,
                    Span::styled(
                        format!("-{}- ", source_str),
                        self.theme.server_style(),
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
                    Span::styled(
                        format!("→ {} has joined{}", nick, host_str),
                        self.theme.join_part_style(),
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
                    Span::styled(
                        format!("← {} has left{}", nick, msg_str),
                        self.theme.join_part_style(),
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
                    Span::styled(
                        format!("← {} has quit{}", nick, msg_str),
                        self.theme.join_part_style(),
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
                    Span::styled(
                        format!("← {} was kicked by {}{}", nick, kicker, reason_str),
                        self.theme.join_part_style(),
                    ),
                ])
            }

            MessageKind::Nick { old_nick, new_nick } => {
                Line::from(vec![
                    time_span,
                    Span::styled(
                        format!("* {} is now known as {}", old_nick, new_nick),
                        self.theme.join_part_style(),
                    ),
                ])
            }

            MessageKind::Topic { setter, topic } => {
                let setter_str = setter.as_deref().unwrap_or("Someone");
                let topic_str = topic.as_deref().unwrap_or("(cleared)");
                Line::from(vec![
                    time_span,
                    Span::styled(
                        format!("* {} set topic: {}", setter_str, topic_str),
                        self.theme.server_style(),
                    ),
                ])
            }

            MessageKind::Mode { setter, modes } => {
                Line::from(vec![
                    time_span,
                    Span::styled(
                        format!("* {} sets mode: {}", setter, modes),
                        self.theme.server_style(),
                    ),
                ])
            }

            MessageKind::Server { text } => {
                Line::from(vec![
                    time_span,
                    Span::styled(text.clone(), self.theme.server_style()),
                ])
            }

            MessageKind::Error { text } => {
                Line::from(vec![
                    time_span,
                    Span::styled(text.clone(), self.theme.error_style()),
                ])
            }

            MessageKind::HistorySeparator => {
                Line::from(vec![
                    Span::styled(
                        "──────────── History ────────────",
                        self.theme.muted_style(),
                    ),
                ])
            }
        }
    }
}

impl Widget for MessagesWidget<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let messages: Vec<_> = self.buffer.messages().collect();

        // Calculate visible range with scroll
        let visible_height = area.height.saturating_sub(2) as usize; // Account for borders
        let total = messages.len();
        let scroll_offset = self.buffer.scroll_offset.min(total.saturating_sub(visible_height));

        let start = total.saturating_sub(visible_height + scroll_offset);
        let end = total.saturating_sub(scroll_offset);

        let lines: Vec<Line> = messages[start..end]
            .iter()
            .map(|m| self.format_message(m))
            .collect();

        let title = if self.buffer.scroll_offset > 0 {
            format!(" {} (scrolled) ", self.buffer.name)
        } else {
            format!(" {} ", self.buffer.name)
        };

        let paragraph = Paragraph::new(lines)
            .block(Block::default().borders(Borders::NONE).title(title))
            .wrap(Wrap { trim: false });

        Widget::render(paragraph, area, buf);
    }
}
