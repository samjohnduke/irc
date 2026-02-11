//! Miscellaneous command handlers (PING, PONG, AWAY).

use irc_proto::{replies::*, Command, Message};

use super::HandlerContext;
use crate::error::Result;

/// Handle PING command.
///
/// Responds with a PONG to keep the connection alive.
pub fn handle_ping(ctx: &HandlerContext, server1: &str, _server2: Option<&str>) -> Result<()> {
    // Send PONG from the server
    let pong = Message::with_prefix(
        ctx.server_prefix(),
        Command::Pong {
            server1: ctx.server_name().to_string(),
            server2: Some(server1.to_string()),
        },
    );
    ctx.client.send(pong)?;

    Ok(())
}

/// Handle PONG command.
///
/// Acknowledges a PING from the server.
pub fn handle_pong(_ctx: &HandlerContext) -> Result<()> {
    // TODO: Update last activity time for ping timeout tracking
    Ok(())
}

/// Handle AWAY command.
///
/// Sets or clears the away message.
pub fn handle_away(ctx: &HandlerContext, message: Option<&str>) -> Result<()> {
    match message {
        Some(msg) if !msg.is_empty() => {
            // Set away message
            ctx.client.set_away(Some(msg.to_string()))?;
            ctx.reply(RPL_NOWAWAY, vec!["You have been marked as being away".into()])?;

            tracing::debug!(
                client_id = %ctx.client.id,
                nick = ?ctx.client.nickname()?,
                message = %msg,
                "Client is now away"
            );
        }
        _ => {
            // Clear away message
            ctx.client.set_away(None)?;
            ctx.reply(
                RPL_UNAWAY,
                vec!["You are no longer marked as being away".into()],
            )?;

            tracing::debug!(
                client_id = %ctx.client.id,
                nick = ?ctx.client.nickname()?,
                "Client is no longer away"
            );
        }
    }

    Ok(())
}
