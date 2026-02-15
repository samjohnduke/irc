//! Channel list modal widget.

use ratatui::{
    buffer::Buffer,
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Padding, Row, Table, Widget},
};

use crate::style::Theme;

/// A channel entry from LIST response.
#[derive(Debug, Clone)]
pub struct ChannelEntry {
    pub name: String,
    pub users: u32,
    pub topic: String,
}

impl ChannelEntry {
    pub fn new(name: String, users: u32, topic: String) -> Self {
        Self { name, users, topic }
    }
}

/// Maximum channels to accept (prevents overload on huge networks).
const MAX_CHANNELS: usize = 5000;

/// State for the channel list modal.
#[derive(Debug, Default)]
pub struct ChannelListState {
    /// All channels received from server.
    pub channels: Vec<ChannelEntry>,
    /// Current filter text.
    pub filter: String,
    /// Currently selected index (in filtered list).
    pub selected: usize,
    /// Whether the list is still loading.
    pub loading: bool,
    /// Whether the modal is visible.
    pub visible: bool,
    /// Scroll offset for the list.
    pub scroll_offset: usize,
    /// Whether we hit the channel limit.
    pub truncated: bool,
}

impl ChannelListState {
    pub fn new() -> Self {
        Self::default()
    }

    /// Open the modal and start loading.
    pub fn open(&mut self) {
        self.visible = true;
        self.loading = true;
        self.channels.clear();
        self.filter.clear();
        self.selected = 0;
        self.scroll_offset = 0;
        self.truncated = false;
    }

    /// Close the modal.
    pub fn close(&mut self) {
        self.visible = false;
        self.loading = false;
    }

    /// Add a channel entry (respects MAX_CHANNELS limit).
    pub fn add_channel(&mut self, entry: ChannelEntry) {
        if self.channels.len() < MAX_CHANNELS {
            self.channels.push(entry);
        } else {
            self.truncated = true;
        }
    }

    /// Mark loading as complete.
    pub fn finish_loading(&mut self) {
        self.loading = false;
        // Sort by user count descending
        self.channels.sort_by(|a, b| b.users.cmp(&a.users));
    }

    /// Get filtered channels.
    pub fn filtered_channels(&self) -> Vec<&ChannelEntry> {
        if self.filter.is_empty() {
            self.channels.iter().collect()
        } else {
            // Strip wildcards for client-side filtering since the server
            // already did wildcard matching. This allows typing "*rust*"
            // to work both as a server pattern and client filter.
            let filter_clean = self.filter.trim_matches('*').to_lowercase();
            if filter_clean.is_empty() {
                return self.channels.iter().collect();
            }
            self.channels
                .iter()
                .filter(|c| {
                    c.name.to_lowercase().contains(&filter_clean)
                        || c.topic.to_lowercase().contains(&filter_clean)
                })
                .collect()
        }
    }

    /// Get the currently selected channel.
    pub fn selected_channel(&self) -> Option<&ChannelEntry> {
        let filtered = self.filtered_channels();
        filtered.get(self.selected).copied()
    }

    /// Move selection up.
    pub fn select_prev(&mut self) {
        if self.selected > 0 {
            self.selected -= 1;
            // Adjust scroll if needed
            if self.selected < self.scroll_offset {
                self.scroll_offset = self.selected;
            }
        }
    }

    /// Move selection down.
    pub fn select_next(&mut self) {
        let filtered_len = self.filtered_channels().len();
        if filtered_len > 0 && self.selected < filtered_len - 1 {
            self.selected += 1;
        }
    }

    /// Handle filter input change.
    pub fn set_filter(&mut self, filter: String) {
        self.filter = filter;
        self.selected = 0;
        self.scroll_offset = 0;
    }

    /// Insert character into filter.
    pub fn filter_insert(&mut self, c: char) {
        self.filter.push(c);
        self.selected = 0;
        self.scroll_offset = 0;
    }

    /// Delete character from filter.
    pub fn filter_backspace(&mut self) {
        self.filter.pop();
        self.selected = 0;
        self.scroll_offset = 0;
    }

    /// Clear the filter.
    pub fn filter_clear(&mut self) {
        self.filter.clear();
        self.selected = 0;
        self.scroll_offset = 0;
    }

    /// Start a new search (clears results and sets loading).
    pub fn start_search(&mut self) {
        self.channels.clear();
        self.loading = true;
        self.selected = 0;
        self.scroll_offset = 0;
        self.truncated = false;
    }

    /// Get the current filter for sending to server.
    pub fn get_search_filter(&self) -> Option<String> {
        if self.filter.is_empty() {
            None
        } else {
            Some(self.filter.clone())
        }
    }
}

/// Channel list modal widget.
pub struct ChannelListWidget<'a> {
    state: &'a ChannelListState,
    theme: &'a Theme,
}

impl<'a> ChannelListWidget<'a> {
    pub fn new(state: &'a ChannelListState, theme: &'a Theme) -> Self {
        Self { state, theme }
    }
}

impl Widget for ChannelListWidget<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        // Calculate centered popup area (90% width, 85% height)
        let popup_width = (area.width * 90 / 100).clamp(60, 120);
        let popup_height = (area.height * 85 / 100).clamp(15, 40);
        let popup_x = area.x + (area.width.saturating_sub(popup_width)) / 2;
        let popup_y = area.y + (area.height.saturating_sub(popup_height)) / 2;

        let popup_area = Rect::new(popup_x, popup_y, popup_width, popup_height);

        // Clear the popup area
        Widget::render(Clear, popup_area, buf);

        // Background
        let bg = Color::Rgb(25, 28, 38);
        let border_color = Color::Rgb(80, 120, 200);
        let header_bg = Color::Rgb(35, 45, 65);

        // Main block
        let title = if self.state.loading {
            format!(
                " Channel List (loading... {} so far) ",
                self.state.channels.len()
            )
        } else if self.state.truncated {
            format!(
                " Channel List ({}+ channels, truncated) ",
                self.state.channels.len()
            )
        } else {
            format!(" Channel List ({} channels) ", self.state.channels.len())
        };

        let block = Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(border_color))
            .title(Span::styled(
                title,
                Style::default()
                    .fg(Color::Rgb(150, 200, 255))
                    .add_modifier(Modifier::BOLD),
            ))
            .title_alignment(Alignment::Center)
            .style(Style::default().bg(bg))
            .padding(Padding::new(1, 1, 0, 0));

        let inner = block.inner(popup_area);
        Widget::render(block, popup_area, buf);

        // Layout: filter input, table, footer
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(3), // Filter input
                Constraint::Min(5),    // Channel table
                Constraint::Length(2), // Footer
            ])
            .split(inner);

        // === Filter input ===
        let filter_area = chunks[0];

        // Filter background
        for y in filter_area.y..filter_area.y + filter_area.height {
            for x in filter_area.x..filter_area.x + filter_area.width {
                buf[(x, y)].set_bg(header_bg);
            }
        }

        let filter_block = Block::default()
            .borders(Borders::BOTTOM)
            .border_style(Style::default().fg(Color::Rgb(60, 70, 90)))
            .style(Style::default().bg(header_bg));

        let filter_inner = filter_block.inner(filter_area);
        Widget::render(filter_block, filter_area, buf);

        // Filter label and input
        let filter_line = Line::from(vec![
            Span::styled(" Filter: ", Style::default().fg(Color::Rgb(150, 150, 170))),
            Span::styled(
                &self.state.filter,
                Style::default()
                    .fg(Color::Rgb(255, 255, 255))
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                "▌",
                Style::default()
                    .fg(self.theme.accent)
                    .add_modifier(Modifier::SLOW_BLINK),
            ),
        ]);
        buf.set_line(
            filter_inner.x,
            filter_inner.y,
            &filter_line,
            filter_inner.width,
        );

        // Filter hint
        let filtered = self.state.filtered_channels();
        let hint = format!(" {} matching ", filtered.len());
        let hint_len = hint.len();
        let hint_x = filter_inner.x + filter_inner.width - hint_len as u16 - 1;
        let hint_span = Span::styled(hint, Style::default().fg(Color::Rgb(100, 110, 130)));
        buf.set_span(hint_x, filter_inner.y, &hint_span, hint_len as u16);

        // === Channel table ===
        let table_area = chunks[1];

        // Calculate visible rows
        let visible_height = table_area.height.saturating_sub(1) as usize; // -1 for header

        // Adjust scroll to keep selection visible
        let scroll_offset = if self.state.selected >= self.state.scroll_offset + visible_height {
            self.state.selected.saturating_sub(visible_height - 1)
        } else if self.state.selected < self.state.scroll_offset {
            self.state.selected
        } else {
            self.state.scroll_offset
        };

        // Build rows
        let rows: Vec<Row> = filtered
            .iter()
            .skip(scroll_offset)
            .take(visible_height)
            .enumerate()
            .map(|(i, channel)| {
                let actual_idx = scroll_offset + i;
                let is_selected = actual_idx == self.state.selected;

                let style = if is_selected {
                    Style::default()
                        .bg(Color::Rgb(50, 70, 100))
                        .fg(Color::Rgb(255, 255, 255))
                        .add_modifier(Modifier::BOLD)
                } else {
                    Style::default().fg(Color::Rgb(200, 200, 210))
                };

                // Truncate topic if needed (use chars to handle Unicode properly)
                let max_topic_len = (popup_width as usize).saturating_sub(35);
                let topic_chars: Vec<char> = channel.topic.chars().collect();
                let topic = if topic_chars.len() > max_topic_len {
                    let truncated: String = topic_chars[..max_topic_len.saturating_sub(1)]
                        .iter()
                        .collect();
                    format!("{}…", truncated)
                } else {
                    channel.topic.clone()
                };

                Row::new(vec![
                    format!(" {}", channel.name),
                    format!("{:>6}", channel.users),
                    topic,
                ])
                .style(style)
            })
            .collect();

        // Header
        let header = Row::new(vec![" Channel", " Users", "Topic"])
            .style(
                Style::default()
                    .fg(Color::Rgb(150, 180, 220))
                    .add_modifier(Modifier::BOLD),
            )
            .bottom_margin(0);

        let table = Table::new(
            rows,
            [
                Constraint::Length(25),
                Constraint::Length(8),
                Constraint::Min(20),
            ],
        )
        .header(header)
        .style(Style::default().bg(bg));

        Widget::render(table, table_area, buf);

        // === Footer ===
        let footer_area = chunks[2];

        // Footer background
        for y in footer_area.y..footer_area.y + footer_area.height {
            for x in footer_area.x..footer_area.x + footer_area.width {
                buf[(x, y)].set_bg(header_bg);
            }
        }

        let footer_line = Line::from(vec![
            Span::styled("Tab ", Style::default().fg(Color::Rgb(200, 180, 100))),
            Span::styled("Search  ", Style::default().fg(Color::Rgb(150, 150, 170))),
            Span::styled("↑↓ ", Style::default().fg(Color::Rgb(200, 180, 100))),
            Span::styled("Navigate  ", Style::default().fg(Color::Rgb(150, 150, 170))),
            Span::styled("Enter ", Style::default().fg(Color::Rgb(200, 180, 100))),
            Span::styled("Join  ", Style::default().fg(Color::Rgb(150, 150, 170))),
            Span::styled("Esc ", Style::default().fg(Color::Rgb(200, 180, 100))),
            Span::styled("Close", Style::default().fg(Color::Rgb(150, 150, 170))),
        ]);

        buf.set_line(
            footer_area.x + 1,
            footer_area.y,
            &footer_line,
            footer_area.width.saturating_sub(2),
        );

        // Scroll indicator on the right
        if filtered.len() > visible_height {
            let scroll_info = format!(
                " {}-{} of {} ",
                scroll_offset + 1,
                (scroll_offset + visible_height).min(filtered.len()),
                filtered.len()
            );
            let scroll_info_len = scroll_info.len();
            let scroll_x = footer_area.x + footer_area.width - scroll_info_len as u16 - 1;
            let scroll_span =
                Span::styled(scroll_info, Style::default().fg(Color::Rgb(100, 110, 130)));
            buf.set_span(
                scroll_x,
                footer_area.y,
                &scroll_span,
                scroll_info_len as u16,
            );
        }
    }
}
