//! Modern minimal layout (inspired by tiny/weechat).
//!
//! Layout:
//! ┌─────────────────────────────────────────────┐
//! │ #channel │ topic... │ 42 users │ ● connected│  <- 1 line status
//! ├─────────────────────────────────────────────┤
//! │ 14:32 alice │ hello everyone                │  <- messages (aligned)
//! │             │ how's it going?               │
//! │ 14:33   bob │ doing great!                  │
//! │             │                               │
//! ├─────────────────────────────────────────────┤
//! │ 1:#chan 2:Server 3:bob*                     │  <- tab bar
//! │ [nick] type here...                         │  <- input
//! └─────────────────────────────────────────────┘

use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
};

use crate::handler::input::InputMode;
use crate::state::BufferList;
use crate::ui::input::InputState;
use crate::ui::userlist::ChannelUser;

/// Colors for the modern theme (muted, minimal).
pub struct ModernColors {
    pub bg: Color,
    pub fg: Color,
    pub muted: Color,
    pub accent: Color,
    pub nick: Color,
    pub timestamp: Color,
    pub join: Color,
    pub part: Color,
    pub highlight: Color,
    pub error: Color,
}

impl Default for ModernColors {
    fn default() -> Self {
        Self {
            bg: Color::Rgb(22, 22, 28),           // Very dark blue-gray
            fg: Color::Rgb(200, 200, 210),        // Soft white
            muted: Color::Rgb(90, 95, 110),       // Gray
            accent: Color::Rgb(130, 170, 255),    // Soft blue
            nick: Color::Rgb(180, 180, 190),      // Light gray for nicks
            timestamp: Color::Rgb(70, 75, 90),    // Dark gray
            join: Color::Rgb(100, 160, 100),      // Muted green
            part: Color::Rgb(160, 100, 100),      // Muted red
            highlight: Color::Rgb(255, 180, 100), // Orange
            error: Color::Rgb(255, 100, 100),     // Red
        }
    }
}

/// Search state for rendering.
pub struct SearchRenderState {
    pub active: bool,
    pub query: String,
    pub current_match: usize,
    pub total_matches: usize,
}

/// User filter state for rendering.
pub struct UserFilterRenderState {
    pub active: bool,
    pub filter: String,
}

/// Command palette state for rendering.
pub struct CommandPaletteRenderState {
    pub filter: String,
    #[allow(dead_code)]
    pub selected: usize,
    pub items: Vec<(String, bool)>, // (label, is_selected)
    pub info_content: Vec<String>,
}

/// Sidebar width when shown.
const SIDEBAR_WIDTH: u16 = 24;

/// Draw the modern minimal layout.
pub fn draw_modern_layout(
    frame: &mut Frame,
    buffers: &BufferList,
    input: &InputState,
    nick: &str,
    connected: bool,
    channel_users: &[ChannelUser],
    input_mode: InputMode,
    search: Option<&SearchRenderState>,
    hide_joinpart: bool,
    show_sidebar: bool,
    user_filter: Option<&UserFilterRenderState>,
    command_palette: Option<&CommandPaletteRenderState>,
) {
    let colors = ModernColors::default();
    let area = frame.area();

    // Fill background
    for y in area.y..area.y + area.height {
        for x in area.x..area.x + area.width {
            frame.buffer_mut()[(x, y)].set_bg(colors.bg);
        }
    }

    // Horizontal split for sidebar (if shown and it's a channel)
    let active = buffers.active();
    let show_sidebar = show_sidebar && active.is_channel() && area.width > 60;

    let (main_area, sidebar_area) = if show_sidebar {
        let h_chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Min(40), Constraint::Length(SIDEBAR_WIDTH)])
            .split(area);
        (h_chunks[0], Some(h_chunks[1]))
    } else {
        (area, None)
    };

    // Layout with breathing room:
    // - Top padding (1)
    // - Status line (1)
    // - Separator/padding (1)
    // - Messages (flex)
    // - Separator (1)
    // - Tab bar (1)
    // - Spacer (1)
    // - Input line (1)
    // - Bottom padding (1)
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1), // Top margin
            Constraint::Length(1), // Status line
            Constraint::Length(1), // Separator below status
            Constraint::Min(5),    // Messages
            Constraint::Length(1), // Separator above tabs
            Constraint::Length(1), // Tab bar
            Constraint::Length(1), // Spacer
            Constraint::Length(1), // Input line
            Constraint::Length(1), // Bottom margin
        ])
        .split(main_area);

    // === Status Line (with horizontal padding) ===
    draw_status_line(
        frame,
        chunks[1],
        buffers,
        nick,
        connected,
        channel_users.len(),
        &colors,
        show_sidebar,
    );

    // === Subtle separator line ===
    draw_separator(frame, chunks[2], &colors);

    // === Messages ===
    draw_messages_modern(frame, chunks[3], buffers, &colors, hide_joinpart);

    // === Separator above tabs ===
    draw_separator(frame, chunks[4], &colors);

    // === Tab Bar ===
    draw_tab_bar(frame, chunks[5], buffers, &colors);

    // === Input Line ===
    draw_input_line(frame, chunks[7], input, nick, input_mode, search, &colors);

    // === Sidebar (if shown) ===
    if let Some(sb_area) = sidebar_area {
        draw_sidebar(frame, sb_area, buffers, channel_users, &colors, user_filter);
    }

    // === Command Palette (modal overlay) ===
    if let Some(palette) = command_palette {
        draw_command_palette(frame, area, palette, &colors);
    }
}

/// Draw the command palette modal.
fn draw_command_palette(
    frame: &mut Frame,
    area: Rect,
    palette: &CommandPaletteRenderState,
    colors: &ModernColors,
) {
    // Calculate palette dimensions (centered, 50% width, up to 60% height)
    let width = (area.width * 50 / 100).min(60).max(40);
    let max_height = (area.height * 60 / 100).max(10);

    // If we have info content, show that instead of the action list
    let content_lines = if !palette.info_content.is_empty() {
        palette.info_content.len() + 4 // +4 for padding and header
    } else {
        palette.items.len() + 4
    };
    let height = (content_lines as u16).min(max_height);

    let x = (area.width - width) / 2;
    let y = (area.height - height) / 3; // Slightly above center

    let palette_area = Rect::new(x, y, width, height);

    let buf = frame.buffer_mut();

    // Draw background with border
    let bg = Color::Rgb(30, 32, 42);
    let border_color = Color::Rgb(60, 65, 80);

    for py in palette_area.y..palette_area.y + palette_area.height {
        for px in palette_area.x..palette_area.x + palette_area.width {
            buf[(px, py)].set_bg(bg);
            buf[(px, py)].set_char(' ');
        }
    }

    // Top border
    for px in palette_area.x..palette_area.x + palette_area.width {
        buf[(px, palette_area.y)].set_fg(border_color);
        buf[(px, palette_area.y)].set_char('─');
    }
    // Bottom border
    for px in palette_area.x..palette_area.x + palette_area.width {
        buf[(px, palette_area.y + palette_area.height - 1)].set_fg(border_color);
        buf[(px, palette_area.y + palette_area.height - 1)].set_char('─');
    }
    // Side borders
    for py in palette_area.y..palette_area.y + palette_area.height {
        buf[(palette_area.x, py)].set_fg(border_color);
        buf[(palette_area.x, py)].set_char('│');
        buf[(palette_area.x + palette_area.width - 1, py)].set_fg(border_color);
        buf[(palette_area.x + palette_area.width - 1, py)].set_char('│');
    }
    // Corners
    buf[(palette_area.x, palette_area.y)].set_char('╭');
    buf[(palette_area.x + palette_area.width - 1, palette_area.y)].set_char('╮');
    buf[(palette_area.x, palette_area.y + palette_area.height - 1)].set_char('╰');
    buf[(
        palette_area.x + palette_area.width - 1,
        palette_area.y + palette_area.height - 1,
    )]
        .set_char('╯');

    let content_x = palette_area.x + 2;
    let content_width = palette_area.width - 4;
    let mut cy = palette_area.y + 1;

    // Search input
    let search_line = Line::from(vec![
        Span::styled("> ", Style::default().fg(colors.accent)),
        Span::styled(&palette.filter, Style::default().fg(Color::White)),
        Span::styled(
            "_",
            Style::default()
                .fg(colors.accent)
                .add_modifier(Modifier::SLOW_BLINK),
        ),
    ]);
    buf.set_line(content_x, cy, &search_line, content_width);
    cy += 1;

    // Separator
    for px in content_x..content_x + content_width {
        buf[(px, cy)].set_fg(Color::Rgb(50, 55, 65));
        buf[(px, cy)].set_char('─');
    }
    cy += 1;

    // Content: either info or items
    if !palette.info_content.is_empty() {
        // Show info content
        for line in &palette.info_content {
            if cy >= palette_area.y + palette_area.height - 1 {
                break;
            }
            let truncated = if line.len() > content_width as usize {
                format!("{}…", &line[..content_width as usize - 1])
            } else {
                line.clone()
            };
            buf.set_span(
                content_x,
                cy,
                &Span::styled(truncated, Style::default().fg(colors.fg)),
                content_width,
            );
            cy += 1;
        }

        // Hint at bottom
        if cy < palette_area.y + palette_area.height - 1 {
            cy = palette_area.y + palette_area.height - 2;
            buf.set_span(
                content_x,
                cy,
                &Span::styled(
                    "Backspace to go back · Esc to close",
                    Style::default().fg(colors.muted),
                ),
                content_width,
            );
        }
    } else {
        // Show action items
        for (label, is_selected) in palette.items.iter() {
            if cy >= palette_area.y + palette_area.height - 1 {
                break;
            }

            let style = if *is_selected {
                Style::default().fg(Color::White).bg(Color::Rgb(50, 55, 70))
            } else {
                Style::default().fg(colors.fg)
            };

            let prefix = if *is_selected { "▸ " } else { "  " };
            let truncated = if label.len() > (content_width as usize - 2) {
                format!("{}…", &label[..content_width as usize - 3])
            } else {
                label.clone()
            };

            // Fill background for selected item
            if *is_selected {
                for px in content_x..content_x + content_width {
                    buf[(px, cy)].set_bg(Color::Rgb(50, 55, 70));
                }
            }

            let line = Line::from(vec![
                Span::styled(prefix, style),
                Span::styled(truncated, style),
            ]);
            buf.set_line(content_x, cy, &line, content_width);
            cy += 1;
        }

        // Show "no matches" if empty
        if palette.items.is_empty() {
            buf.set_span(
                content_x,
                cy,
                &Span::styled("No matching commands", Style::default().fg(colors.muted)),
                content_width,
            );
        }
    }
}

/// Draw a subtle separator line.
fn draw_separator(frame: &mut Frame, area: Rect, colors: &ModernColors) {
    let buf = frame.buffer_mut();
    let sep_color = Color::Rgb(40, 42, 50);

    for x in area.x..area.x + area.width {
        buf[(x, area.y)].set_bg(colors.bg);
        buf[(x, area.y)].set_fg(sep_color);
        buf[(x, area.y)].set_char('─');
    }
}

/// Draw the tab bar with clean, uniform styling.
fn draw_tab_bar(frame: &mut Frame, area: Rect, buffers: &BufferList, colors: &ModernColors) {
    let buf = frame.buffer_mut();

    // Uniform background
    for x in area.x..area.x + area.width {
        buf[(x, area.y)].set_bg(colors.bg);
        buf[(x, area.y)].set_char(' ');
    }

    let mut spans = Vec::new();
    let active_idx = buffers.active_index();

    // Left padding
    spans.push(Span::raw("  "));

    for (i, buffer) in buffers.all().iter().enumerate() {
        let is_active = i == active_idx;
        let has_highlight = buffer.has_highlight;
        let has_unread = buffer.unread_count > 0;

        // Number prefix
        let num_style = if is_active {
            Style::default().fg(colors.accent)
        } else {
            Style::default().fg(Color::Rgb(80, 85, 100))
        };
        spans.push(Span::styled(format!("{}:", i + 1), num_style));

        // Buffer name
        let name = truncate_name(&buffer.name, 14);
        let name_style = if is_active {
            Style::default()
                .fg(Color::White)
                .add_modifier(Modifier::BOLD)
        } else if has_highlight {
            Style::default()
                .fg(Color::Rgb(255, 120, 120))
                .add_modifier(Modifier::BOLD)
        } else if has_unread {
            Style::default().fg(colors.accent)
        } else {
            Style::default().fg(Color::Rgb(100, 105, 120))
        };
        spans.push(Span::styled(name, name_style));

        // Activity indicator (only for inactive tabs)
        if !is_active {
            if has_highlight {
                spans.push(Span::styled(
                    "*",
                    Style::default().fg(Color::Rgb(255, 120, 120)),
                ));
            } else if has_unread {
                spans.push(Span::styled("+", Style::default().fg(colors.accent)));
            }
        }

        // Spacer between tabs
        spans.push(Span::raw("   "));
    }

    let line = Line::from(spans);
    buf.set_line(area.x, area.y, &line, area.width);
}

/// Truncate a name with ellipsis.
fn truncate_name(name: &str, max_len: usize) -> String {
    if name.len() > max_len {
        format!("{}…", &name[..max_len - 1])
    } else {
        name.to_string()
    }
}

/// Draw the minimal status line with padding.
fn draw_status_line(
    frame: &mut Frame,
    area: Rect,
    buffers: &BufferList,
    nick: &str,
    connected: bool,
    user_count: usize,
    colors: &ModernColors,
    sidebar_visible: bool,
) {
    let buf = frame.buffer_mut();
    let active = buffers.active();

    // No special background - use main bg
    for x in area.x..area.x + area.width {
        buf[(x, area.y)].set_bg(colors.bg);
        buf[(x, area.y)].set_char(' ');
    }

    let mut spans = Vec::new();

    // Left padding
    spans.push(Span::raw("  "));

    // Connection indicator
    let conn_indicator = if connected { "●" } else { "○" };
    let conn_color = if connected {
        Color::Rgb(100, 200, 120)
    } else {
        Color::Rgb(200, 100, 100)
    };
    spans.push(Span::styled(
        conn_indicator,
        Style::default().fg(conn_color),
    ));
    spans.push(Span::raw(" "));

    // Nick
    spans.push(Span::styled(nick, Style::default().fg(colors.accent)));
    spans.push(Span::styled(
        " │ ",
        Style::default().fg(Color::Rgb(50, 55, 65)),
    ));

    // Buffer name
    spans.push(Span::styled(
        &active.name,
        Style::default()
            .fg(Color::White)
            .add_modifier(Modifier::BOLD),
    ));

    // User count for channels (only when sidebar is hidden)
    if active.is_channel() && user_count > 0 && !sidebar_visible {
        spans.push(Span::styled(
            format!(" ({})", user_count),
            Style::default().fg(Color::Rgb(100, 105, 120)),
        ));
    }

    let line = Line::from(spans);
    buf.set_line(area.x, area.y, &line, area.width);
}

/// Draw the sidebar with topic and user list.
fn draw_sidebar(
    frame: &mut Frame,
    area: Rect,
    buffers: &BufferList,
    users: &[ChannelUser],
    colors: &ModernColors,
    user_filter: Option<&UserFilterRenderState>,
) {
    use crate::style::nick_color;

    let buf = frame.buffer_mut();
    let active = buffers.active();

    // Sidebar background (slightly lighter)
    let sidebar_bg = Color::Rgb(26, 28, 36);
    for y in area.y..area.y + area.height {
        for x in area.x..area.x + area.width {
            buf[(x, y)].set_bg(sidebar_bg);
            buf[(x, y)].set_char(' ');
        }
    }

    // Draw left border
    let border_color = Color::Rgb(40, 42, 50);
    for y in area.y..area.y + area.height {
        buf[(area.x, y)].set_fg(border_color);
        buf[(area.x, y)].set_char('│');
    }

    let content_x = area.x + 2;
    let content_width = area.width.saturating_sub(3) as usize;
    let mut y = area.y + 1;

    // === Topic Section ===
    if let Some(topic) = &active.topic {
        if !topic.is_empty() {
            // Topic header
            buf.set_span(
                content_x,
                y,
                &Span::styled(
                    "TOPIC",
                    Style::default()
                        .fg(colors.muted)
                        .add_modifier(Modifier::BOLD),
                ),
                content_width as u16,
            );
            y += 1;

            // Allow topic to use up to 1/3 of sidebar height
            let max_topic_lines = ((area.height as usize) / 3).max(3);
            let topic_lines = wrap_text(topic, content_width);
            let truncated = topic_lines.len() > max_topic_lines;

            for line in topic_lines.iter().take(max_topic_lines) {
                if y >= area.y + area.height - 2 {
                    break;
                }
                buf.set_span(
                    content_x,
                    y,
                    &Span::styled(line, Style::default().fg(Color::Rgb(150, 155, 170))),
                    content_width as u16,
                );
                y += 1;
            }
            if truncated {
                buf.set_span(
                    content_x,
                    y,
                    &Span::styled("... /topic for full", Style::default().fg(colors.muted)),
                    content_width as u16,
                );
                y += 1;
            }

            y += 1; // Spacing after topic
        }
    }

    // Get filter state
    let filter_active = user_filter.map(|f| f.active).unwrap_or(false);
    let filter_text = user_filter.map(|f| f.filter.as_str()).unwrap_or("");

    // === Filter Input (if active) ===
    if filter_active {
        let filter_line = Line::from(vec![
            Span::styled("/ ", Style::default().fg(colors.accent)),
            Span::styled(filter_text, Style::default().fg(Color::White)),
            Span::styled(
                "_",
                Style::default()
                    .fg(colors.accent)
                    .add_modifier(Modifier::SLOW_BLINK),
            ),
        ]);
        buf.set_line(content_x, y, &filter_line, content_width as u16);
        y += 1;
    }

    // Sort and filter users
    let mut sorted_users: Vec<_> = users.to_vec();
    sorted_users.sort_by(|a, b| {
        a.status
            .cmp(&b.status)
            .then_with(|| a.nick.to_lowercase().cmp(&b.nick.to_lowercase()))
    });

    // Apply filter if there's filter text
    let filtered_users: Vec<_> = if !filter_text.is_empty() {
        let filter_lower = filter_text.to_lowercase();
        sorted_users
            .into_iter()
            .filter(|u| u.nick.to_lowercase().contains(&filter_lower))
            .collect()
    } else {
        sorted_users
    };

    // === Users Section ===
    if y < area.y + area.height - 1 {
        let user_header = if !filter_text.is_empty() {
            format!("USERS ({}/{})", filtered_users.len(), users.len())
        } else {
            format!("USERS ({})", users.len())
        };
        buf.set_span(
            content_x,
            y,
            &Span::styled(
                user_header,
                Style::default()
                    .fg(colors.muted)
                    .add_modifier(Modifier::BOLD),
            ),
            content_width as u16,
        );
        y += 1;
    }

    // Draw users
    let mut displayed = 0;
    for user in filtered_users.iter() {
        if y >= area.y + area.height - 1 {
            // Show "and N more" if we can't fit all users
            let remaining = filtered_users.len() - displayed;
            if remaining > 0 {
                buf.set_span(
                    content_x,
                    y,
                    &Span::styled(
                        format!("  +{} more", remaining),
                        Style::default().fg(colors.muted),
                    ),
                    content_width as u16,
                );
            }
            break;
        }

        let prefix = user.status.symbol();
        let prefix_color = user.status.color();
        let nick_col = if user.away {
            Color::Rgb(80, 85, 100)
        } else {
            nick_color(&user.nick)
        };

        let nick_display = if user.nick.len() > content_width - 2 {
            format!("{}…", &user.nick[..content_width - 3])
        } else {
            user.nick.clone()
        };

        let line = Line::from(vec![
            Span::styled(prefix, Style::default().fg(prefix_color)),
            Span::styled(" ", Style::default()),
            Span::styled(nick_display, Style::default().fg(nick_col)),
        ]);
        buf.set_line(content_x, y, &line, content_width as u16);
        y += 1;
        displayed += 1;
    }

    // Show hint if no filter and not active
    if !filter_active && filter_text.is_empty() && y < area.y + area.height - 1 {
        y = area.y + area.height - 1;
        buf.set_span(
            content_x,
            y,
            &Span::styled(
                "Alt+F to filter",
                Style::default().fg(Color::Rgb(60, 65, 80)),
            ),
            content_width as u16,
        );
    }
}

/// Wrap text to fit within a given width, breaking on word boundaries.
fn wrap_text(text: &str, max_width: usize) -> Vec<String> {
    if max_width == 0 {
        return vec![text.to_string()];
    }

    let mut lines = Vec::new();
    let mut current_line = String::new();
    let mut current_width = 0;

    for word in text.split_whitespace() {
        let word_width = word.chars().count();

        if current_width == 0 {
            // First word on line
            if word_width > max_width {
                // Word too long, force break it
                let mut chars = word.chars().peekable();
                while chars.peek().is_some() {
                    let chunk: String = chars.by_ref().take(max_width).collect();
                    if !current_line.is_empty() {
                        lines.push(current_line);
                    }
                    current_line = chunk.clone();
                    current_width = chunk.chars().count();
                }
            } else {
                current_line = word.to_string();
                current_width = word_width;
            }
        } else if current_width + 1 + word_width <= max_width {
            // Word fits on current line
            current_line.push(' ');
            current_line.push_str(word);
            current_width += 1 + word_width;
        } else {
            // Word doesn't fit, start new line
            lines.push(current_line);
            if word_width > max_width {
                // Word too long, force break it
                let mut chars = word.chars().peekable();
                current_line = String::new();
                current_width = 0;
                while chars.peek().is_some() {
                    let chunk: String = chars.by_ref().take(max_width).collect();
                    if !current_line.is_empty() {
                        lines.push(current_line);
                    }
                    current_line = chunk.clone();
                    current_width = chunk.chars().count();
                }
            } else {
                current_line = word.to_string();
                current_width = word_width;
            }
        }
    }

    if !current_line.is_empty() {
        lines.push(current_line);
    }

    if lines.is_empty() {
        lines.push(String::new());
    }

    lines
}

fn draw_messages_modern(
    frame: &mut Frame,
    area: Rect,
    buffers: &BufferList,
    colors: &ModernColors,
    hide_joinpart: bool,
) {
    use crate::state::message::MessageKind;
    use chrono::{Duration, Local};

    let buf = frame.buffer_mut();
    let buffer = buffers.active();

    // Fill background
    for y in area.y..area.y + area.height {
        for x in area.x..area.x + area.width {
            buf[(x, y)].set_bg(colors.bg);
        }
    }

    // Layout constants
    const LEFT_MARGIN: u16 = 2;
    const NICK_WIDTH: usize = 12;
    const TIME_WIDTH: usize = 6; // "HH:MM "
    const PREFIX_WIDTH: usize = TIME_WIDTH + NICK_WIDTH + 3; // +3 for " │ "

    let msg_x = area.x + LEFT_MARGIN;
    let msg_width = area.width.saturating_sub(LEFT_MARGIN * 2) as usize;
    let text_width = msg_width.saturating_sub(PREFIX_WIDTH);

    // Build visual lines from messages (each message may wrap to multiple lines)
    #[derive(Clone)]
    enum VisualLine {
        MessageFirst {
            time_str: String,
            nick: String,
            text: String,
            nick_color: Color,
        },
        MessageContinuation {
            text: String,
        },
        GroupedContinuation {
            text: String,
        },
        Action {
            time_str: String,
            nick: String,
            text: String,
        },
        System {
            prefix: &'static str,
            text: String,
            color: Color,
        },
    }

    let mut visual_lines: Vec<VisualLine> = Vec::new();
    let messages: Vec<_> = buffer.messages().collect();

    let mut prev_nick: Option<String> = None;
    let mut prev_time: Option<chrono::DateTime<chrono::Utc>> = None;

    for (i, msg) in messages.iter().enumerate() {
        // Skip join/part messages if hidden
        if hide_joinpart {
            match &msg.kind {
                MessageKind::Join { .. }
                | MessageKind::Part { .. }
                | MessageKind::Quit { .. }
                | MessageKind::AggregatedJoin { .. }
                | MessageKind::AggregatedPart { .. } => continue,
                _ => {}
            }
        }

        // Check if this is a continuation (same nick within 5 min)
        let is_grouped = if i > 0 {
            if let (Some(pn), Some(pt)) = (&prev_nick, prev_time) {
                if let MessageKind::Privmsg { nick, .. } = &msg.kind {
                    nick.eq_ignore_ascii_case(pn) && (msg.time - pt) < Duration::minutes(5)
                } else {
                    false
                }
            } else {
                false
            }
        } else {
            false
        };

        match &msg.kind {
            MessageKind::Privmsg { nick, text } => {
                let wrapped = wrap_text(text, text_width);
                let nc = nick_color(nick);

                if is_grouped {
                    // All lines are continuations (no header)
                    for line_text in wrapped {
                        visual_lines.push(VisualLine::GroupedContinuation { text: line_text });
                    }
                } else {
                    // First line has header, rest are continuations
                    let time_str = msg.time.with_timezone(&Local).format("%H:%M").to_string();
                    for (j, line_text) in wrapped.into_iter().enumerate() {
                        if j == 0 {
                            visual_lines.push(VisualLine::MessageFirst {
                                time_str: time_str.clone(),
                                nick: nick.clone(),
                                text: line_text,
                                nick_color: nc,
                            });
                        } else {
                            visual_lines.push(VisualLine::MessageContinuation { text: line_text });
                        }
                    }
                    prev_nick = Some(nick.clone());
                    prev_time = Some(msg.time);
                }
            }

            MessageKind::Action { nick, text } => {
                let time_str = msg.time.with_timezone(&Local).format("%H:%M").to_string();
                visual_lines.push(VisualLine::Action {
                    time_str,
                    nick: nick.clone(),
                    text: text.clone(),
                });
                prev_nick = None;
            }

            MessageKind::Join { nick, .. } => {
                visual_lines.push(VisualLine::System {
                    prefix: "→ ",
                    text: format!("{} joined", nick),
                    color: colors.join,
                });
                prev_nick = None;
            }

            MessageKind::AggregatedJoin { nicks } => {
                let names = if nicks.len() <= 3 {
                    nicks.join(", ")
                } else {
                    format!("{} +{}", nicks[..3].join(", "), nicks.len() - 3)
                };
                visual_lines.push(VisualLine::System {
                    prefix: "→ ",
                    text: format!("{} joined", names),
                    color: colors.join,
                });
                prev_nick = None;
            }

            MessageKind::Part { nick, message } => {
                let m = message
                    .as_ref()
                    .map(|m| format!(" ({})", m))
                    .unwrap_or_default();
                visual_lines.push(VisualLine::System {
                    prefix: "← ",
                    text: format!("{} left{}", nick, m),
                    color: colors.part,
                });
                prev_nick = None;
            }

            MessageKind::Quit { nick, message } => {
                let m = message
                    .as_ref()
                    .map(|m| format!(" ({})", m))
                    .unwrap_or_default();
                visual_lines.push(VisualLine::System {
                    prefix: "← ",
                    text: format!("{} quit{}", nick, m),
                    color: colors.part,
                });
                prev_nick = None;
            }

            MessageKind::AggregatedPart { nicks, is_quit } => {
                let names = if nicks.len() <= 3 {
                    nicks.join(", ")
                } else {
                    format!("{} +{}", nicks[..3].join(", "), nicks.len() - 3)
                };
                let action = if *is_quit { "quit" } else { "left" };
                visual_lines.push(VisualLine::System {
                    prefix: "← ",
                    text: format!("{} {}", names, action),
                    color: colors.part,
                });
                prev_nick = None;
            }

            MessageKind::Notice { source, text } => {
                let src = source.as_deref().unwrap_or("-");
                visual_lines.push(VisualLine::System {
                    prefix: "",
                    text: format!("-{}- {}", src, text),
                    color: colors.highlight,
                });
                prev_nick = None;
            }

            MessageKind::Server { text } => {
                visual_lines.push(VisualLine::System {
                    prefix: "• ",
                    text: text.clone(),
                    color: colors.muted,
                });
                prev_nick = None;
            }

            MessageKind::Error { text } => {
                visual_lines.push(VisualLine::System {
                    prefix: "✕ ",
                    text: text.clone(),
                    color: colors.error,
                });
                prev_nick = None;
            }

            MessageKind::Topic { setter, topic } => {
                let setter_str = setter.as_deref().unwrap_or("someone");
                let topic_str = topic.as_deref().unwrap_or("(cleared)");
                let full_text = format!("{} set topic: {}", setter_str, topic_str);

                // Wrap topic text (account for "★ " prefix = 2 chars)
                let wrapped = wrap_text(&full_text, text_width.saturating_sub(2));
                for (i, line) in wrapped.into_iter().enumerate() {
                    visual_lines.push(VisualLine::System {
                        prefix: if i == 0 { "★ " } else { "  " },
                        text: line,
                        color: colors.highlight,
                    });
                }
                prev_nick = None;
            }

            MessageKind::Mode { setter, modes } => {
                visual_lines.push(VisualLine::System {
                    prefix: "⚙ ",
                    text: format!("{} sets mode {}", setter, modes),
                    color: colors.muted,
                });
                prev_nick = None;
            }

            MessageKind::Nick { old_nick, new_nick } => {
                visual_lines.push(VisualLine::System {
                    prefix: "• ",
                    text: format!("{} → {}", old_nick, new_nick),
                    color: colors.muted,
                });
                prev_nick = None;
            }

            MessageKind::Kick {
                nick,
                kicker,
                reason,
            } => {
                let r = reason
                    .as_ref()
                    .map(|r| format!(" ({})", r))
                    .unwrap_or_default();
                visual_lines.push(VisualLine::System {
                    prefix: "✕ ",
                    text: format!("{} kicked by {}{}", nick, kicker, r),
                    color: colors.error,
                });
                prev_nick = None;
            }

            MessageKind::HistorySeparator => {
                visual_lines.push(VisualLine::System {
                    prefix: "",
                    text: "─── history ───".to_string(),
                    color: colors.muted,
                });
                prev_nick = None;
            }
        }
    }

    // Calculate scrolling based on visual lines
    let visible_height = area.height as usize;
    let total_visual = visual_lines.len();
    let scroll_offset = buffer
        .scroll_offset
        .min(total_visual.saturating_sub(visible_height));

    let start = total_visual.saturating_sub(visible_height + scroll_offset);
    let end = total_visual.saturating_sub(scroll_offset);

    // Render visual lines
    let mut y = area.y;
    for vline in visual_lines[start..end].iter() {
        if y >= area.y + area.height {
            break;
        }

        match vline {
            VisualLine::MessageFirst {
                time_str,
                nick,
                text,
                nick_color,
            } => {
                let nick_display = if nick.len() > NICK_WIDTH - 1 {
                    format!("{:.width$}", nick, width = NICK_WIDTH - 1)
                } else {
                    format!("{:>width$}", nick, width = NICK_WIDTH - 1)
                };

                let line = Line::from(vec![
                    Span::styled(
                        format!("{} ", time_str),
                        Style::default().fg(colors.timestamp),
                    ),
                    Span::styled(nick_display, Style::default().fg(*nick_color)),
                    Span::styled(" │ ", Style::default().fg(Color::Rgb(50, 55, 65))),
                    Span::styled(text.clone(), Style::default().fg(colors.fg)),
                ]);
                buf.set_line(msg_x, y, &line, msg_width as u16);
            }

            VisualLine::MessageContinuation { text } => {
                let indent = " ".repeat(PREFIX_WIDTH);
                let line = Line::from(vec![
                    Span::styled(indent, Style::default()),
                    Span::styled(text.clone(), Style::default().fg(colors.fg)),
                ]);
                buf.set_line(msg_x, y, &line, msg_width as u16);
            }

            VisualLine::GroupedContinuation { text } => {
                let indent = " ".repeat(PREFIX_WIDTH);
                let line = Line::from(vec![
                    Span::styled(indent, Style::default()),
                    Span::styled(text.clone(), Style::default().fg(colors.fg)),
                ]);
                buf.set_line(msg_x, y, &line, msg_width as u16);
            }

            VisualLine::Action {
                time_str,
                nick,
                text,
            } => {
                let line = Line::from(vec![
                    Span::styled(
                        format!("{} ", time_str),
                        Style::default().fg(colors.timestamp),
                    ),
                    Span::styled("        ", Style::default()),
                    Span::styled("* ", Style::default().fg(colors.accent)),
                    Span::styled(nick.clone(), Style::default().fg(nick_color(nick))),
                    Span::styled(format!(" {}", text), Style::default().fg(colors.accent)),
                ]);
                buf.set_line(msg_x, y, &line, msg_width as u16);
            }

            VisualLine::System {
                prefix,
                text,
                color,
            } => {
                let line = Line::from(vec![
                    Span::styled("      ", Style::default()),
                    Span::styled(*prefix, Style::default().fg(*color)),
                    Span::styled(text.clone(), Style::default().fg(*color)),
                ]);
                buf.set_line(msg_x, y, &line, msg_width as u16);
            }
        }

        y += 1;
    }

    // Scroll indicator if scrolled up
    if buffer.scroll_offset > 0 {
        let indicator = format!(" ↑{} ", buffer.scroll_offset);
        let ind_x = area.x + area.width - indicator.len() as u16 - 1;
        let ind_style = Style::default()
            .fg(Color::Rgb(200, 180, 100))
            .add_modifier(Modifier::BOLD);
        let ind_span = Span::styled(indicator, ind_style);
        buf.set_span(ind_x, area.y, &ind_span, 10);
    }

    // New messages indicator
    if buffer.new_messages_while_scrolled > 0 && !buffer.is_at_bottom() {
        let indicator = format!(" ↓ {} new ", buffer.new_messages_while_scrolled);
        let ind_x = area.x + (area.width - indicator.len() as u16) / 2;
        let ind_y = area.y + area.height - 1;
        let ind_style = Style::default()
            .fg(Color::White)
            .bg(Color::Rgb(80, 100, 140));
        for (i, c) in indicator.chars().enumerate() {
            buf[(ind_x + i as u16, ind_y)].set_char(c);
            buf[(ind_x + i as u16, ind_y)].set_style(ind_style);
        }
    }
}

/// Draw the input line with proper padding.
fn draw_input_line(
    frame: &mut Frame,
    area: Rect,
    input: &InputState,
    _nick: &str,
    input_mode: InputMode,
    search: Option<&SearchRenderState>,
    colors: &ModernColors,
) {
    let buf = frame.buffer_mut();

    // No special background - use main bg
    for x in area.x..area.x + area.width {
        buf[(x, area.y)].set_bg(colors.bg);
        buf[(x, area.y)].set_char(' ');
    }

    // Mode prefix
    let mode_prefix = match input_mode {
        InputMode::Normal => "[N] ",
        InputMode::Search => "/ ",
        InputMode::Insert => "",
    };
    let mode_style = match input_mode {
        InputMode::Normal => Style::default().fg(Color::Rgb(255, 200, 100)),
        InputMode::Search => Style::default().fg(colors.accent),
        InputMode::Insert => Style::default(),
    };

    // Build the prompt and display text
    let (prompt, display_text) = if let Some(s) = search {
        if s.active {
            let info = if s.total_matches > 0 {
                format!("  [{}/{}]", s.current_match + 1, s.total_matches)
            } else if !s.query.is_empty() {
                "  [no match]".to_string()
            } else {
                String::new()
            };
            (String::new(), format!("{}{}", s.query, info))
        } else {
            ("> ".to_string(), input.text.clone())
        }
    } else {
        ("> ".to_string(), input.text.clone())
    };

    let mut spans = vec![];

    // Left padding
    spans.push(Span::raw("  "));

    // Mode indicator (if not insert)
    if !mode_prefix.is_empty() {
        spans.push(Span::styled(mode_prefix, mode_style));
    }

    // Prompt
    spans.push(Span::styled(&prompt, Style::default().fg(colors.muted)));

    // Input text
    spans.push(Span::styled(&display_text, Style::default().fg(colors.fg)));

    let line = Line::from(spans);
    buf.set_line(area.x, area.y, &line, area.width);

    // Cursor position calculation
    let prefix_len = 2 + mode_prefix.len();
    let cursor_offset = prefix_len + prompt.len() + input.text[..input.cursor].chars().count();
    let cursor_x = area.x + cursor_offset as u16;

    if cursor_x < area.x + area.width {
        let cursor_char = input.text[input.cursor..].chars().next().unwrap_or(' ');
        let cursor_style = Style::default().fg(colors.bg).bg(colors.accent);
        buf[(cursor_x, area.y)].set_char(cursor_char);
        buf[(cursor_x, area.y)].set_style(cursor_style);
        frame.set_cursor_position((cursor_x, area.y));
    }
}

/// Generate nick color (muted palette).
fn nick_color(nick: &str) -> Color {
    // Muted nick colors
    const NICK_COLORS: [Color; 8] = [
        Color::Rgb(180, 140, 140), // Dusty rose
        Color::Rgb(140, 180, 140), // Sage
        Color::Rgb(140, 160, 180), // Steel blue
        Color::Rgb(180, 160, 140), // Tan
        Color::Rgb(160, 140, 180), // Lavender
        Color::Rgb(140, 180, 170), // Seafoam
        Color::Rgb(180, 170, 140), // Sand
        Color::Rgb(170, 140, 160), // Mauve
    ];

    let mut hash: u32 = 5381;
    for c in nick.bytes() {
        hash = hash.wrapping_mul(33).wrapping_add(c as u32);
    }

    NICK_COLORS[(hash as usize) % NICK_COLORS.len()]
}
