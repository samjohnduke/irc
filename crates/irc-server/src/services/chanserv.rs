//! ChanServ service for channel registration and access control.

use irc_proto::{Command, Message};

use super::ServiceContext;
use crate::db::{accounts, channels};
use crate::error::Result;
use crate::lock::RwLockExt;

/// Handle a ChanServ command.
pub fn handle_command(sctx: &ServiceContext, args: &[&str]) -> Result<()> {
    match args.first().map(|s| s.to_uppercase()).as_deref() {
        Some("HELP") => cmd_help(sctx),
        Some("REGISTER") => cmd_register(sctx, &args[1..]),
        Some("DROP") => cmd_drop(sctx, &args[1..]),
        Some("INFO") => cmd_info(sctx, &args[1..]),
        Some("OP") => cmd_op(sctx, &args[1..]),
        Some("DEOP") => cmd_deop(sctx, &args[1..]),
        Some("VOICE") => cmd_voice(sctx, &args[1..]),
        Some("DEVOICE") => cmd_devoice(sctx, &args[1..]),
        Some("FLAGS") => cmd_flags(sctx, &args[1..]),
        Some(cmd) => {
            sctx.error(&format!(
                "Unknown command: {}. Use HELP for a list of commands.",
                cmd
            ))?;
            Ok(())
        }
        None => cmd_help(sctx),
    }
}

/// Show help message.
fn cmd_help(sctx: &ServiceContext) -> Result<()> {
    sctx.reply("***** ChanServ Help *****")?;
    sctx.reply(" ")?;
    sctx.reply("ChanServ allows you to register and manage channels.")?;
    sctx.reply(" ")?;
    sctx.reply("Commands:")?;
    sctx.reply("  REGISTER #channel         - Register a channel (must be op)")?;
    sctx.reply("  DROP #channel             - Unregister a channel (founder only)")?;
    sctx.reply("  INFO #channel             - Show channel registration info")?;
    sctx.reply("  OP #channel [nick]        - Give operator status")?;
    sctx.reply("  DEOP #channel [nick]      - Remove operator status")?;
    sctx.reply("  VOICE #channel [nick]     - Give voice status")?;
    sctx.reply("  DEVOICE #channel [nick]   - Remove voice status")?;
    sctx.reply("  FLAGS #channel [account [+/-flags]]")?;
    sctx.reply("                            - View or modify channel access")?;
    sctx.reply(" ")?;
    sctx.reply("Access flags:")?;
    sctx.reply("  +v  Auto-voice on join")?;
    sctx.reply("  +o  Auto-op on join")?;
    sctx.reply("  +r  Can kick/ban users")?;
    sctx.reply("  +f  Can modify FLAGS")?;
    sctx.reply("  +F  Founder (full access)")?;
    sctx.reply(" ")?;
    sctx.reply("***** End of Help *****")?;
    Ok(())
}

/// Register a channel.
fn cmd_register(sctx: &ServiceContext, args: &[&str]) -> Result<()> {
    let db = sctx.require_db()?;
    let conn = db.connection()?;
    let account_name = sctx.require_account()?;

    // Get channel name
    let channel_name = match args.first() {
        Some(c) if irc_proto::is_channel(c) => *c,
        Some(c) => {
            sctx.error(&format!("{} is not a valid channel name.", c))?;
            return Ok(());
        }
        None => {
            sctx.error("Usage: REGISTER #channel")?;
            return Ok(());
        }
    };

    // Check if already registered
    if channels::is_registered(&conn, channel_name)? {
        sctx.error(&format!("{} is already registered.", channel_name))?;
        return Ok(());
    }

    // Check if channel exists and user is op
    let channel_arc = match sctx.ctx.state.get_channel(channel_name) {
        Some(c) => c,
        None => {
            sctx.error(&format!("You must be in {} to register it.", channel_name))?;
            return Ok(());
        }
    };

    {
        let channel = channel_arc.read_lock("channel")?;
        if !channel.is_member(sctx.ctx.client.id) {
            sctx.error(&format!("You must be in {} to register it.", channel_name))?;
            return Ok(());
        }
        if !channel.is_operator(sctx.ctx.client.id) {
            sctx.error(&format!(
                "You must be a channel operator in {} to register it.",
                channel_name
            ))?;
            return Ok(());
        }
    }

    // Get account ID
    let account = match accounts::find_by_name(&conn, &account_name)? {
        Some(acc) => acc,
        None => {
            sctx.error("Internal error: account not found.")?;
            return Ok(());
        }
    };

    // Register the channel
    channels::register(&conn, channel_name, account.id)?;

    sctx.reply(&format!(
        "Channel {} has been registered to your account.",
        channel_name
    ))?;

    tracing::info!(
        channel = %channel_name,
        founder = %account_name,
        "Channel registered via ChanServ"
    );

    Ok(())
}

/// Drop (unregister) a channel.
fn cmd_drop(sctx: &ServiceContext, args: &[&str]) -> Result<()> {
    let db = sctx.require_db()?;
    let conn = db.connection()?;
    let account_name = sctx.require_account()?;

    // Get channel name
    let channel_name = match args.first() {
        Some(c) if irc_proto::is_channel(c) => *c,
        _ => {
            sctx.error("Usage: DROP #channel")?;
            return Ok(());
        }
    };

    // Check if registered
    let reg_channel = match channels::find(&conn, channel_name)? {
        Some(c) => c,
        None => {
            sctx.error(&format!("{} is not registered.", channel_name))?;
            return Ok(());
        }
    };

    // Check if user is the founder
    let account = match accounts::find_by_name(&conn, &account_name)? {
        Some(acc) => acc,
        None => {
            sctx.error("Internal error: account not found.")?;
            return Ok(());
        }
    };

    if reg_channel.founder_account_id != account.id {
        // Check if user has +F flag
        if !channels::has_flag(&conn, channel_name, &account_name, 'F')? {
            sctx.error("You must be the founder to drop this channel.")?;
            return Ok(());
        }
    }

    // Unregister
    channels::unregister(&conn, channel_name)?;

    sctx.reply(&format!("Channel {} has been dropped.", channel_name))?;

    tracing::info!(
        channel = %channel_name,
        by = %account_name,
        "Channel dropped via ChanServ"
    );

    Ok(())
}

/// Show channel info.
fn cmd_info(sctx: &ServiceContext, args: &[&str]) -> Result<()> {
    let db = sctx.require_db()?;
    let conn = db.connection()?;

    // Get channel name
    let channel_name = match args.first() {
        Some(c) if irc_proto::is_channel(c) => *c,
        _ => {
            sctx.error("Usage: INFO #channel")?;
            return Ok(());
        }
    };

    // Check if registered
    let reg_channel = match channels::find(&conn, channel_name)? {
        Some(c) => c,
        None => {
            sctx.reply(&format!("{} is not registered.", channel_name))?;
            return Ok(());
        }
    };

    // Get founder name
    let founder =
        channels::get_founder(&conn, channel_name)?.unwrap_or_else(|| "(unknown)".to_string());

    sctx.reply(&format!("***** ChanServ Info for {} *****", channel_name))?;
    sctx.reply(&format!("Founder:      {}", founder))?;
    sctx.reply(&format!(
        "Registered:   {}",
        reg_channel.registered_at.format("%Y-%m-%d %H:%M:%S UTC")
    ))?;

    if let Some(topic) = &reg_channel.topic {
        sctx.reply(&format!("Topic:        {}", topic))?;
    }

    // Check if channel is currently active
    if let Some(channel_arc) = sctx.ctx.state.get_channel(channel_name) {
        let channel = channel_arc.read_lock("channel")?;
        sctx.reply(&format!(
            "Status:       Active ({} users)",
            channel.member_count()
        ))?;
    } else {
        sctx.reply("Status:       Inactive")?;
    }

    sctx.reply("***** End of Info *****")?;

    Ok(())
}

/// Give operator status.
fn cmd_op(sctx: &ServiceContext, args: &[&str]) -> Result<()> {
    set_mode(sctx, args, "+o", "OP")
}

/// Remove operator status.
fn cmd_deop(sctx: &ServiceContext, args: &[&str]) -> Result<()> {
    set_mode(sctx, args, "-o", "DEOP")
}

/// Give voice status.
fn cmd_voice(sctx: &ServiceContext, args: &[&str]) -> Result<()> {
    set_mode(sctx, args, "+v", "VOICE")
}

/// Remove voice status.
fn cmd_devoice(sctx: &ServiceContext, args: &[&str]) -> Result<()> {
    set_mode(sctx, args, "-v", "DEVOICE")
}

/// Set a mode on a user via ChanServ.
fn set_mode(sctx: &ServiceContext, args: &[&str], mode: &str, cmd_name: &str) -> Result<()> {
    let db = sctx.require_db()?;
    let conn = db.connection()?;
    let account_name = sctx.require_account()?;

    // Parse arguments
    let (channel_name, target_nick) = match args.len() {
        1 => (args[0], sctx.nickname()?),
        2 => (args[0], args[1].to_string()),
        _ => {
            sctx.error(&format!("Usage: {} #channel [nick]", cmd_name))?;
            return Ok(());
        }
    };

    if !irc_proto::is_channel(channel_name) {
        sctx.error(&format!("{} is not a valid channel name.", channel_name))?;
        return Ok(());
    }

    // Check if channel is registered
    if !channels::is_registered(&conn, channel_name)? {
        sctx.error(&format!("{} is not registered.", channel_name))?;
        return Ok(());
    }

    // Check if user has appropriate flags
    let has_access = channels::has_flag(&conn, channel_name, &account_name, 'o')?
        || channels::has_flag(&conn, channel_name, &account_name, 'F')?;

    if !has_access {
        sctx.error(&format!(
            "You do not have access to use {} on {}.",
            cmd_name, channel_name
        ))?;
        return Ok(());
    }

    // Get the channel
    let channel_arc = match sctx.ctx.state.get_channel(channel_name) {
        Some(c) => c,
        None => {
            sctx.error(&format!("{} is not active.", channel_name))?;
            return Ok(());
        }
    };

    // Find target
    let target = match sctx.ctx.state.find_client_by_nick(&target_nick) {
        Some(c) => c,
        None => {
            sctx.error(&format!("{} is not online.", target_nick))?;
            return Ok(());
        }
    };

    // Check if target is in channel
    {
        let mut channel = channel_arc.write_lock("channel")?;
        let status = match channel.get_member_status_mut(target.id) {
            Some(s) => s,
            None => {
                sctx.error(&format!("{} is not in {}.", target_nick, channel_name))?;
                return Ok(());
            }
        };

        // Apply mode
        let adding = mode.starts_with('+');
        if mode.contains('o') {
            status.operator = adding;
        }
        if mode.contains('v') {
            status.voice = adding;
        }
    }

    // Broadcast mode change from ChanServ
    let chanserv_prefix = irc_proto::Prefix::from_user(
        "ChanServ".to_string(),
        "service".to_string(),
        sctx.ctx.server_name().to_string(),
    );

    let mode_msg = Message::with_prefix(
        chanserv_prefix,
        Command::Mode {
            target: channel_name.to_string(),
            modes: Some(format!("{} {}", mode, target_nick)),
            params: vec![],
        },
    );

    let channel = channel_arc.read_lock("channel")?;
    sctx.ctx
        .state
        .broadcast_to_channel(&channel, mode_msg, None);

    sctx.reply(&format!(
        "Mode {} {} set on {}.",
        mode, target_nick, channel_name
    ))?;

    Ok(())
}

/// View or modify channel access flags.
fn cmd_flags(sctx: &ServiceContext, args: &[&str]) -> Result<()> {
    let db = sctx.require_db()?;
    let conn = db.connection()?;

    // Get channel name
    let channel_name = match args.first() {
        Some(c) if irc_proto::is_channel(c) => *c,
        _ => {
            sctx.error("Usage: FLAGS #channel [account [+/-flags]]")?;
            return Ok(());
        }
    };

    // Check if registered
    let reg_channel = match channels::find(&conn, channel_name)? {
        Some(c) => c,
        None => {
            sctx.error(&format!("{} is not registered.", channel_name))?;
            return Ok(());
        }
    };

    match args.len() {
        1 => {
            // List all access
            let access_list = channels::get_all_access(&conn, reg_channel.id)?;

            sctx.reply(&format!("***** {} Access List *****", channel_name))?;
            if access_list.is_empty() {
                sctx.reply("No access entries.")?;
            } else {
                for (account, flags) in access_list {
                    sctx.reply(&format!("  {}: {}", account, flags))?;
                }
            }
            sctx.reply("***** End of Access List *****")?;
        }
        2 => {
            // Show access for specific account
            let target_account = args[1];

            if let Some(flags) = channels::get_user_flags(&conn, channel_name, target_account)? {
                sctx.reply(&format!(
                    "{} has flags {} on {}.",
                    target_account, flags, channel_name
                ))?;
            } else {
                sctx.reply(&format!(
                    "{} has no access on {}.",
                    target_account, channel_name
                ))?;
            }
        }
        3 => {
            // Modify access
            let account_name = sctx.require_account()?;

            // Check if user has +f or +F flag
            let can_modify = channels::has_flag(&conn, channel_name, &account_name, 'f')?
                || channels::has_flag(&conn, channel_name, &account_name, 'F')?;

            if !can_modify {
                sctx.error("You do not have access to modify flags on this channel.")?;
                return Ok(());
            }

            let target_account = args[1];
            let flag_changes = args[2];

            // Get target account ID
            let target = match accounts::find_by_name(&conn, target_account)? {
                Some(acc) => acc,
                None => {
                    sctx.error(&format!("Account {} does not exist.", target_account))?;
                    return Ok(());
                }
            };

            // Get current flags
            let current_flags =
                channels::get_user_flags(&conn, channel_name, target_account)?.unwrap_or_default();

            // Apply changes
            let new_flags = apply_flag_changes(&current_flags, flag_changes);

            if new_flags.is_empty() || new_flags == "+" {
                // Remove access entirely
                channels::remove_access(&conn, reg_channel.id, target.id)?;
                sctx.reply(&format!(
                    "Removed all access for {} on {}.",
                    target_account, channel_name
                ))?;
            } else {
                channels::set_access(&conn, reg_channel.id, target.id, &new_flags)?;
                sctx.reply(&format!(
                    "Set flags for {} on {} to {}.",
                    target_account, channel_name, new_flags
                ))?;
            }

            tracing::info!(
                channel = %channel_name,
                target = %target_account,
                flags = %new_flags,
                by = %account_name,
                "Channel flags modified via ChanServ"
            );
        }
        _ => {
            sctx.error("Usage: FLAGS #channel [account [+/-flags]]")?;
        }
    }

    Ok(())
}

/// Apply flag changes to a flag string.
///
/// Examples:
/// - current="+vo", changes="+f" -> "+vof"
/// - current="+vof", changes="-o" -> "+vf"
/// - current="", changes="+v" -> "+v"
fn apply_flag_changes(current: &str, changes: &str) -> String {
    // Remove the leading + from current flags
    let mut flags: Vec<char> = current.chars().filter(|c| c.is_alphabetic()).collect();

    let mut adding = true;
    for c in changes.chars() {
        match c {
            '+' => adding = true,
            '-' => adding = false,
            flag if flag.is_alphabetic() => {
                if adding {
                    if !flags.contains(&flag) {
                        flags.push(flag);
                    }
                } else {
                    flags.retain(|&f| f != flag);
                }
            }
            _ => {}
        }
    }

    if flags.is_empty() {
        String::new()
    } else {
        format!("+{}", flags.iter().collect::<String>())
    }
}

/// Apply auto-modes when a user joins a registered channel.
///
/// This should be called after a user successfully joins a channel.
pub fn apply_auto_modes(ctx: &crate::handler::HandlerContext, channel_name: &str) -> Result<()> {
    let db = match ctx.state.db.as_ref() {
        Some(db) => db,
        None => return Ok(()),
    };

    let conn = db.connection()?;

    // Get user's account
    let account_name = match ctx.client.account()? {
        Some(acc) => acc,
        None => return Ok(()),
    };

    // Get flags for this user on this channel
    let flags = match channels::get_user_flags(&conn, channel_name, &account_name)? {
        Some(f) => f,
        None => return Ok(()),
    };

    // Determine which modes to apply
    let mut modes_to_apply = Vec::new();
    let nick = ctx.client.nickname()?.unwrap_or_default();

    if flags.contains('o') || flags.contains('F') {
        modes_to_apply.push(("+o", nick.clone()));
    }
    if flags.contains('v') {
        modes_to_apply.push(("+v", nick.clone()));
    }

    if modes_to_apply.is_empty() {
        return Ok(());
    }

    // Get channel and apply modes
    let channel_arc = match ctx.state.get_channel(channel_name) {
        Some(c) => c,
        None => return Ok(()),
    };

    {
        let mut channel = channel_arc.write_lock("channel")?;
        if let Some(status) = channel.get_member_status_mut(ctx.client.id) {
            for (mode, _) in &modes_to_apply {
                if *mode == "+o" {
                    status.operator = true;
                } else if *mode == "+v" {
                    status.voice = true;
                }
            }
        }
    }

    // Broadcast mode changes from ChanServ
    let chanserv_prefix = irc_proto::Prefix::from_user(
        "ChanServ".to_string(),
        "service".to_string(),
        ctx.server_name().to_string(),
    );

    for (mode, target) in modes_to_apply {
        let mode_msg = Message::with_prefix(
            chanserv_prefix.clone(),
            Command::Mode {
                target: channel_name.to_string(),
                modes: Some(format!("{} {}", mode, target)),
                params: vec![],
            },
        );

        let channel = channel_arc.read_lock("channel")?;
        ctx.state.broadcast_to_channel(&channel, mode_msg, None);
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_apply_flag_changes() {
        assert_eq!(apply_flag_changes("", "+v"), "+v");
        assert_eq!(apply_flag_changes("+vo", "+f"), "+vof");
        assert_eq!(apply_flag_changes("+vof", "-o"), "+vf");
        assert_eq!(apply_flag_changes("+v", "-v"), "");
        assert_eq!(apply_flag_changes("+voF", "-vo+r"), "+Fr");
    }
}
