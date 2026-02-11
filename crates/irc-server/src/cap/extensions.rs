//! IRCv3 capability extension helpers.
//!
//! This module provides helper functions for broadcasting capability-specific
//! messages to clients that have enabled the relevant capabilities.

use irc_proto::{Command, Message, Prefix};

use crate::error::Result;
use crate::handler::HandlerContext;

/// Broadcast ACCOUNT notification to common channel members.
///
/// This is sent when a user logs in or out of their account.
/// Format: `:nick!user@host ACCOUNT <accountname>` or `:nick!user@host ACCOUNT *`
pub fn broadcast_account_notify(ctx: &HandlerContext, account: Option<&str>) -> Result<()> {
    let account_value = account.unwrap_or("*");

    let msg = Message::with_prefix(
        ctx.client.prefix()?,
        Command::Account {
            account: account_value.to_string(),
        },
    );

    // Get all clients that share channels with this user
    let common_members = ctx.state.get_common_channel_members(ctx.client.id)?;

    for member_id in common_members {
        if let Some(member) = ctx.state.clients.get(&member_id) {
            // Only send to clients that have account-notify enabled
            if member.has_cap("account-notify")? {
                let _ = member.send(msg.clone());
            }
        }
    }

    Ok(())
}

/// Broadcast AWAY notification to common channel members.
///
/// This is sent when a user sets or clears their away status.
/// Format: `:nick!user@host AWAY` or `:nick!user@host AWAY :<message>`
pub fn broadcast_away_notify(ctx: &HandlerContext, message: Option<&str>) -> Result<()> {
    let msg = Message::with_prefix(
        ctx.client.prefix()?,
        Command::Away {
            message: message.map(String::from),
        },
    );

    // Get all clients that share channels with this user
    let common_members = ctx.state.get_common_channel_members(ctx.client.id)?;

    for member_id in common_members {
        if let Some(member) = ctx.state.clients.get(&member_id) {
            // Only send to clients that have away-notify enabled
            if member.has_cap("away-notify")? {
                let _ = member.send(msg.clone());
            }
        }
    }

    Ok(())
}

/// Build a JOIN message with extended-join data.
///
/// For clients with extended-join enabled, include account and realname.
/// Format: `:nick!user@host JOIN #channel <account> :<realname>`
pub fn build_extended_join(
    prefix: Prefix,
    channel: &str,
    account: Option<&str>,
    realname: &str,
) -> Message {
    let account_str = account.unwrap_or("*");
    // For extended-join, we use Unknown to preserve the extra parameters
    // since the standard Join command doesn't support account/realname
    Message::with_prefix(
        prefix,
        Command::Unknown {
            command: "JOIN".to_string(),
            params: vec![
                channel.to_string(),
                account_str.to_string(),
                realname.to_string(),
            ],
        },
    )
}

/// Build a standard JOIN message.
pub fn build_standard_join(prefix: Prefix, channel: &str) -> Message {
    Message::with_prefix(
        prefix,
        Command::Join {
            channels: vec![(channel.to_string(), None)],
        },
    )
}

/// Get the prefix string for a channel member with multi-prefix support.
///
/// If multi_prefix is true, returns all prefixes (e.g., "@+" for op+voice).
/// Otherwise returns only the highest prefix.
pub fn format_member_prefix(is_op: bool, has_voice: bool, multi_prefix: bool) -> String {
    if multi_prefix {
        let mut prefix = String::new();
        if is_op {
            prefix.push('@');
        }
        if has_voice {
            prefix.push('+');
        }
        prefix
    } else if is_op {
        "@".to_string()
    } else if has_voice {
        "+".to_string()
    } else {
        String::new()
    }
}

/// Broadcast SETNAME to common channel members.
///
/// This is sent when a user changes their realname.
/// Format: `:nick!user@host SETNAME :<new realname>`
pub fn broadcast_setname(ctx: &HandlerContext, new_realname: &str) -> Result<()> {
    let msg = Message::with_prefix(
        ctx.client.prefix()?,
        Command::Setname {
            realname: new_realname.to_string(),
        },
    );

    // Get all clients that share channels with this user
    let common_members = ctx.state.get_common_channel_members(ctx.client.id)?;

    for member_id in common_members {
        if let Some(member) = ctx.state.clients.get(&member_id) {
            // Only send to clients that have setname enabled
            if member.has_cap("setname")? {
                let _ = member.send(msg.clone());
            }
        }
    }

    // Also send to self if they have the cap
    if ctx.client.has_cap("setname")? {
        let _ = ctx.client.send(msg);
    }

    Ok(())
}

/// Add account tag to a message if the sender is identified.
///
/// Clients with account-tag enabled will receive the account= tag on messages
/// from identified users.
pub fn add_account_tag(msg: &mut Message, account: Option<&str>) {
    if let Some(account_name) = account {
        let tags = msg.tags.get_or_insert_with(irc_proto::Tags::new);
        tags.set("account", account_name.to_string());
    }
}
