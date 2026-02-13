//! Splash screen widget for connecting state.

use ratatui::{
    buffer::Buffer,
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Widget, Wrap},
};

use crate::style::Theme;

/// ASCII art logo for the splash screen (block style).
const LOGO_LINES: &[&str] = &[
    "██╗██████╗  ██████╗",
    "██║██╔══██╗██╔════╝",
    "██║██████╔╝██║     ",
    "██║██╔══██╗██║     ",
    "██║██║  ██║╚██████╗",
    "╚═╝╚═╝  ╚═╝ ╚═════╝",
];

/// Connection state for splash display.
#[derive(Debug, Clone)]
pub enum ConnectionPhase {
    /// Initial state, not started.
    Starting,
    /// TCP/TLS connection in progress.
    Connecting,
    /// Negotiating capabilities.
    Capabilities,
    /// SASL authentication.
    Authenticating,
    /// Sending NICK/USER.
    Registering,
    /// Connected successfully.
    Connected,
    /// Connection failed.
    Failed(String),
}

impl ConnectionPhase {
    /// Get a display string for the current phase.
    pub fn display(&self) -> &str {
        match self {
            ConnectionPhase::Starting => "Initializing...",
            ConnectionPhase::Connecting => "Connecting to server...",
            ConnectionPhase::Capabilities => "Negotiating capabilities...",
            ConnectionPhase::Authenticating => "Authenticating...",
            ConnectionPhase::Registering => "Registering...",
            ConnectionPhase::Connected => "Connected!",
            ConnectionPhase::Failed(_) => "Connection failed",
        }
    }
}

/// A log entry for the connection log.
#[derive(Debug, Clone)]
pub struct LogEntry {
    pub text: String,
    pub is_error: bool,
    pub is_success: bool,
}

impl LogEntry {
    pub fn info(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            is_error: false,
            is_success: false,
        }
    }

    pub fn success(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            is_error: false,
            is_success: true,
        }
    }

    pub fn error(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            is_error: true,
            is_success: false,
        }
    }
}

/// Splash screen widget.
pub struct SplashWidget<'a> {
    server: &'a str,
    port: u16,
    phase: &'a ConnectionPhase,
    log: &'a [LogEntry],
    frame_count: usize,
    #[allow(dead_code)]
    theme: &'a Theme,
}

impl<'a> SplashWidget<'a> {
    pub fn new(
        server: &'a str,
        port: u16,
        phase: &'a ConnectionPhase,
        log: &'a [LogEntry],
        frame_count: usize,
        theme: &'a Theme,
    ) -> Self {
        Self {
            server,
            port,
            phase,
            log,
            frame_count,
            theme,
        }
    }

    fn render_spinner(&self) -> &'static str {
        const SPINNERS: [&str; 8] = ["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧"];
        SPINNERS[self.frame_count % SPINNERS.len()]
    }

    fn render_dots(&self) -> String {
        let dots = (self.frame_count / 5) % 4;
        ".".repeat(dots) + &" ".repeat(3 - dots)
    }
}

impl Widget for SplashWidget<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        // Clear the area with a dark background
        buf.set_style(area, Style::default().bg(Color::Reset));

        // Calculate content height for vertical centering
        let logo_height = LOGO_LINES.len() as u16;
        let content_height = logo_height + 6; // logo + subtitle + spacing + server + status
        let log_height = 8u16; // Fixed log area height
        let total_height = content_height + log_height + 2; // +2 for borders/hint

        // Calculate vertical padding for centering
        let v_padding = area.height.saturating_sub(total_height) / 2;

        // Split into sections with centering
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(v_padding),       // Top padding
                Constraint::Length(logo_height + 3), // Logo + subtitle
                Constraint::Length(4),               // Server info + status
                Constraint::Length(log_height),      // Log area
                Constraint::Length(1),               // Hint at bottom
                Constraint::Min(0),                  // Bottom padding (absorbs rest)
            ])
            .split(area);

        // Gradient colors for the logo (top to bottom: bright to dim)
        let gradient_colors = [
            Color::Rgb(0, 255, 255),   // Bright cyan
            Color::Rgb(0, 220, 255),
            Color::Rgb(0, 180, 220),
            Color::Rgb(0, 150, 200),
            Color::Rgb(0, 120, 180),
            Color::Rgb(0, 100, 160),   // Darker cyan
        ];

        // Render logo with gradient effect
        let mut logo_lines: Vec<Line> = LOGO_LINES
            .iter()
            .enumerate()
            .map(|(i, line)| {
                let color = gradient_colors[i % gradient_colors.len()];
                Line::from(Span::styled(
                    *line,
                    Style::default().fg(color).add_modifier(Modifier::BOLD),
                ))
            })
            .collect();

        // Add empty line and subtitle
        logo_lines.push(Line::from(""));
        logo_lines.push(Line::from(Span::styled(
            "━━━ Terminal IRC Client ━━━",
            Style::default().fg(Color::Rgb(80, 80, 100)),
        )));

        let logo = Paragraph::new(logo_lines)
            .alignment(Alignment::Center);
        Widget::render(logo, chunks[1], buf);

        // Render server info and status
        let server_line = Line::from(vec![
            Span::styled("● ", Style::default().fg(Color::Rgb(100, 100, 120))),
            Span::styled("Server: ", Style::default().fg(Color::Rgb(120, 120, 140))),
            Span::styled(
                format!("{}:{}", self.server, self.port),
                Style::default().fg(Color::White).add_modifier(Modifier::BOLD),
            ),
        ]);

        let (status_icon, status_style) = match self.phase {
            ConnectionPhase::Connected => ("✓", Style::default().fg(Color::Rgb(100, 255, 100))),
            ConnectionPhase::Failed(_) => ("✗", Style::default().fg(Color::Rgb(255, 100, 100))),
            _ => (self.render_spinner(), Style::default().fg(Color::Rgb(255, 200, 100))),
        };

        let status_text = match self.phase {
            ConnectionPhase::Failed(err) => format!("{} {}", status_icon, err),
            ConnectionPhase::Connected => format!("{} {}", status_icon, self.phase.display()),
            _ => format!(
                "{} {}{}",
                status_icon,
                self.phase.display(),
                self.render_dots()
            ),
        };

        let status_line = Line::from(Span::styled(status_text, status_style));

        let info = Paragraph::new(vec![
            Line::from(""),
            server_line,
            status_line,
        ])
        .alignment(Alignment::Center);
        Widget::render(info, chunks[2], buf);

        // Render connection log in a box
        let log_area = chunks[3];

        // Calculate visible entries
        let visible_height = log_area.height.saturating_sub(2) as usize;
        let start = self.log.len().saturating_sub(visible_height);

        let log_lines: Vec<Line> = self.log[start..]
            .iter()
            .map(|entry| {
                let (prefix, style) = if entry.is_error {
                    ("  ✗ ", Style::default().fg(Color::Rgb(255, 100, 100)))
                } else if entry.is_success {
                    ("  ✓ ", Style::default().fg(Color::Rgb(100, 255, 100)))
                } else {
                    ("  › ", Style::default().fg(Color::Rgb(100, 100, 120)))
                };

                Line::from(vec![
                    Span::styled(prefix, style),
                    Span::styled(entry.text.clone(), style),
                ])
            })
            .collect();

        // Center the log box horizontally
        let log_width = 60u16.min(area.width.saturating_sub(4));
        let log_x = area.x + (area.width.saturating_sub(log_width)) / 2;
        let centered_log_area = Rect::new(log_x, log_area.y, log_width, log_area.height);

        let log_block = Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Rgb(60, 60, 80)))
            .title(Span::styled(
                " Connection Log ",
                Style::default().fg(Color::Rgb(100, 100, 120)),
            ));

        let log_paragraph = Paragraph::new(log_lines)
            .block(log_block)
            .wrap(Wrap { trim: false });

        Widget::render(log_paragraph, centered_log_area, buf);

        // Render hint at bottom
        let hint = Paragraph::new(Line::from(vec![
            Span::styled("Press ", Style::default().fg(Color::Rgb(80, 80, 100))),
            Span::styled("Ctrl+C", Style::default().fg(Color::Rgb(120, 120, 140)).add_modifier(Modifier::BOLD)),
            Span::styled(" to cancel", Style::default().fg(Color::Rgb(80, 80, 100))),
        ]))
        .alignment(Alignment::Center);
        Widget::render(hint, chunks[4], buf);
    }
}
