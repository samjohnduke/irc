//! Main layout composition.

use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    symbols,
    text::{Line, Span},
    widgets::{Block, Borders},
};

use crate::handler::input::InputMode;
use crate::state::BufferList;
use crate::style::Theme;
use crate::ui::input::InputState;
use crate::ui::messages::MessagesWidget;
use crate::ui::sidebar::SidebarWidget;
use crate::ui::statusbar::StatusbarWidget;
use crate::ui::userlist::{ChannelUser, EmptyUserListWidget, UserListWidget};

/// Search state for rendering.
pub struct SearchRenderState {
    pub active: bool,
    pub query: String,
    pub current_match: usize,
    pub total_matches: usize,
}

/// Layout configuration.
pub struct LayoutConfig {
    /// Sidebar width (0 to hide).
    pub sidebar_width: u16,
    /// User list width (0 to hide).
    pub userlist_width: u16,
    /// Show status bar.
    pub show_statusbar: bool,
    /// Input area height.
    pub input_height: u16,
}

impl Default for LayoutConfig {
    fn default() -> Self {
        Self {
            sidebar_width: 22,
            userlist_width: 20,
            show_statusbar: true,
            input_height: 3,
        }
    }
}

/// Draw the complete UI layout.
#[allow(dead_code)]
pub fn draw_layout(
    frame: &mut Frame,
    buffers: &BufferList,
    input: &InputState,
    nick: &str,
    connected: bool,
    theme: &Theme,
    config: &LayoutConfig,
    channel_users: &[ChannelUser],
) {
    draw_layout_with_mode(
        frame,
        buffers,
        input,
        nick,
        connected,
        theme,
        config,
        channel_users,
        InputMode::Insert,
        None,
    )
}

/// Draw the complete UI layout with vim mode and search state.
pub fn draw_layout_with_mode(
    frame: &mut Frame,
    buffers: &BufferList,
    input: &InputState,
    nick: &str,
    connected: bool,
    theme: &Theme,
    config: &LayoutConfig,
    channel_users: &[ChannelUser],
    input_mode: InputMode,
    search: Option<&SearchRenderState>,
) {
    let area = frame.area();

    // Fill entire background
    let bg = theme.bg;
    for y in area.y..area.y + area.height {
        for x in area.x..area.x + area.width {
            frame.buffer_mut()[(x, y)].set_bg(bg);
        }
    }

    // Main vertical layout: header, content, input
    let mut constraints = vec![];

    // Header/statusbar area (2 lines for spacious look)
    if config.show_statusbar {
        constraints.push(Constraint::Length(2));
    }

    // Content area (flexible)
    constraints.push(Constraint::Min(5));

    // Input area (with border)
    constraints.push(Constraint::Length(config.input_height));

    let vertical = Layout::default()
        .direction(Direction::Vertical)
        .constraints(constraints)
        .split(area);

    let mut slot = 0;

    // Header bar (2 lines, spacious)
    if config.show_statusbar {
        let active = buffers.active();
        let channel = if active.is_server() {
            None
        } else {
            Some(active.name.as_str())
        };
        let topic = active.topic.as_deref();

        let statusbar = StatusbarWidget::new(nick, channel, topic, connected, theme)
            .with_user_count(if active.is_channel() {
                Some(channel_users.len())
            } else {
                None
            });
        frame.render_widget(statusbar, vertical[slot]);
        slot += 1;
    }

    // Content area: sidebar + messages + userlist
    let content_area = vertical[slot];
    slot += 1;

    // Determine if we should show user list (only for channels)
    let active = buffers.active();
    let show_userlist = active.is_channel() && config.userlist_width > 0;

    // Build horizontal constraints
    let mut h_constraints = vec![];
    if config.sidebar_width > 0 {
        h_constraints.push(Constraint::Length(config.sidebar_width));
    }
    h_constraints.push(Constraint::Min(30)); // Messages area
    if show_userlist {
        h_constraints.push(Constraint::Length(config.userlist_width));
    }

    let horizontal = Layout::default()
        .direction(Direction::Horizontal)
        .constraints(h_constraints)
        .split(content_area);

    let mut h_slot = 0;

    // Sidebar
    if config.sidebar_width > 0 {
        let sidebar = SidebarWidget::new(buffers, theme);
        frame.render_widget(sidebar, horizontal[h_slot]);
        h_slot += 1;
    }

    // Messages
    let messages = MessagesWidget::new(buffers.active(), theme);
    frame.render_widget(messages, horizontal[h_slot]);
    h_slot += 1;

    // User list (only for channels)
    if show_userlist {
        if channel_users.is_empty() {
            let empty = EmptyUserListWidget::new(theme);
            frame.render_widget(empty, horizontal[h_slot]);
        } else {
            let userlist = UserListWidget::new(channel_users, Some(&active.name), theme);
            frame.render_widget(userlist, horizontal[h_slot]);
        }
    }

    // Input area with border
    let input_area = vertical[slot];
    draw_input_area_with_mode(frame, input, nick, theme, input_area, input_mode, search);
}

/// Draw the input area with a nice border and prompt.
fn draw_input_area_with_mode(
    frame: &mut Frame,
    input: &InputState,
    nick: &str,
    theme: &Theme,
    area: Rect,
    input_mode: InputMode,
    search: Option<&SearchRenderState>,
) {
    // Input background (slightly lighter)
    let input_bg = Color::Rgb(30, 32, 42);

    // Create a block for the input area
    let block = Block::default()
        .borders(Borders::TOP)
        .border_style(Style::default().fg(theme.border))
        .border_set(symbols::border::PLAIN)
        .style(Style::default().bg(input_bg));

    // Calculate inner area
    let inner = block.inner(area);
    frame.render_widget(block, area);

    // Fill inner background
    for y in inner.y..inner.y + inner.height {
        for x in inner.x..inner.x + inner.width {
            frame.buffer_mut()[(x, y)].set_bg(input_bg);
        }
    }

    // Mode indicator for vim-style navigation
    let (mode_indicator, mode_style) = match input_mode {
        InputMode::Insert => ("", Style::default()),
        InputMode::Normal => (
            "[N] ",
            Style::default()
                .fg(Color::Rgb(255, 200, 100))
                .add_modifier(Modifier::BOLD),
        ),
        InputMode::Search => (
            "[/] ",
            Style::default()
                .fg(Color::Rgb(100, 200, 255))
                .add_modifier(Modifier::BOLD),
        ),
    };

    // Create prompt with nick (or search indicator)
    let (prompt_text, prompt_style, display_text) = if let Some(search_state) = search {
        if search_state.active {
            let search_info = if search_state.total_matches > 0 {
                format!(
                    " [{}/{}]",
                    search_state.current_match + 1,
                    search_state.total_matches
                )
            } else if !search_state.query.is_empty() {
                " [no match]".to_string()
            } else {
                String::new()
            };
            (
                "/".to_string(),
                Style::default()
                    .fg(Color::Rgb(100, 200, 255))
                    .add_modifier(Modifier::BOLD),
                format!("{}{}", search_state.query, search_info),
            )
        } else {
            (
                format!("[{}] ", nick),
                Style::default()
                    .fg(theme.accent)
                    .add_modifier(Modifier::BOLD),
                input.text.clone(),
            )
        }
    } else {
        (
            format!("[{}] ", nick),
            Style::default()
                .fg(theme.accent)
                .add_modifier(Modifier::BOLD),
            input.text.clone(),
        )
    };

    // Input text style
    let text_style = Style::default().fg(theme.fg);

    // Build the line
    let mut spans = vec![];
    if !mode_indicator.is_empty() {
        spans.push(Span::styled(mode_indicator, mode_style));
    }
    spans.push(Span::styled(&prompt_text, prompt_style));
    spans.push(Span::styled(&display_text, text_style));

    let line = Line::from(spans);

    // Render on the first line of inner area
    if inner.height > 0 {
        let text_y = inner.y;
        frame
            .buffer_mut()
            .set_line(inner.x + 1, text_y, &line, inner.width.saturating_sub(2));

        // Calculate cursor position
        let prompt_width = prompt_text.chars().count() as u16;
        let cursor_char_pos = input.text[..input.cursor].chars().count() as u16;
        let cursor_x = inner.x + 1 + prompt_width + cursor_char_pos;
        let cursor_y = text_y;

        // Make sure cursor is within bounds
        if cursor_x < inner.x + inner.width {
            // Draw cursor character with inverted colors
            let cursor_char = input.text[input.cursor..].chars().next().unwrap_or(' ');
            let cursor_style = Style::default()
                .fg(input_bg)
                .bg(theme.accent)
                .add_modifier(Modifier::BOLD);

            frame.buffer_mut()[(cursor_x, cursor_y)]
                .set_char(cursor_char)
                .set_style(cursor_style);

            // Also set terminal cursor for blinking
            frame.set_cursor_position((cursor_x, cursor_y));
        }
    }

    // Hint text on the right side (if space allows)
    if inner.width > 50 && inner.height > 0 {
        let hint = "Tab: complete | Ctrl+C: quit";
        let hint_style = Style::default().fg(theme.muted);
        let hint_x = inner.x + inner.width - hint.len() as u16 - 1;
        if hint_x > inner.x + prompt_text.len() as u16 + input.text.len() as u16 + 5 {
            let hint_span = Span::styled(hint, hint_style);
            frame
                .buffer_mut()
                .set_span(hint_x, inner.y, &hint_span, hint.len() as u16);
        }
    }
}
