//! IRC Services (NickServ, ChanServ).
//!
//! This module provides pseudo-client services for nickname and channel
//! registration/management.

pub mod chanserv;
pub mod common;
pub mod nickserv;

pub use common::ServiceContext;

use crate::error::Result;
use crate::handler::HandlerContext;

/// Check if a nickname is a service.
pub fn is_service_nick(nick: &str) -> bool {
    matches!(
        nick.to_uppercase().as_str(),
        "NICKSERV" | "CHANSERV" | "NS" | "CS"
    )
}

/// Handle a message to a service.
///
/// Returns Ok(true) if the message was handled by a service, Ok(false) otherwise.
pub fn handle_service_message(ctx: &HandlerContext, target: &str, message: &str) -> Result<bool> {
    let target_upper = target.to_uppercase();

    // Parse command and arguments
    let args: Vec<&str> = message.split_whitespace().collect();

    match target_upper.as_str() {
        "NICKSERV" | "NS" => {
            let sctx = ServiceContext::new(ctx, "NickServ");
            nickserv::handle_command(&sctx, &args)?;
            Ok(true)
        }
        "CHANSERV" | "CS" => {
            let sctx = ServiceContext::new(ctx, "ChanServ");
            chanserv::handle_command(&sctx, &args)?;
            Ok(true)
        }
        _ => Ok(false),
    }
}
