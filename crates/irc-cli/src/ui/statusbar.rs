//! Status bar widget.

use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::Widget,
};

use crate::style::Theme;

/// Status bar showing connection info, nick, and channel info.
pub struct StatusbarWidget<'a> {
    nick: &'a str,
    channel: Option<&'a str>,
    topic: Option<&'a str>,
    connected: bool,
    theme: &'a Theme,
}

impl<'a> StatusbarWidget<'a> {
    pub fn new(
        nick: &'a str,
        channel: Option<&'a str>,
        topic: Option<&'a str>,
        connected: bool,
        theme: &'a Theme,
    ) -> Self {
        Self {
            nick,
            channel,
            topic,
            connected,
            theme,
        }
    }
}

impl Widget for StatusbarWidget<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        // Clear the area with inverted colors
        let status_style = Style::default()
            .bg(self.theme.fg)
            .fg(self.theme.bg)
            .add_modifier(Modifier::BOLD);

        for x in area.x..area.x + area.width {
            buf[(x, area.y)].set_style(status_style);
            buf[(x, area.y)].set_char(' ');
        }

        let mut spans = Vec::new();

        // Connection status
        if self.connected {
            spans.push(Span::styled(" ● ", Style::default().fg(ratatui::style::Color::Green)));
        } else {
            spans.push(Span::styled(" ○ ", Style::default().fg(ratatui::style::Color::Red)));
        }

        // Nick
        spans.push(Span::styled(
            format!("[{}]", self.nick),
            status_style.add_modifier(Modifier::BOLD),
        ));

        // Channel/buffer name
        if let Some(channel) = self.channel {
            spans.push(Span::styled(" ", status_style));
            spans.push(Span::styled(channel, status_style));
        }

        // Topic (truncated if too long)
        if let Some(topic) = self.topic {
            spans.push(Span::styled(" | ", status_style));

            // Calculate remaining space
            let used_width: usize = spans.iter().map(|s| s.width()).sum();
            let remaining = area.width as usize - used_width - 1;

            let display_topic = if topic.len() > remaining {
                format!("{}...", &topic[..remaining.saturating_sub(3)])
            } else {
                topic.to_string()
            };

            spans.push(Span::styled(display_topic, status_style));
        }

        let line = Line::from(spans);
        buf.set_line(area.x, area.y, &line, area.width);
    }
}
