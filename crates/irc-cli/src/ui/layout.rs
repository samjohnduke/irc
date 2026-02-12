//! Main layout composition.

use ratatui::{
    layout::{Constraint, Direction, Layout},
    Frame,
};

use crate::state::BufferList;
use crate::style::Theme;
use crate::ui::input::{InputState, InputWidget};
use crate::ui::messages::MessagesWidget;
use crate::ui::sidebar::SidebarWidget;
use crate::ui::statusbar::StatusbarWidget;

/// Layout configuration.
pub struct LayoutConfig {
    /// Sidebar width (0 to hide).
    pub sidebar_width: u16,
    /// Show status bar.
    pub show_statusbar: bool,
}

impl Default for LayoutConfig {
    fn default() -> Self {
        Self {
            sidebar_width: 15,
            show_statusbar: true,
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
) {
    let area = frame.area();

    // Main vertical layout: statusbar, content, input
    let mut constraints = vec![];
    if config.show_statusbar {
        constraints.push(Constraint::Length(1));
    }
    constraints.push(Constraint::Min(3));
    constraints.push(Constraint::Length(1));

    let vertical = Layout::default()
        .direction(Direction::Vertical)
        .constraints(constraints)
        .split(area);

    let mut slot = 0;

    // Status bar
    if config.show_statusbar {
        let active = buffers.active();
        let channel = if active.is_server() {
            None
        } else {
            Some(active.name.as_str())
        };
        let topic = active.topic.as_deref();

        let statusbar = StatusbarWidget::new(nick, channel, topic, connected, theme);
        frame.render_widget(statusbar, vertical[slot]);
        slot += 1;
    }

    // Content area: sidebar + messages
    let content_area = vertical[slot];
    slot += 1;

    if config.sidebar_width > 0 {
        let horizontal = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Length(config.sidebar_width),
                Constraint::Min(20),
            ])
            .split(content_area);

        // Sidebar
        let sidebar = SidebarWidget::new(buffers, theme);
        frame.render_widget(sidebar, horizontal[0]);

        // Messages
        let messages = MessagesWidget::new(buffers.active(), theme);
        frame.render_widget(messages, horizontal[1]);
    } else {
        // No sidebar, just messages
        let messages = MessagesWidget::new(buffers.active(), theme);
        frame.render_widget(messages, content_area);
    }

    // Input line
    let input_area = vertical[slot];
    let prompt = "> ";
    let input_widget = InputWidget::new(input, prompt, theme);
    frame.render_widget(input_widget, input_area);

    // Set cursor position
    let cursor_x = input_area.x + prompt.len() as u16 + input.cursor_char_pos() as u16;
    frame.set_cursor_position((cursor_x, input_area.y));
}

