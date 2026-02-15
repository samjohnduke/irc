//! Miscellaneous command handlers (PING, PONG, AWAY, SETNAME).

use irc_proto::{Command, Message, replies::*};

use super::HandlerContext;
use crate::cap::extensions::broadcast_away_notify;
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

            // Broadcast away-notify to common channel members
            let _ = broadcast_away_notify(ctx, Some(msg));

            ctx.reply(
                RPL_NOWAWAY,
                vec!["You have been marked as being away".into()],
            )?;

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

            // Broadcast away-notify to common channel members
            let _ = broadcast_away_notify(ctx, None);

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

/// Handle SETNAME command (IRCv3).
///
/// Allows users to change their realname.
pub fn handle_setname(ctx: &HandlerContext, new_realname: &str) -> Result<()> {
    use crate::cap::extensions::broadcast_setname;

    if new_realname.is_empty() {
        ctx.reply(
            irc_proto::errors::ERR_NEEDMOREPARAMS,
            vec!["SETNAME".into(), "Not enough parameters".into()],
        )?;
        return Ok(());
    }

    // Update the realname
    {
        let username = ctx.client.username()?.unwrap_or_default();
        ctx.client.set_user(username, new_realname.to_string())?;
    }

    // Broadcast to clients with setname cap
    let _ = broadcast_setname(ctx, new_realname);

    tracing::debug!(
        client_id = %ctx.client.id,
        nick = ?ctx.client.nickname()?,
        new_realname = %new_realname,
        "Client changed realname"
    );

    Ok(())
}
