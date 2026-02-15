//! Messaging command handlers (PRIVMSG, NOTICE).

use irc_proto::{Command, Message, errors::*, replies::*};

use super::HandlerContext;
use crate::error::{Error, Result};
use crate::lock::RwLockExt;
use crate::services;

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

    // Check if target is a service (NickServ, ChanServ)
    if services::is_service_nick(target) {
        return services::handle_service_message(ctx, target, message).map(|_| ());
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
    let mut msg = Message::with_prefix(
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

    // Add account tag if sender is identified (for account-tag capability)
    let sender_account = ctx.client.account()?;
    crate::cap::extensions::add_account_tag(&mut msg, sender_account.as_deref());

    // Add msgid tag (used by message-ids capability and history)
    let _msgid = crate::cap::extensions::add_msgid_tag(&mut msg);

    // Broadcast to all members except sender
    // Only include account tag for clients with account-tag capability
    // Only include msgid tag for clients with message-ids capability
    for member_id in channel.members.keys() {
        if *member_id == client_id {
            continue;
        }
        if let Some(member) = ctx.state.clients.get(member_id) {
            let mut member_msg = msg.clone();
            // Remove account tag if member doesn't have the cap
            if !member.has_cap("account-tag")?
                && let Some(ref mut tags) = member_msg.tags
            {
                tags.remove("account");
            }
            // Remove msgid tag if member doesn't have the cap
            if !member.has_cap("message-ids")?
                && let Some(ref mut tags) = member_msg.tags
            {
                tags.remove("msgid");
            }
            let _ = member.send_with_tags(member_msg);
        }
    }

    // Echo back to sender if echo-message is enabled
    if ctx.client.has_cap("echo-message")? {
        // Keep msgid in echo for sender if they support message-ids
        let mut echo_msg = msg.clone();
        if !ctx.client.has_cap("message-ids")?
            && let Some(ref mut tags) = echo_msg.tags
        {
            tags.remove("msgid");
        }
        ctx.client.send_with_tags(echo_msg)?;
    }

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
    if let Some(away_msg) = target_client.away_message()?
        && send_errors
    {
        ctx.reply(
            RPL_AWAY,
            vec![target_client.nickname()?.unwrap_or_default(), away_msg],
        )?;
    }

    // Build the message from the sender
    let mut msg = Message::with_prefix(
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

    // Add account tag if sender is identified
    let sender_account = ctx.client.account()?;
    crate::cap::extensions::add_account_tag(&mut msg, sender_account.as_deref());

    // Add msgid tag (used by message-ids capability and history)
    let _msgid = crate::cap::extensions::add_msgid_tag(&mut msg);

    // Prepare message for target (remove tags they don't support)
    let mut target_msg = msg.clone();
    if !target_client.has_cap("account-tag")?
        && let Some(ref mut tags) = target_msg.tags
    {
        tags.remove("account");
    }
    if !target_client.has_cap("message-ids")?
        && let Some(ref mut tags) = target_msg.tags
    {
        tags.remove("msgid");
    }

    // Send to target
    match target_client.send_with_tags(target_msg) {
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

    // Echo back to sender if echo-message is enabled
    if ctx.client.has_cap("echo-message")? {
        // Keep msgid in echo for sender if they support message-ids
        let mut echo_msg = msg.clone();
        if !ctx.client.has_cap("message-ids")?
            && let Some(ref mut tags) = echo_msg.tags
        {
            tags.remove("msgid");
        }
        ctx.client.send_with_tags(echo_msg)?;
    }

    tracing::debug!(
        from = ?ctx.client.nickname()?,
        to = %target,
        message = %message,
        "Message delivered"
    );

    Ok(())
}
