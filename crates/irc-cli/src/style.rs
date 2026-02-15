//! Styling and theming for the TUI.

use ratatui::style::{Color, Modifier, Style};

/// Application theme colors.
#[allow(dead_code)]
pub struct Theme {
    /// Background color for main areas.
    pub bg: Color,
    /// Default foreground text color.
    pub fg: Color,
    /// Accent/highlight color.
    pub accent: Color,
    /// Error/warning color.
    pub error: Color,
    /// Muted/secondary text color.
    pub muted: Color,
    /// Timestamp color.
    pub timestamp: Color,
    /// Server message color.
    pub server: Color,
    /// Action (/me) color.
    pub action: Color,
    /// Join/part message color.
    pub join_part: Color,
    /// Selected item background.
    pub selection_bg: Color,
    /// Selected item foreground.
    pub selection_fg: Color,
    /// Unread indicator color.
    pub unread: Color,
    /// Input prompt color.
    pub prompt: Color,
    /// Border color.
    pub border: Color,
    /// Title color.
    pub title: Color,
    /// Status bar background.
    pub statusbar_bg: Color,
    /// Status bar foreground.
    pub statusbar_fg: Color,
}

impl Default for Theme {
    fn default() -> Self {
        Self {
            bg: Color::Rgb(20, 20, 28),
            fg: Color::Rgb(220, 220, 230),
            accent: Color::Rgb(100, 180, 255),
            error: Color::Rgb(255, 100, 100),
            muted: Color::Rgb(100, 100, 120),
            timestamp: Color::Rgb(80, 80, 100),
            server: Color::Rgb(200, 180, 100),
            action: Color::Rgb(200, 150, 255),
            join_part: Color::Rgb(80, 120, 80),
            selection_bg: Color::Rgb(50, 60, 80),
            selection_fg: Color::Rgb(255, 255, 255),
            unread: Color::Rgb(100, 200, 100),
            prompt: Color::Rgb(100, 180, 255),
            border: Color::Rgb(50, 55, 70),
            title: Color::Rgb(150, 180, 220),
            statusbar_bg: Color::Rgb(35, 40, 55),
            statusbar_fg: Color::Rgb(180, 185, 200),
        }
    }
}

impl Theme {
    /// Style for timestamps.
    pub fn timestamp_style(&self) -> Style {
        Style::default().fg(self.timestamp)
    }

    /// Style for regular message text.
    pub fn message_style(&self) -> Style {
        Style::default().fg(self.fg)
    }

    /// Style for server messages.
    pub fn server_style(&self) -> Style {
        Style::default().fg(self.server)
    }

    /// Style for action messages (/me).
    pub fn action_style(&self) -> Style {
        Style::default().fg(self.action)
    }

    /// Style for join/part messages.
    #[allow(dead_code)]
    pub fn join_part_style(&self) -> Style {
        Style::default().fg(self.join_part)
    }

    /// Style for error messages.
    pub fn error_style(&self) -> Style {
        Style::default().fg(self.error)
    }

    /// Style for selected items.
    #[allow(dead_code)]
    pub fn selected_style(&self) -> Style {
        Style::default()
            .bg(self.selection_bg)
            .fg(self.selection_fg)
            .add_modifier(Modifier::BOLD)
    }

    /// Style for unread indicators.
    pub fn unread_style(&self) -> Style {
        Style::default()
            .fg(self.unread)
            .add_modifier(Modifier::BOLD)
    }

    /// Style for the input prompt.
    #[allow(dead_code)]
    pub fn prompt_style(&self) -> Style {
        Style::default()
            .fg(self.prompt)
            .add_modifier(Modifier::BOLD)
    }

    /// Style for muted text.
    pub fn muted_style(&self) -> Style {
        Style::default().fg(self.muted)
    }

    /// Style for borders.
    pub fn border_style(&self) -> Style {
        Style::default().fg(self.border)
    }

    /// Style for titles.
    pub fn title_style(&self) -> Style {
        Style::default().fg(self.title).add_modifier(Modifier::BOLD)
    }

    /// Style for status bar.
    pub fn statusbar_style(&self) -> Style {
        Style::default().bg(self.statusbar_bg).fg(self.statusbar_fg)
    }
}

/// Generate a consistent color for a nickname.
///
/// Uses a simple hash to assign colors so the same nick always
/// gets the same color. Uses softer, more readable colors.
pub fn nick_color(nick: &str) -> Color {
    // Modern IRC-style nick colors (softer, more readable)
    const NICK_COLORS: [Color; 16] = [
        Color::Rgb(255, 120, 120), // Soft red
        Color::Rgb(120, 220, 120), // Soft green
        Color::Rgb(255, 200, 100), // Soft yellow
        Color::Rgb(120, 160, 255), // Soft blue
        Color::Rgb(220, 140, 255), // Soft magenta
        Color::Rgb(100, 220, 220), // Soft cyan
        Color::Rgb(255, 160, 120), // Soft orange
        Color::Rgb(180, 220, 140), // Lime
        Color::Rgb(255, 180, 200), // Pink
        Color::Rgb(140, 200, 255), // Sky blue
        Color::Rgb(200, 160, 255), // Lavender
        Color::Rgb(140, 230, 200), // Teal
        Color::Rgb(255, 200, 150), // Peach
        Color::Rgb(180, 180, 255), // Periwinkle
        Color::Rgb(200, 255, 200), // Mint
        Color::Rgb(255, 220, 180), // Cream
    ];

    // Simple DJB2 hash
    let mut hash: u32 = 5381;
    for c in nick.bytes() {
        hash = hash.wrapping_mul(33).wrapping_add(c as u32);
    }

    NICK_COLORS[(hash as usize) % NICK_COLORS.len()]
}

/// Format a timestamp for display.
pub fn format_timestamp(dt: &chrono::DateTime<chrono::Utc>) -> String {
    dt.format("%H:%M").to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_nick_color_consistency() {
        // Same nick should always get same color
        let color1 = nick_color("alice");
        let color2 = nick_color("alice");
        assert_eq!(color1, color2);

        // Different nicks might get different colors
        let color3 = nick_color("bob");
        // Note: could be same by chance, so we don't assert inequality
        let _ = color3;
    }
}
