//! Channel/buffer sidebar widget.

use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, Widget},
};

use crate::state::BufferList;
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
        let items: Vec<ListItem> = self
            .buffers
            .all()
            .iter()
            .enumerate()
            .map(|(i, buffer)| {
                let is_active = i == self.buffers.active_index();
                let has_unread = buffer.unread_count > 0;
                let has_highlight = buffer.has_highlight;

                let mut spans = Vec::new();

                // Unread indicator
                if has_highlight {
                    spans.push(Span::styled("! ", Style::default().fg(Color::Red).add_modifier(Modifier::BOLD)));
                } else if has_unread {
                    spans.push(Span::styled("● ", self.theme.unread_style()));
                } else {
                    spans.push(Span::raw("  "));
                }

                // Buffer name with appropriate style
                let name_style = if is_active {
                    self.theme.selected_style()
                } else if has_highlight {
                    Style::default().fg(Color::Red).add_modifier(Modifier::BOLD)
                } else if has_unread {
                    self.theme.unread_style()
                } else {
                    Style::default()
                };

                spans.push(Span::styled(buffer.display_name(), name_style));

                // Unread count
                if has_unread && buffer.unread_count > 0 {
                    spans.push(Span::styled(
                        format!(" ({})", buffer.unread_count),
                        self.theme.muted_style(),
                    ));
                }

                ListItem::new(Line::from(spans))
            })
            .collect();

        let list = List::new(items)
            .block(Block::default().borders(Borders::RIGHT).title("Buffers"));

        Widget::render(list, area, buf);
    }
}
