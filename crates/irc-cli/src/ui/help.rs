//! Help overlay widget.

use ratatui::{
    buffer::Buffer,
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Padding, Paragraph, Widget, Wrap},
};

/// Help overlay widget that displays keybindings and commands.
pub struct HelpWidget;

impl HelpWidget {
    pub fn new() -> Self {
        Self
    }

    fn keybindings() -> Vec<(&'static str, &'static str)> {
        vec![
            ("Navigation", ""),
            ("Ctrl+N / Ctrl+P", "Next/prev buffer"),
            ("Alt+↑↓ / Alt+←→", "Switch buffers"),
            ("PageUp/PageDown", "Scroll messages"),
            ("End", "Scroll to bottom"),
            ("", ""),
            ("Editing", ""),
            ("Ctrl+U", "Clear input line"),
            ("Ctrl+W", "Delete word"),
            ("Ctrl+A / Home", "Start of line"),
            ("Ctrl+E / End", "End of line"),
            ("Tab", "Tab completion"),
            ("↑ / ↓", "Input history"),
            ("", ""),
            ("Application", ""),
            ("F1 / ?", "Toggle this help"),
            ("Ctrl+C", "Quit"),
        ]
    }

    fn commands() -> Vec<(&'static str, &'static str)> {
        vec![
            ("Channel Commands", ""),
            ("/join #channel [key]", "Join a channel"),
            ("/part [message]", "Leave current channel"),
            ("/topic [text]", "View/set channel topic"),
            ("/names", "List channel members"),
            ("", ""),
            ("Messaging", ""),
            ("/msg <nick> <text>", "Send private message"),
            ("/me <action>", "Send action (/me waves)"),
            ("/notice <target> <text>", "Send notice"),
            ("", ""),
            ("User Commands", ""),
            ("/nick <newnick>", "Change nickname"),
            ("/away [message]", "Set/clear away status"),
            ("/whois <nick>", "Get user info"),
            ("", ""),
            ("Window Commands", ""),
            ("/close", "Close current buffer"),
            ("/clear", "Clear current buffer"),
            ("/quit [message]", "Disconnect and quit"),
        ]
    }
}

impl Widget for HelpWidget {
    fn render(self, area: Rect, buf: &mut Buffer) {
        // Calculate centered popup area (80% width, 80% height, max 80x30)
        let popup_width = (area.width * 80 / 100).min(80);
        let popup_height = (area.height * 80 / 100).min(30);
        let popup_x = area.x + (area.width.saturating_sub(popup_width)) / 2;
        let popup_y = area.y + (area.height.saturating_sub(popup_height)) / 2;

        let popup_area = Rect::new(popup_x, popup_y, popup_width, popup_height);

        // Clear the popup area
        Widget::render(Clear, popup_area, buf);

        // Draw background with border
        let block = Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Rgb(100, 150, 200)))
            .title(Span::styled(
                " Help ",
                Style::default()
                    .fg(Color::Rgb(150, 200, 255))
                    .add_modifier(Modifier::BOLD),
            ))
            .title_alignment(Alignment::Center)
            .style(Style::default().bg(Color::Rgb(25, 25, 35)))
            .padding(Padding::new(1, 1, 1, 1));

        let inner = block.inner(popup_area);
        Widget::render(block, popup_area, buf);

        // Split into two columns
        let columns = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
            .split(inner);

        // Left column: Keybindings
        let keybindings = Self::keybindings();
        let mut kb_lines: Vec<Line> = Vec::new();

        for (key, desc) in keybindings {
            if key.is_empty() && desc.is_empty() {
                kb_lines.push(Line::from(""));
            } else if desc.is_empty() {
                // Section header
                kb_lines.push(Line::from(Span::styled(
                    key,
                    Style::default()
                        .fg(Color::Rgb(100, 200, 255))
                        .add_modifier(Modifier::BOLD),
                )));
            } else {
                kb_lines.push(Line::from(vec![
                    Span::styled(
                        format!("{:<18}", key),
                        Style::default().fg(Color::Rgb(200, 180, 100)),
                    ),
                    Span::styled(desc, Style::default().fg(Color::Rgb(180, 180, 180))),
                ]));
            }
        }

        let kb_para = Paragraph::new(kb_lines).wrap(Wrap { trim: false });
        Widget::render(kb_para, columns[0], buf);

        // Right column: Commands
        let commands = Self::commands();
        let mut cmd_lines: Vec<Line> = Vec::new();

        for (cmd, desc) in commands {
            if cmd.is_empty() && desc.is_empty() {
                cmd_lines.push(Line::from(""));
            } else if desc.is_empty() {
                // Section header
                cmd_lines.push(Line::from(Span::styled(
                    cmd,
                    Style::default()
                        .fg(Color::Rgb(100, 200, 255))
                        .add_modifier(Modifier::BOLD),
                )));
            } else {
                cmd_lines.push(Line::from(vec![
                    Span::styled(
                        format!("{:<24}", cmd),
                        Style::default().fg(Color::Rgb(150, 220, 150)),
                    ),
                    Span::styled(desc, Style::default().fg(Color::Rgb(180, 180, 180))),
                ]));
            }
        }

        let cmd_para = Paragraph::new(cmd_lines).wrap(Wrap { trim: false });
        Widget::render(cmd_para, columns[1], buf);

        // Footer hint
        let footer_area = Rect::new(
            popup_area.x + 1,
            popup_area.y + popup_area.height - 2,
            popup_area.width - 2,
            1,
        );
        let footer = Paragraph::new(Line::from(Span::styled(
            "Press F1 or Esc to close",
            Style::default().fg(Color::Rgb(100, 100, 120)),
        )))
        .alignment(Alignment::Center);
        Widget::render(footer, footer_area, buf);
    }
}
