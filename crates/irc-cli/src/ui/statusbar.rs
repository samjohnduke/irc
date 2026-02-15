//! Status bar widget (2-line spacious header).

use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::Widget,
};

use crate::style::Theme;

/// Status bar showing connection info, nick, and channel info.
/// Now renders as a 2-line spacious header.
pub struct StatusbarWidget<'a> {
    nick: &'a str,
    channel: Option<&'a str>,
    topic: Option<&'a str>,
    connected: bool,
    theme: &'a Theme,
    show_help_hint: bool,
    user_count: Option<usize>,
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
            show_help_hint: true,
            user_count: None,
        }
    }

    #[allow(dead_code)]
    pub fn with_help_hint(mut self, show: bool) -> Self {
        self.show_help_hint = show;
        self
    }

    pub fn with_user_count(mut self, count: Option<usize>) -> Self {
        self.user_count = count;
        self
    }
}

impl Widget for StatusbarWidget<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        if area.height < 2 {
            // Fallback to single line if not enough space
            self.render_single_line(area, buf);
            return;
        }

        // Two-line header
        let header_bg = Color::Rgb(30, 35, 50);
        let header_accent_bg = Color::Rgb(40, 50, 70);

        // Fill background
        for y in area.y..area.y + area.height {
            for x in area.x..area.x + area.width {
                buf[(x, y)].set_bg(header_bg);
                buf[(x, y)].set_char(' ');
            }
        }

        // === LINE 1: Main info (nick, channel, connection status) ===
        let y1 = area.y;

        let mut spans = Vec::new();
        spans.push(Span::raw(" "));

        // Connection status with icon
        if self.connected {
            spans.push(Span::styled(
                "●",
                Style::default().fg(Color::Rgb(100, 220, 100)),
            ));
            spans.push(Span::styled(
                " Connected",
                Style::default().fg(Color::Rgb(140, 200, 140)),
            ));
        } else {
            spans.push(Span::styled(
                "○",
                Style::default().fg(Color::Rgb(255, 100, 100)),
            ));
            spans.push(Span::styled(
                " Disconnected",
                Style::default().fg(Color::Rgb(255, 140, 140)),
            ));
        }

        spans.push(Span::styled(
            "  │  ",
            Style::default().fg(self.theme.border),
        ));

        // Nick
        spans.push(Span::styled(
            "Nick: ",
            Style::default().fg(self.theme.muted),
        ));
        spans.push(Span::styled(
            self.nick,
            Style::default()
                .fg(self.theme.accent)
                .add_modifier(Modifier::BOLD),
        ));

        // Channel/buffer
        if let Some(channel) = self.channel {
            spans.push(Span::styled(
                "  │  ",
                Style::default().fg(self.theme.border),
            ));

            // Channel icon
            if let Some(channel_name) = channel.strip_prefix('#') {
                spans.push(Span::styled(
                    "#",
                    Style::default()
                        .fg(Color::Rgb(255, 200, 100))
                        .add_modifier(Modifier::BOLD),
                ));
                spans.push(Span::styled(
                    channel_name,
                    Style::default()
                        .fg(Color::Rgb(255, 255, 255))
                        .add_modifier(Modifier::BOLD),
                ));
            } else {
                spans.push(Span::styled(
                    channel,
                    Style::default()
                        .fg(Color::Rgb(255, 255, 255))
                        .add_modifier(Modifier::BOLD),
                ));
            }

            // User count
            if let Some(count) = self.user_count {
                spans.push(Span::styled(
                    format!(" ({} users)", count),
                    Style::default().fg(self.theme.muted),
                ));
            }
        }

        let line1 = Line::from(spans);
        buf.set_line(area.x, y1, &line1, area.width);

        // Right side: help hint
        if self.show_help_hint {
            let hint = " F1 Help ";
            let hint_style = Style::default().fg(self.theme.muted).bg(header_accent_bg);
            let hint_x = area.x + area.width - hint.len() as u16;
            let hint_span = Span::styled(hint, hint_style);
            buf.set_span(hint_x, y1, &hint_span, hint.len() as u16);
        }

        // === LINE 2: Topic or secondary info ===
        let y2 = area.y + 1;

        // Slightly different background for visual separation
        for x in area.x..area.x + area.width {
            buf[(x, y2)].set_bg(header_accent_bg);
        }

        let mut line2_spans = Vec::new();
        line2_spans.push(Span::raw(" "));

        if let Some(topic) = self.topic {
            if !topic.is_empty() {
                line2_spans.push(Span::styled(
                    "Topic: ",
                    Style::default().fg(self.theme.muted),
                ));

                // Calculate remaining space for topic
                let used_width = 8; // " Topic: "
                let remaining = (area.width as usize).saturating_sub(used_width + 2);

                let display_topic = if topic.len() > remaining && remaining > 3 {
                    format!("{}…", &topic[..remaining.saturating_sub(1)])
                } else {
                    topic.to_string()
                };

                line2_spans.push(Span::styled(
                    display_topic,
                    Style::default()
                        .fg(Color::Rgb(200, 200, 210))
                        .add_modifier(Modifier::ITALIC),
                ));
            } else {
                line2_spans.push(Span::styled(
                    "No topic set",
                    Style::default().fg(self.theme.muted),
                ));
            }
        } else if self.channel.is_none() {
            // Server buffer
            line2_spans.push(Span::styled(
                "Server messages and notices",
                Style::default().fg(self.theme.muted),
            ));
        } else {
            line2_spans.push(Span::styled(
                "Private conversation",
                Style::default().fg(self.theme.muted),
            ));
        }

        let line2 = Line::from(line2_spans);
        buf.set_line(area.x, y2, &line2, area.width);

        // Right side of line 2: keyboard shortcuts
        let shortcuts = " PgUp/PgDn Scroll │ Ctrl+N/P Switch ";
        let shortcuts_style = Style::default().fg(self.theme.muted);
        let shortcuts_x = area.x + area.width - shortcuts.len() as u16;
        if shortcuts_x > area.x + 20 {
            let shortcuts_span = Span::styled(shortcuts, shortcuts_style);
            buf.set_span(shortcuts_x, y2, &shortcuts_span, shortcuts.len() as u16);
        }
    }
}

impl StatusbarWidget<'_> {
    fn render_single_line(self, area: Rect, buf: &mut Buffer) {
        // Fallback single-line rendering
        let status_style = self.theme.statusbar_style();

        for x in area.x..area.x + area.width {
            buf[(x, area.y)].set_style(status_style);
            buf[(x, area.y)].set_char(' ');
        }

        let mut spans = Vec::new();

        if self.connected {
            spans.push(Span::styled(
                " ● ",
                Style::default().fg(Color::Rgb(100, 220, 100)),
            ));
        } else {
            spans.push(Span::styled(
                " ○ ",
                Style::default().fg(Color::Rgb(255, 100, 100)),
            ));
        }

        spans.push(Span::styled(
            format!("[{}]", self.nick),
            status_style.add_modifier(Modifier::BOLD),
        ));

        if let Some(channel) = self.channel {
            spans.push(Span::styled(format!(" {}", channel), status_style));
        }

        let line = Line::from(spans);
        buf.set_line(area.x, area.y, &line, area.width);
    }
}
