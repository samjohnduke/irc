//! User list sidebar widget.

use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Color, Modifier, Style},
    symbols,
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, Widget},
};

use crate::style::{Theme, nick_color};

/// User status/modes in the channel.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum UserStatus {
    Owner,  // ~
    Admin,  // &
    Op,     // @
    HalfOp, // %
    Voice,  // +
    Normal,
}

impl UserStatus {
    #[allow(dead_code)]
    pub fn from_prefix(c: char) -> Option<Self> {
        match c {
            '~' => Some(Self::Owner),
            '&' => Some(Self::Admin),
            '@' => Some(Self::Op),
            '%' => Some(Self::HalfOp),
            '+' => Some(Self::Voice),
            _ => None,
        }
    }

    pub fn symbol(&self) -> &'static str {
        match self {
            Self::Owner => "~",
            Self::Admin => "&",
            Self::Op => "@",
            Self::HalfOp => "%",
            Self::Voice => "+",
            Self::Normal => " ",
        }
    }

    pub fn color(&self) -> Color {
        match self {
            Self::Owner => Color::Rgb(255, 215, 0),    // Gold
            Self::Admin => Color::Rgb(220, 100, 255),  // Purple
            Self::Op => Color::Rgb(100, 220, 100),     // Green
            Self::HalfOp => Color::Rgb(100, 200, 220), // Cyan
            Self::Voice => Color::Rgb(255, 200, 100),  // Yellow
            Self::Normal => Color::Rgb(100, 100, 120), // Muted
        }
    }
}

/// A user in the channel.
#[derive(Debug, Clone)]
pub struct ChannelUser {
    pub nick: String,
    pub status: UserStatus,
    pub away: bool,
}

impl ChannelUser {
    #[allow(dead_code)]
    pub fn new(nick: String, status: UserStatus) -> Self {
        Self {
            nick,
            status,
            away: false,
        }
    }

    /// Parse nick with prefix (e.g., "@alice" -> Op, "alice")
    #[allow(dead_code)]
    pub fn from_prefixed_nick(prefixed: &str) -> Self {
        if let Some(first) = prefixed.chars().next() {
            if let Some(status) = UserStatus::from_prefix(first) {
                return Self::new(prefixed[1..].to_string(), status);
            }
        }
        Self::new(prefixed.to_string(), UserStatus::Normal)
    }
}

/// Widget for displaying the user list in a channel.
pub struct UserListWidget<'a> {
    users: &'a [ChannelUser],
    theme: &'a Theme,
    #[allow(dead_code)]
    channel_name: Option<&'a str>,
}

impl<'a> UserListWidget<'a> {
    pub fn new(users: &'a [ChannelUser], channel_name: Option<&'a str>, theme: &'a Theme) -> Self {
        Self {
            users,
            theme,
            channel_name,
        }
    }
}

impl Widget for UserListWidget<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        // Fill background
        let bg = Color::Rgb(25, 25, 35);
        for y in area.y..area.y + area.height {
            for x in area.x..area.x + area.width {
                buf[(x, y)].set_bg(bg);
            }
        }

        // Count users by status
        let ops = self
            .users
            .iter()
            .filter(|u| {
                matches!(
                    u.status,
                    UserStatus::Owner | UserStatus::Admin | UserStatus::Op
                )
            })
            .count();
        let voiced = self
            .users
            .iter()
            .filter(|u| u.status == UserStatus::Voice)
            .count();
        let normal = self.users.len() - ops - voiced;

        // Sort users by status then name
        let mut sorted_users: Vec<_> = self.users.iter().collect();
        sorted_users.sort_by(|a, b| {
            a.status
                .cmp(&b.status)
                .then_with(|| a.nick.to_lowercase().cmp(&b.nick.to_lowercase()))
        });

        let items: Vec<ListItem> = sorted_users
            .iter()
            .map(|user| {
                let mut spans = Vec::new();

                // Status symbol
                spans.push(Span::styled(
                    user.status.symbol(),
                    Style::default()
                        .fg(user.status.color())
                        .add_modifier(Modifier::BOLD),
                ));

                // Nick with color
                let nick_style = if user.away {
                    Style::default()
                        .fg(self.theme.muted)
                        .add_modifier(Modifier::DIM)
                } else {
                    Style::default().fg(nick_color(&user.nick))
                };

                // Truncate nick if needed
                let max_len = area.width.saturating_sub(4) as usize;
                let display_nick = if user.nick.len() > max_len {
                    format!("{}…", &user.nick[..max_len.saturating_sub(1)])
                } else {
                    user.nick.clone()
                };

                spans.push(Span::styled(display_nick, nick_style));

                // Away indicator
                if user.away {
                    spans.push(Span::styled(
                        " (away)",
                        Style::default().fg(self.theme.muted),
                    ));
                }

                ListItem::new(Line::from(spans))
            })
            .collect();

        // Title with user count
        let title = format!(" Users ({}) ", self.users.len());

        // Subtitle with breakdown
        let subtitle = if ops > 0 || voiced > 0 {
            format!(" @{} +{} •{} ", ops, voiced, normal)
        } else {
            String::new()
        };

        let block = Block::default()
            .borders(Borders::LEFT)
            .border_style(self.theme.border_style())
            .border_set(symbols::border::PLAIN)
            .title(Span::styled(title, self.theme.title_style()))
            .title_bottom(Span::styled(
                subtitle,
                Style::default().fg(self.theme.muted),
            ))
            .style(Style::default().bg(bg));

        let list = List::new(items).block(block);

        Widget::render(list, area, buf);
    }
}

/// Empty user list placeholder for non-channel buffers.
pub struct EmptyUserListWidget<'a> {
    theme: &'a Theme,
}

impl<'a> EmptyUserListWidget<'a> {
    pub fn new(theme: &'a Theme) -> Self {
        Self { theme }
    }
}

impl Widget for EmptyUserListWidget<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let bg = Color::Rgb(25, 25, 35);
        for y in area.y..area.y + area.height {
            for x in area.x..area.x + area.width {
                buf[(x, y)].set_bg(bg);
            }
        }

        let block = Block::default()
            .borders(Borders::LEFT)
            .border_style(self.theme.border_style())
            .border_set(symbols::border::PLAIN)
            .style(Style::default().bg(bg));

        Widget::render(block, area, buf);
    }
}
