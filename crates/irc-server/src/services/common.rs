//! Common service utilities.

use irc_proto::{Command, Message, Prefix};

use crate::db::Database;
use crate::error::{Error, Result};
use crate::handler::HandlerContext;

/// Context for service command handling.
pub struct ServiceContext<'a> {
    /// The underlying handler context.
    pub ctx: &'a HandlerContext<'a>,
    /// The service name (e.g., "NickServ").
    pub service_name: &'static str,
}

impl<'a> ServiceContext<'a> {
    /// Create a new service context.
    pub fn new(ctx: &'a HandlerContext<'a>, service_name: &'static str) -> Self {
        Self { ctx, service_name }
    }

    /// Get the service prefix for outgoing messages.
    fn service_prefix(&self) -> Prefix {
        Prefix::from_user(
            self.service_name.to_string(),
            "service".to_string(),
            self.ctx.server_name().to_string(),
        )
    }

    /// Send a NOTICE reply to the user from this service.
    pub fn reply(&self, message: &str) -> Result<()> {
        let target = self.ctx.client.nickname()?.unwrap_or_else(|| "*".to_string());
        let msg = Message::with_prefix(
            self.service_prefix(),
            Command::Notice {
                target,
                message: message.to_string(),
            },
        );
        self.ctx.client.send(msg)?;
        Ok(())
    }

    /// Send an error reply to the user.
    pub fn error(&self, message: &str) -> Result<()> {
        self.reply(&format!("Error: {}", message))
    }

    /// Require that the database is available.
    pub fn require_db(&self) -> Result<&Database> {
        self.ctx
            .state
            .db
            .as_ref()
            .map(|arc| arc.as_ref())
            .ok_or_else(|| {
                let _ = self.error("Services are not available (no database configured).");
                Error::ServicesUnavailable
            })
    }

    /// Require that the user is logged in.
    pub fn require_account(&self) -> Result<String> {
        self.ctx.client.account()?.ok_or_else(|| {
            let _ = self.error("You are not logged in.");
            Error::NotLoggedIn
        })
    }

    /// Get the user's current nickname.
    pub fn nickname(&self) -> Result<String> {
        self.ctx
            .client
            .nickname()?
            .ok_or_else(|| Error::NotRegistered)
    }
}
