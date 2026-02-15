//! MONITOR command handler.
//!
//! Implements the IRCv3 MONITOR extension for tracking user online/offline status.

use irc_proto::errors::ERR_MONLISTFULL;
use irc_proto::replies::*;

use super::HandlerContext;
use crate::error::Result;

/// Handle MONITOR command.
///
/// Subcommands:
/// - MONITOR + targets - Add to monitor list
/// - MONITOR - targets - Remove from monitor list
/// - MONITOR C - Clear monitor list
/// - MONITOR L - List monitored nicks
/// - MONITOR S - Show status of monitored nicks
pub fn handle_monitor(ctx: &HandlerContext, subcommand: char, targets: Option<&str>) -> Result<()> {
    match subcommand.to_ascii_uppercase() {
        '+' => monitor_add(ctx, targets.unwrap_or("")),
        '-' => monitor_remove(ctx, targets.unwrap_or("")),
        'C' => monitor_clear(ctx),
        'L' => monitor_list(ctx),
        'S' => monitor_status(ctx),
        _ => {
            // Unknown subcommand - just ignore
            Ok(())
        }
    }
}

/// Add nicknames to the monitor list.
fn monitor_add(ctx: &HandlerContext, targets: &str) -> Result<()> {
    let nicks: Vec<&str> = targets
        .split(',')
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .collect();

    if nicks.is_empty() {
        return Ok(());
    }

    // Check limit
    let current = ctx.client.monitor_count()?;
    let max = ctx.state.config.limits.max_monitor;

    if current + nicks.len() > max {
        ctx.reply(
            ERR_MONLISTFULL,
            vec![
                max.to_string(),
                targets.to_string(),
                "Monitor list is full".into(),
            ],
        )?;
        return Ok(());
    }

    let added = ctx.client.monitor_add(&nicks)?;

    // Partition by online/offline status
    let (online, offline) = partition_by_online(ctx, &added)?;

    // Send RPL_MONONLINE for online users
    if !online.is_empty() {
        ctx.reply(RPL_MONONLINE, vec![online.join(",")])?;
    }

    // Send RPL_MONOFFLINE for offline users
    if !offline.is_empty() {
        ctx.reply(RPL_MONOFFLINE, vec![offline.join(",")])?;
    }

    Ok(())
}

/// Remove nicknames from the monitor list.
fn monitor_remove(ctx: &HandlerContext, targets: &str) -> Result<()> {
    let nicks: Vec<&str> = targets
        .split(',')
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .collect();

    ctx.client.monitor_remove(&nicks)?;

    Ok(())
}

/// Clear the monitor list.
fn monitor_clear(ctx: &HandlerContext) -> Result<()> {
    ctx.client.monitor_clear()?;
    Ok(())
}

/// List all monitored nicknames.
fn monitor_list(ctx: &HandlerContext) -> Result<()> {
    let nicks = ctx.client.monitor_list()?;

    // Send in batches to avoid line length limits
    for chunk in nicks.chunks(20) {
        ctx.reply(RPL_MONLIST, vec![chunk.join(",")])?;
    }

    ctx.reply(RPL_ENDOFMONLIST, vec!["End of MONITOR list".into()])?;

    Ok(())
}

/// Show status of all monitored nicknames.
fn monitor_status(ctx: &HandlerContext) -> Result<()> {
    let nicks = ctx.client.monitor_list()?;
    let refs: Vec<&str> = nicks.iter().map(|s| s.as_str()).collect();
    let (online, offline) = partition_by_online(ctx, &refs)?;

    // Send RPL_MONONLINE for online users
    if !online.is_empty() {
        ctx.reply(RPL_MONONLINE, vec![online.join(",")])?;
    }

    // Send RPL_MONOFFLINE for offline users
    if !offline.is_empty() {
        ctx.reply(RPL_MONOFFLINE, vec![offline.join(",")])?;
    }

    Ok(())
}

/// Partition nicknames into online and offline lists.
/// For online users, returns nick!user@host format.
fn partition_by_online<S: AsRef<str>>(
    ctx: &HandlerContext,
    nicks: &[S],
) -> Result<(Vec<String>, Vec<String>)> {
    let mut online = Vec::new();
    let mut offline = Vec::new();

    for nick in nicks {
        let nick = nick.as_ref();
        if let Some(client) = ctx.state.find_client_by_nick(nick) {
            // Format: nick!user@host
            let hostmask = client.hostmask()?;
            online.push(hostmask);
        } else {
            offline.push(nick.to_string());
        }
    }

    Ok((online, offline))
}

/// Broadcast MONONLINE to all clients monitoring a nickname.
/// Called when a user registers or changes nick to this nick.
#[allow(dead_code)]
pub fn broadcast_monitor_online(ctx: &HandlerContext) -> Result<()> {
    let nick = match ctx.client.nickname()? {
        Some(n) => n,
        None => return Ok(()),
    };

    let hostmask = ctx.client.hostmask()?;
    let monitors = ctx.state.get_monitors_for_nick(&nick)?;

    for monitor in monitors {
        monitor.send_numeric(
            &ctx.state.config.server_name,
            RPL_MONONLINE,
            vec![hostmask.clone()],
        )?;
    }

    Ok(())
}

/// Broadcast MONOFFLINE to all clients monitoring a nickname.
/// Called when a user quits or changes nick away from this nick.
#[allow(dead_code)]
pub fn broadcast_monitor_offline(ctx: &HandlerContext, nick: &str) -> Result<()> {
    let monitors = ctx.state.get_monitors_for_nick(nick)?;

    for monitor in monitors {
        monitor.send_numeric(
            &ctx.state.config.server_name,
            RPL_MONOFFLINE,
            vec![nick.to_string()],
        )?;
    }

    Ok(())
}
