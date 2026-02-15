//! Tab bar widget (bottom bar showing buffers like tiny/weechat).

#![allow(dead_code)]

use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::Widget,
};

use crate::state::BufferList;

/// Tab bar showing all buffers at the bottom of the screen.
pub struct TabBarWidget<'a> {
    buffers: &'a BufferList,
}

impl<'a> TabBarWidget<'a> {
    pub fn new(buffers: &'a BufferList) -> Self {
        Self { buffers }
    }
}

impl Widget for TabBarWidget<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        if area.height < 1 {
            return;
        }

        // Background
        let bg = Color::Rgb(30, 32, 40);
        for x in area.x..area.x + area.width {
            buf[(x, area.y)].set_bg(bg);
            buf[(x, area.y)].set_char(' ');
        }

        let mut spans = Vec::new();
        let active_idx = self.buffers.active_index();

        for (i, buffer) in self.buffers.all().iter().enumerate() {
            let is_active = i == active_idx;
            let has_highlight = buffer.has_highlight;
            let has_unread = buffer.unread_count > 0;

            // Tab number
            let num_style = Style::default().fg(Color::Rgb(100, 100, 120));
            spans.push(Span::styled(format!("{}", i + 1), num_style));

            // Separator
            spans.push(Span::styled(
                ":",
                Style::default().fg(Color::Rgb(60, 60, 70)),
            ));

            // Buffer name with styling
            let name = if buffer.name.len() > 12 {
                format!("{}…", &buffer.name[..11])
            } else {
                buffer.name.clone()
            };

            let name_style = if is_active {
                Style::default()
                    .fg(Color::White)
                    .add_modifier(Modifier::BOLD)
            } else if has_highlight {
                Style::default()
                    .fg(Color::Rgb(255, 100, 100))
                    .add_modifier(Modifier::BOLD)
            } else if has_unread {
                Style::default().fg(Color::Rgb(100, 180, 255))
            } else {
                Style::default().fg(Color::Rgb(140, 140, 150))
            };

            spans.push(Span::styled(name, name_style));

            // Activity indicator
            if has_highlight {
                spans.push(Span::styled(
                    "*",
                    Style::default().fg(Color::Rgb(255, 100, 100)),
                ));
            } else if has_unread {
                spans.push(Span::styled(
                    "+",
                    Style::default().fg(Color::Rgb(100, 180, 255)),
                ));
            }

            // Spacer between tabs
            spans.push(Span::raw("  "));
        }

        let line = Line::from(spans);
        buf.set_line(area.x + 1, area.y, &line, area.width.saturating_sub(2));
    }
}
