//! Main layout composition.

use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    symbols,
    text::{Line, Span},
    widgets::{Block, Borders},
    Frame,
};

use crate::state::BufferList;
use crate::style::Theme;
use crate::ui::input::InputState;
use crate::ui::messages::MessagesWidget;
use crate::ui::sidebar::SidebarWidget;
use crate::ui::statusbar::StatusbarWidget;
use crate::ui::userlist::{ChannelUser, EmptyUserListWidget, UserListWidget};

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
            .with_user_count(if active.is_channel() { Some(channel_users.len()) } else { None });
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
    draw_input_area(frame, input, nick, theme, input_area);
}

/// Draw the input area with a nice border and prompt.
fn draw_input_area(
    frame: &mut Frame,
    input: &InputState,
    nick: &str,
    theme: &Theme,
    area: Rect,
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

    // Create prompt with nick
    let prompt_text = format!("[{}] ", nick);
    let prompt_style = Style::default()
        .fg(theme.accent)
        .add_modifier(Modifier::BOLD);

    // Input text style
    let text_style = Style::default().fg(theme.fg);

    // Build the line
    let prompt_span = Span::styled(&prompt_text, prompt_style);
    let text_span = Span::styled(&input.text, text_style);

    let line = Line::from(vec![prompt_span.clone(), text_span]);

    // Render on the first line of inner area
    if inner.height > 0 {
        let text_y = inner.y;
        frame.buffer_mut().set_line(inner.x + 1, text_y, &line, inner.width.saturating_sub(2));

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
            frame.buffer_mut().set_span(hint_x, inner.y, &hint_span, hint.len() as u16);
        }
    }
}
