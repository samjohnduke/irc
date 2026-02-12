//! Styling and theming for the TUI.

use ratatui::style::{Color, Modifier, Style};

/// Application theme colors.
pub struct Theme {
    /// Background color for main areas.
    pub bg: Color,
    /// Default foreground text color.
    pub fg: Color,
    /// Accent/highlight color.
    #[allow(dead_code)]
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
    /// Unread indicator color.
    pub unread: Color,
    /// Input prompt color.
    pub prompt: Color,
}

impl Default for Theme {
    fn default() -> Self {
        Self {
            bg: Color::Reset,
            fg: Color::Reset,
            accent: Color::Cyan,
            error: Color::Red,
            muted: Color::DarkGray,
            timestamp: Color::DarkGray,
            server: Color::Yellow,
            action: Color::Magenta,
            join_part: Color::DarkGray,
            selection_bg: Color::DarkGray,
            unread: Color::Green,
            prompt: Color::Cyan,
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
    pub fn join_part_style(&self) -> Style {
        Style::default().fg(self.join_part)
    }

    /// Style for error messages.
    pub fn error_style(&self) -> Style {
        Style::default().fg(self.error)
    }

    /// Style for selected items.
    pub fn selected_style(&self) -> Style {
        Style::default().bg(self.selection_bg).add_modifier(Modifier::BOLD)
    }

    /// Style for unread indicators.
    pub fn unread_style(&self) -> Style {
        Style::default().fg(self.unread).add_modifier(Modifier::BOLD)
    }

    /// Style for the input prompt.
    pub fn prompt_style(&self) -> Style {
        Style::default().fg(self.prompt).add_modifier(Modifier::BOLD)
    }

    /// Style for muted text.
    pub fn muted_style(&self) -> Style {
        Style::default().fg(self.muted)
    }

}

/// Generate a consistent color for a nickname.
///
/// Uses a simple hash to assign colors so the same nick always
/// gets the same color.
pub fn nick_color(nick: &str) -> Color {
    // IRC-style nick colors
    const NICK_COLORS: [Color; 12] = [
        Color::Red,
        Color::Green,
        Color::Yellow,
        Color::Blue,
        Color::Magenta,
        Color::Cyan,
        Color::LightRed,
        Color::LightGreen,
        Color::LightYellow,
        Color::LightBlue,
        Color::LightMagenta,
        Color::LightCyan,
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
