//! Messaging command handlers (PRIVMSG, NOTICE).

use irc_proto::{errors::*, replies::*, Command, Message};

use super::HandlerContext;
use crate::error::{Error, Result};
use crate::lock::RwLockExt;

/// Handle PRIVMSG command.
///
/// Routes private messages to users or channels.
pub fn handle_privmsg(ctx: &HandlerContext, target: &str, message: &str) -> Result<()> {
    if target.is_empty() {
        ctx.reply(ERR_NORECIPIENT, vec!["No recipient given (PRIVMSG)".into()])?;
        return Err(Error::NeedMoreParams("PRIVMSG".into()));
    }

    if message.is_empty() {
        ctx.reply(ERR_NOTEXTTOSEND, vec!["No text to send".into()])?;
        return Err(Error::NeedMoreParams("PRIVMSG".into()));
    }

    if irc_proto::is_channel(target) {
        return send_to_channel(ctx, target, message, true);
    }

    // User message
    send_to_user(ctx, target, message, true)
}

/// Handle NOTICE command.
///
/// Routes notices to users or channels.
pub fn handle_notice(ctx: &HandlerContext, target: &str, message: &str) -> Result<()> {
    if target.is_empty() || message.is_empty() {
        // NOTICE should not generate error replies (RFC 2812)
        return Ok(());
    }

    if irc_proto::is_channel(target) {
        // Channel notice - don't send errors
        let _ = send_to_channel(ctx, target, message, false);
        return Ok(());
    }

    // User notice - don't send errors for NOTICE
    let _ = send_to_user(ctx, target, message, false);
    Ok(())
}

/// Send a message to a channel.
fn send_to_channel(
    ctx: &HandlerContext,
    target: &str,
    message: &str,
    send_errors: bool,
) -> Result<()> {
    // Find the channel
    let channel_arc = match ctx.state.get_channel(target) {
        Some(c) => c,
        None => {
            if send_errors {
                ctx.reply(
                    ERR_NOSUCHCHANNEL,
                    vec![target.to_string(), "No such channel".into()],
                )?;
            }
            return Err(Error::NoSuchChannel(target.to_string()));
        }
    };

    let channel = channel_arc.read_lock("channel")?;
    let client_id = ctx.client.id;
    let is_member = channel.is_member(client_id);

    // Check if client can speak
    if !channel.can_speak(client_id, is_member) {
        if send_errors {
            ctx.reply(
                ERR_CANNOTSENDTOCHAN,
                vec![target.to_string(), "Cannot send to channel".into()],
            )?;
        }
        return Err(Error::CannotSendToChannel(target.to_string()));
    }

    // Build the message from the sender
    let msg = Message::with_prefix(
        ctx.client.prefix()?,
        if send_errors {
            Command::Privmsg {
                target: target.to_string(),
                message: message.to_string(),
            }
        } else {
            Command::Notice {
                target: target.to_string(),
                message: message.to_string(),
            }
        },
    );

    // Broadcast to all members except sender
    ctx.state.broadcast_to_channel(&channel, msg, Some(client_id));

    tracing::debug!(
        from = ?ctx.client.nickname()?,
        to = %target,
        message = %message,
        "Channel message delivered"
    );

    Ok(())
}

/// Send a message to a user.
fn send_to_user(
    ctx: &HandlerContext,
    target: &str,
    message: &str,
    send_errors: bool,
) -> Result<()> {
    // Find the target user
    let target_client = match ctx.state.find_client_by_nick(target) {
        Some(client) => client,
        None => {
            if send_errors {
                ctx.reply(
                    ERR_NOSUCHNICK,
                    vec![target.to_string(), "No such nick/channel".into()],
                )?;
            }
            return Err(Error::NoSuchNick(target.to_string()));
        }
    };

    // Check if target is away
    if let Some(away_msg) = target_client.away_message()? {
        if send_errors {
            ctx.reply(
                RPL_AWAY,
                vec![
                    target_client.nickname()?.unwrap_or_default(),
                    away_msg,
                ],
            )?;
        }
    }

    // Build the message from the sender
    let msg = Message::with_prefix(
        ctx.client.prefix()?,
        if send_errors {
            Command::Privmsg {
                target: target.to_string(),
                message: message.to_string(),
            }
        } else {
            Command::Notice {
                target: target.to_string(),
                message: message.to_string(),
            }
        },
    );

    // Send to target
    match target_client.send(msg) {
        Ok(true) => {}
        Ok(false) => {
            // Client disconnected
            if send_errors {
                ctx.reply(
                    ERR_NOSUCHNICK,
                    vec![target.to_string(), "No such nick/channel".into()],
                )?;
            }
            return Err(Error::NoSuchNick(target.to_string()));
        }
        Err(e) => {
            // Send buffer full - client too slow
            tracing::debug!(target = %target, error = %e, "Failed to send message");
            return Err(e);
        }
    }

    tracing::debug!(
        from = ?ctx.client.nickname()?,
        to = %target,
        message = %message,
        "Message delivered"
    );

    Ok(())
}
