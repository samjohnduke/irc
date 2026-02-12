//! MARKREAD command handler for draft/read-marker.
//!
//! This implements read position synchronization as per the IRCv3
//! draft/read-marker specification.

use chrono::{DateTime, TimeZone, Utc};
use irc_proto::Command;

use super::HandlerContext;
use crate::db::{accounts, read_markers};
use crate::error::Result;

/// Handle MARKREAD command.
///
/// Formats:
/// - MARKREAD <target> - Query read position
/// - MARKREAD <target> timestamp=<timestamp> - Set read position
pub fn handle_markread(ctx: &HandlerContext, target: &str, timestamp_param: Option<&str>) -> Result<()> {
    // Check if client has the capability enabled
    if !ctx.client.has_cap("draft/read-marker")? {
        ctx.reply(
            irc_proto::errors::ERR_UNKNOWNCOMMAND,
            vec!["MARKREAD".into(), "Unknown command".into()],
        )?;
        return Ok(());
    }

    // Must be authenticated
    let account_name = match ctx.client.account()? {
        Some(name) => name,
        None => {
            // Send error - need to be logged in
            ctx.reply(
                irc_proto::errors::ERR_NOTREGISTERED,
                vec!["MARKREAD".into(), "You must be logged in to use MARKREAD".into()],
            )?;
            return Ok(());
        }
    };

    // Look up account ID
    let db = ctx.state.db.as_ref().ok_or(crate::error::Error::ServicesUnavailable)?;
    let conn = db.connection()?;
    let account = match accounts::find_by_name(&conn, &account_name)? {
        Some(acc) => acc,
        None => {
            // This shouldn't happen if they're logged in, but handle it
            ctx.reply(
                irc_proto::errors::ERR_NOTREGISTERED,
                vec!["MARKREAD".into(), "Account not found".into()],
            )?;
            return Ok(());
        }
    };

    match timestamp_param {
        Some(ts_str) => {
            // Set read position
            // Parse timestamp= parameter
            let timestamp_value = if let Some(ts) = ts_str.strip_prefix("timestamp=") {
                ts
            } else {
                ts_str
            };

            let timestamp = parse_timestamp(timestamp_value)?;

            read_markers::set(&conn, account.id, target, timestamp)?;

            // Send confirmation
            send_markread_response(ctx, target, timestamp)?;

            tracing::debug!(
                account = %account_name,
                target = %target,
                timestamp = %timestamp,
                "Read marker set"
            );
        }
        None => {
            // Query read position
            if let Some(marker) = read_markers::get(&conn, account.id, target)? {
                send_markread_response(ctx, target, marker.timestamp)?;
            } else {
                // No marker set - send response with * (no timestamp)
                let nick = ctx.client.nickname()?.unwrap_or_else(|| "*".to_string());
                ctx.send_server_message(Command::Markread {
                    target: target.to_string(),
                    timestamp: None,
                })?;
                // Also send a note that there's no marker
                // Format: MARKREAD <target> (no timestamp parameter means no marker)
                let _ = nick; // Just to use the variable
            }
        }
    }

    Ok(())
}

/// Parse a timestamp string (ISO 8601 format or Unix timestamp).
fn parse_timestamp(s: &str) -> Result<DateTime<Utc>> {
    // Try ISO 8601 first
    if let Ok(dt) = DateTime::parse_from_rfc3339(s) {
        return Ok(dt.with_timezone(&Utc));
    }

    // Try parsing as Unix timestamp (seconds)
    if let Ok(ts) = s.parse::<i64>() {
        if let Some(dt) = Utc.timestamp_opt(ts, 0).single() {
            return Ok(dt);
        }
    }

    // Try parsing as Unix timestamp (milliseconds)
    if let Ok(ts) = s.parse::<i64>() {
        if let Some(dt) = Utc.timestamp_millis_opt(ts).single() {
            return Ok(dt);
        }
    }

    // Default to current time
    Ok(Utc::now())
}

/// Send MARKREAD response.
fn send_markread_response(ctx: &HandlerContext, target: &str, timestamp: DateTime<Utc>) -> Result<()> {
    let ts_str = format!(
        "timestamp={}",
        timestamp.to_rfc3339_opts(chrono::SecondsFormat::Millis, true)
    );

    ctx.send_server_message(Command::Markread {
        target: target.to_string(),
        timestamp: Some(ts_str),
    })?;

    Ok(())
}
