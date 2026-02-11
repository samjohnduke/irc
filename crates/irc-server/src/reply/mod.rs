//! Reply generation helpers.
//!
//! This module provides utilities for generating IRC numeric replies.

mod welcome;

pub use welcome::send_welcome_burst;

use irc_proto::{Command, Message, Prefix};

use crate::state::Client;

/// Helper for building numeric replies.
pub struct ReplyBuilder<'a> {
    server_name: &'a str,
    target: String,
}

impl<'a> ReplyBuilder<'a> {
    /// Create a new reply builder.
    pub fn new(server_name: &'a str, client: &Client) -> Self {
        Self {
            server_name,
            target: client.nickname().unwrap_or_else(|| "*".to_string()),
        }
    }

    /// Build a numeric reply message.
    pub fn numeric(&self, code: u16, params: Vec<String>) -> Message {
        Message::with_prefix(
            Prefix::from_server(self.server_name),
            Command::Numeric {
                code,
                target: self.target.clone(),
                params,
            },
        )
    }

    /// Build and send a numeric reply.
    pub fn send(&self, client: &Client, code: u16, params: Vec<String>) {
        client.send(self.numeric(code, params));
    }
}

/// Format a list of items for ISUPPORT.
pub fn format_isupport(items: &[(&str, Option<&str>)]) -> String {
    items
        .iter()
        .map(|(key, value)| match value {
            Some(v) => format!("{}={}", key, v),
            None => (*key).to_string(),
        })
        .collect::<Vec<_>>()
        .join(" ")
}
