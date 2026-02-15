//! CHATHISTORY command handler.
//!
//! Implements the IRCv3 CHATHISTORY extension for message replay.

use irc_proto::{Command, Message, is_channel};

use super::HandlerContext;
use crate::db::history;
use crate::error::{Error, Result};

/// Handle CHATHISTORY command.
///
/// Subcommands:
/// - CHATHISTORY LATEST <target> * <limit>
/// - CHATHISTORY BEFORE <target> <msgid> <limit>
/// - CHATHISTORY AFTER <target> <msgid> <limit>
/// - CHATHISTORY BETWEEN <target> <start_msgid> <end_msgid> <limit>
pub fn handle_chathistory(
    ctx: &HandlerContext,
    subcommand: &str,
    target: &str,
    params: &[String],
) -> Result<()> {
    // Check if database is available
    let db = match &ctx.state.db {
        Some(db) => db,
        None => {
            // Send FAIL response
            send_fail(
                ctx,
                "CHATHISTORY",
                "UNKNOWN_ERROR",
                "*",
                "Chat history not available",
            )?;
            return Ok(());
        }
    };

    // Check if client has the capability enabled
    if !ctx.client.has_cap("draft/chathistory")? {
        send_fail(
            ctx,
            "CHATHISTORY",
            "UNKNOWN_ERROR",
            target,
            "Capability not enabled",
        )?;
        return Ok(());
    }

    // Check permission - user must be in channel or target is themselves
    if !can_access_history(ctx, target)? {
        send_fail(
            ctx,
            "CHATHISTORY",
            "INVALID_TARGET",
            target,
            "Cannot access history for this target",
        )?;
        return Ok(());
    }

    let max_limit = ctx.state.config.limits.max_history;
    let conn = db.connection()?;

    let messages = match subcommand.to_uppercase().as_str() {
        "LATEST" => {
            // CHATHISTORY LATEST #channel * 50
            let limit = parse_limit(params.get(1).map(String::as_str), max_limit);
            history::get_latest(&conn, target, limit)?
        }
        "BEFORE" => {
            // CHATHISTORY BEFORE #channel msgid=xxx 50
            let msgid = params
                .first()
                .ok_or_else(|| Error::NeedMoreParams("CHATHISTORY".into()))?;
            let msgid = parse_msgid(msgid);
            let limit = parse_limit(params.get(1).map(String::as_str), max_limit);
            history::get_before(&conn, target, &msgid, limit)?
        }
        "AFTER" => {
            // CHATHISTORY AFTER #channel msgid=xxx 50
            let msgid = params
                .first()
                .ok_or_else(|| Error::NeedMoreParams("CHATHISTORY".into()))?;
            let msgid = parse_msgid(msgid);
            let limit = parse_limit(params.get(1).map(String::as_str), max_limit);
            history::get_after(&conn, target, &msgid, limit)?
        }
        "BETWEEN" => {
            // CHATHISTORY BETWEEN #channel msgid=start msgid=end 50
            let start_msgid = params
                .first()
                .ok_or_else(|| Error::NeedMoreParams("CHATHISTORY".into()))?;
            let end_msgid = params
                .get(1)
                .ok_or_else(|| Error::NeedMoreParams("CHATHISTORY".into()))?;
            let start_msgid = parse_msgid(start_msgid);
            let end_msgid = parse_msgid(end_msgid);
            let limit = parse_limit(params.get(2).map(String::as_str), max_limit);
            history::get_between(&conn, target, &start_msgid, &end_msgid, limit)?
        }
        "TARGETS" => {
            // Not implemented - would list available targets
            send_fail(
                ctx,
                "CHATHISTORY",
                "UNKNOWN_COMMAND",
                "*",
                "TARGETS not implemented",
            )?;
            return Ok(());
        }
        _ => {
            send_fail(
                ctx,
                "CHATHISTORY",
                "UNKNOWN_COMMAND",
                "*",
                "Unknown CHATHISTORY subcommand",
            )?;
            return Ok(());
        }
    };

    // Send as BATCH
    send_history_batch(ctx, target, &messages)?;

    Ok(())
}

/// Check if the client can access history for a target.
fn can_access_history(ctx: &HandlerContext, target: &str) -> Result<bool> {
    if is_channel(target) {
        // Must be in the channel
        ctx.client.is_in_channel(target)
    } else {
        // For private messages, must be the target or the sender
        let nick = ctx.client.nickname()?.unwrap_or_default();
        Ok(target.eq_ignore_ascii_case(&nick))
    }
}

/// Parse a limit parameter.
fn parse_limit(param: Option<&str>, max: usize) -> usize {
    param
        .and_then(|s| s.parse::<usize>().ok())
        .map(|n| n.min(max))
        .unwrap_or(50)
        .min(max)
}

/// Parse a msgid parameter (may be prefixed with "msgid=").
fn parse_msgid(param: &str) -> String {
    if let Some(stripped) = param.strip_prefix("msgid=") {
        stripped.to_string()
    } else {
        param.to_string()
    }
}

/// Generate a unique batch ID.
fn generate_batch_id() -> String {
    use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
    let random: u64 = rand::random();
    URL_SAFE_NO_PAD.encode(random.to_be_bytes())
}

/// Send history messages in a BATCH.
fn send_history_batch(
    ctx: &HandlerContext,
    target: &str,
    messages: &[history::HistoryMessage],
) -> Result<()> {
    let batch_id = generate_batch_id();

    // Start batch
    let batch_start = Message::with_prefix(
        ctx.server_prefix(),
        Command::Batch {
            reference: format!("+{}", batch_id),
            batch_type: Some("chathistory".into()),
            params: vec![target.to_string()],
        },
    );
    ctx.client.send(batch_start)?;

    // Send messages with batch tag
    for msg in messages {
        let mut irc_msg = msg.to_irc_message();

        // Add batch tag
        let tags = irc_msg.tags.get_or_insert_with(irc_proto::Tags::new);
        tags.set("batch", &batch_id);

        ctx.client.send(irc_msg)?;
    }

    // End batch
    let batch_end = Message::with_prefix(
        ctx.server_prefix(),
        Command::Batch {
            reference: format!("-{}", batch_id),
            batch_type: None,
            params: vec![],
        },
    );
    ctx.client.send(batch_end)?;

    Ok(())
}

/// Send a FAIL response for CHATHISTORY errors.
fn send_fail(
    ctx: &HandlerContext,
    command: &str,
    code: &str,
    context: &str,
    description: &str,
) -> Result<()> {
    // FAIL command code context :description
    let fail_msg = Message::with_prefix(
        ctx.server_prefix(),
        Command::Unknown {
            command: "FAIL".into(),
            params: vec![
                command.to_string(),
                code.to_string(),
                context.to_string(),
                description.to_string(),
            ],
        },
    );
    ctx.client.send(fail_msg)?;
    Ok(())
}
