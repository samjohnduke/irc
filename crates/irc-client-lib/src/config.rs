//! Client configuration.

use serde::Deserialize;

/// Client configuration.
#[derive(Debug, Deserialize)]
pub struct ClientConfig {
    /// Nicknames to try (in order)
    pub nicknames: Vec<String>,

    /// Username
    pub username: String,

    /// Real name
    pub realname: String,
}

impl Default for ClientConfig {
    fn default() -> Self {
        Self {
            nicknames: vec!["user".into()],
            username: "user".into(),
            realname: "IRC User".into(),
        }
    }
}
