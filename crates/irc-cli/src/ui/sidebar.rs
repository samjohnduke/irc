//! Channel/buffer sidebar widget.

use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Color, Modifier, Style},
    symbols,
    text::{Line, Span},
    widgets::{Block, Borders, Widget},
};

use crate::state::{BufferKind, BufferList};
use crate::style::Theme;

/// Widget for displaying the buffer list sidebar.
pub struct SidebarWidget<'a> {
    buffers: &'a BufferList,
    theme: &'a Theme,
}

impl<'a> SidebarWidget<'a> {
    pub fn new(buffers: &'a BufferList, theme: &'a Theme) -> Self {
        Self { buffers, theme }
    }
}

impl Widget for SidebarWidget<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        // Sidebar background (slightly different from main)
        let sidebar_bg = Color::Rgb(25, 25, 35);

        // Fill background
        for y in area.y..area.y + area.height {
            for x in area.x..area.x + area.width {
                buf[(x, y)].set_bg(sidebar_bg);
            }
        }

        // Draw border
        let block = Block::default()
            .borders(Borders::RIGHT)
            .border_style(self.theme.border_style())
            .border_set(symbols::border::PLAIN)
            .style(Style::default().bg(sidebar_bg));

        let inner = block.inner(area);
        Widget::render(block, area, buf);

        // Separate buffers by type
        let server: Vec<_> = self
            .buffers
            .all()
            .iter()
            .enumerate()
            .filter(|(_, b)| b.kind == BufferKind::Server)
            .collect();
        let channels: Vec<_> = self
            .buffers
            .all()
            .iter()
            .enumerate()
            .filter(|(_, b)| b.kind == BufferKind::Channel)
            .collect();
        let queries: Vec<_> = self
            .buffers
            .all()
            .iter()
            .enumerate()
            .filter(|(_, b)| b.kind == BufferKind::Query)
            .collect();

        let mut y = inner.y;
        let max_y = inner.y + inner.height;

        // Helper to render a section
        let render_section =
            |buf: &mut Buffer,
             y: &mut u16,
             title: &str,
             items: &[(usize, &crate::state::Buffer)]| {
                if items.is_empty() || *y >= max_y {
                    return;
                }

                // Section header
                let header_style = Style::default()
                    .fg(Color::Rgb(100, 110, 140))
                    .add_modifier(Modifier::BOLD);

                let header = Line::from(Span::styled(format!(" {} ", title), header_style));
                buf.set_line(inner.x, *y, &header, inner.width);
                *y += 1;

                // Items
                for (idx, buffer) in items.iter() {
                    if *y >= max_y {
                        break;
                    }

                    let is_active = *idx == self.buffers.active_index();
                    let has_unread = buffer.unread_count > 0;
                    let has_highlight = buffer.has_highlight;

                    let mut spans = Vec::new();

                    // Selection indicator (left bar)
                    if is_active {
                        spans.push(Span::styled("▌", Style::default().fg(self.theme.accent)));
                    } else {
                        spans.push(Span::raw(" "));
                    }

                    // Unread/highlight indicator
                    if has_highlight {
                        spans.push(Span::styled(
                            "●",
                            Style::default()
                                .fg(Color::Rgb(255, 100, 100))
                                .add_modifier(Modifier::BOLD),
                        ));
                    } else if has_unread {
                        spans.push(Span::styled("●", self.theme.unread_style()));
                    } else {
                        spans.push(Span::styled(
                            "○",
                            Style::default().fg(Color::Rgb(60, 65, 80)),
                        ));
                    }
                    spans.push(Span::raw(" "));

                    // Buffer name with icon
                    let (icon, name) = match buffer.kind {
                        BufferKind::Server => ("◈ ", buffer.name.as_str()),
                        BufferKind::Channel => {
                            if buffer.name.starts_with('#') {
                                ("", buffer.name.as_str())
                            } else {
                                ("# ", buffer.name.as_str())
                            }
                        }
                        BufferKind::Query => ("⊕ ", buffer.name.as_str()),
                    };

                    // Style based on state
                    // Different colors for: active, highlight/mention, private msg unread, channel unread, inactive
                    let is_private = buffer.kind == BufferKind::Query;
                    let name_style = if is_active {
                        Style::default()
                            .fg(Color::Rgb(255, 255, 255))
                            .add_modifier(Modifier::BOLD)
                    } else if has_highlight {
                        // Mentions/highlights: accent color with bold
                        Style::default()
                            .fg(self.theme.accent)
                            .add_modifier(Modifier::BOLD)
                    } else if has_unread && is_private {
                        // Private message unread: magenta/pink
                        Style::default()
                            .fg(Color::Rgb(255, 140, 200))
                            .add_modifier(Modifier::BOLD)
                    } else if has_unread {
                        // Regular unread: bright white
                        Style::default()
                            .fg(Color::Rgb(240, 240, 250))
                            .add_modifier(Modifier::BOLD)
                    } else {
                        // Inactive: muted
                        Style::default().fg(Color::Rgb(120, 125, 140))
                    };

                    // Icon
                    if !icon.is_empty() {
                        spans.push(Span::styled(icon, Style::default().fg(self.theme.muted)));
                    }

                    // Truncate name if needed
                    let max_name_len = inner.width.saturating_sub(7) as usize;
                    let display_name = if name.len() > max_name_len {
                        format!("{}…", &name[..max_name_len.saturating_sub(1)])
                    } else {
                        name.to_string()
                    };

                    spans.push(Span::styled(display_name, name_style));

                    // Unread count badge
                    if has_unread && buffer.unread_count > 0 {
                        let remaining_space = inner
                            .width
                            .saturating_sub(spans.iter().map(|s| s.width() as u16).sum::<u16>() + 1)
                            as usize;

                        let count_text = if buffer.unread_count > 99 {
                            "99+".to_string()
                        } else {
                            buffer.unread_count.to_string()
                        };

                        if remaining_space > count_text.len() {
                            spans.push(Span::styled(
                                format!(" {}", count_text),
                                Style::default().fg(self.theme.accent),
                            ));
                        }
                    }

                    let line = Line::from(spans);

                    // Highlight background for active item
                    if is_active {
                        for x in inner.x..inner.x + inner.width {
                            buf[(x, *y)].set_bg(Color::Rgb(40, 50, 70));
                        }
                    }

                    buf.set_line(inner.x, *y, &line, inner.width);
                    *y += 1;
                }

                // Spacing after section
                *y += 1;
            };

        // Render sections
        render_section(buf, &mut y, "SERVER", &server);
        render_section(buf, &mut y, "CHANNELS", &channels);
        render_section(buf, &mut y, "PRIVATE", &queries);

        // Footer with buffer count
        if area.height > 3 {
            let footer_y = area.y + area.height - 1;
            let total = self.buffers.all().len();
            let footer_text = format!(" {} buffers ", total);
            let footer_style = Style::default().fg(self.theme.muted);
            let footer_span = Span::styled(footer_text, footer_style);
            buf.set_span(inner.x, footer_y, &footer_span, inner.width);
        }
    }
}
