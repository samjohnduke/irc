//! Operator command handlers (OPER, KILL, WALLOPS, REHASH, RESTART, DIE, KLINE, ZLINE).

use chrono::{Duration, Utc};
use irc_proto::{Command, Message, errors::*, replies::*};

use super::HandlerContext;
use crate::db::bans::{self, BanType, ServerBan};
use crate::error::{Error, Result};
use crate::lock::RwLockExt;
use crate::state::matches_mask;

/// Handle OPER command - obtain operator privileges.
pub fn handle_oper(ctx: &HandlerContext, name: &str, password: &str) -> Result<()> {
    // Find the operator config by name
    let oper_config = ctx.state.config.operators.iter().find(|op| op.name == name);

    let oper_config = match oper_config {
        Some(config) => config,
        None => {
            // No such operator name
            ctx.reply(ERR_PASSWDMISMATCH, vec!["Password incorrect".into()])?;
            return Err(Error::PasswordMismatch);
        }
    };

    // Check host mask if configured
    if let Some(ref host_mask) = oper_config.host_mask {
        let client_hostmask = ctx.client.hostmask()?;
        if !matches_mask(host_mask, &client_hostmask) {
            ctx.reply(ERR_NOOPERHOST, vec!["No O-lines for your host".into()])?;
            return Err(Error::NoOperHost);
        }
    }

    // Verify password using argon2
    let password_hash = &oper_config.password_hash;
    let parsed_hash = match argon2::PasswordHash::new(password_hash) {
        Ok(hash) => hash,
        Err(_) => {
            tracing::error!(oper_name = %name, "Invalid password hash in config");
            ctx.reply(ERR_PASSWDMISMATCH, vec!["Password incorrect".into()])?;
            return Err(Error::PasswordMismatch);
        }
    };

    use argon2::PasswordVerifier;
    let argon2 = argon2::Argon2::default();
    if argon2
        .verify_password(password.as_bytes(), &parsed_hash)
        .is_err()
    {
        ctx.reply(ERR_PASSWDMISMATCH, vec!["Password incorrect".into()])?;
        return Err(Error::PasswordMismatch);
    }

    // Set operator mode
    ctx.client.modes.write_lock("modes")?.operator = true;

    // 381 RPL_YOUREOPER
    ctx.reply(RPL_YOUREOPER, vec!["You are now an IRC operator".into()])?;

    // Send MODE +o to the client
    let nick = ctx.client.nickname()?.unwrap_or_default();
    let mode_msg = Message::with_prefix(
        ctx.server_prefix(),
        Command::Mode {
            target: nick,
            modes: Some("+o".into()),
            params: vec![],
        },
    );
    ctx.client.send(mode_msg)?;

    tracing::info!(
        client_id = %ctx.client.id,
        oper_name = %name,
        "Client obtained operator privileges"
    );

    Ok(())
}

/// Handle KILL command - disconnect a user.
pub fn handle_kill(ctx: &HandlerContext, nickname: &str, comment: &str) -> Result<()> {
    // Check if caller is an operator
    if !ctx.client.modes.read_lock("modes")?.operator {
        ctx.reply(
            ERR_NOPRIVILEGES,
            vec!["Permission Denied- You're not an IRC operator".into()],
        )?;
        return Err(Error::NoPrivileges);
    }

    // Find target user
    let target = match ctx.state.find_client_by_nick(nickname) {
        Some(t) => t,
        None => {
            ctx.reply(
                ERR_NOSUCHNICK,
                vec![nickname.to_string(), "No such nick/channel".into()],
            )?;
            return Err(Error::NoSuchNick(nickname.to_string()));
        }
    };

    let killer_nick = ctx
        .client
        .nickname()?
        .unwrap_or_else(|| "operator".to_string());
    let target_nick = target.nickname()?.unwrap_or_default();
    let kill_path = format!("{} ({})", ctx.state.config.server_name, comment);

    tracing::info!(
        killer = %killer_nick,
        target = %target_nick,
        comment = %comment,
        "KILL command executed"
    );

    // Send KILL message to target
    let kill_msg = Message::with_prefix(
        ctx.client.prefix()?,
        Command::Kill {
            nickname: target_nick.clone(),
            comment: kill_path.clone(),
        },
    );
    let _ = target.send(kill_msg);

    // Send ERROR to target
    let error_msg = Message::new(Command::Unknown {
        command: "ERROR".into(),
        params: vec![format!(
            "Closing Link: {} (Killed ({}: {}))",
            target.hostname()?,
            killer_nick,
            comment
        )],
    });
    let _ = target.send(error_msg);

    // Broadcast QUIT to all users sharing channels with the target
    let quit_msg = Message::with_prefix(
        target.prefix()?,
        Command::Quit {
            message: Some(format!("Killed ({}: {})", killer_nick, comment)),
        },
    );

    let common_members = ctx.state.get_common_channel_members(target.id)?;
    for member_id in common_members {
        if let Some(member) = ctx.state.clients.get(&member_id) {
            let _ = member.send(quit_msg.clone());
        }
    }

    // Remove target from all channels
    ctx.state.remove_client_from_all_channels(target.id)?;

    // The target's connection will be closed when they receive the ERROR message
    // The connection handler will clean up the client from the server state

    Ok(())
}

/// Handle WALLOPS command - send message to operators.
pub fn handle_wallops(ctx: &HandlerContext, message: &str) -> Result<()> {
    // Check if caller is an operator
    if !ctx.client.modes.read_lock("modes")?.operator {
        ctx.reply(
            ERR_NOPRIVILEGES,
            vec!["Permission Denied- You're not an IRC operator".into()],
        )?;
        return Err(Error::NoPrivileges);
    }

    // Build WALLOPS message
    let wallops_msg = Message::with_prefix(
        ctx.client.prefix()?,
        Command::Wallops {
            message: message.to_string(),
        },
    );

    // Send to all clients with +w mode
    for entry in ctx.state.clients.iter() {
        let client = entry.value();

        // Only send to registered clients with wallops mode enabled
        if client.is_registered()? && client.modes.read_lock("modes")?.wallops {
            let _ = client.send(wallops_msg.clone());
        }
    }

    tracing::debug!(
        sender = ?ctx.client.nickname()?,
        message = %message,
        "WALLOPS sent"
    );

    Ok(())
}

// ========================================
// Server Admin Commands
// ========================================

/// Check if the caller is an operator.
fn require_oper(ctx: &HandlerContext) -> Result<()> {
    if !ctx.client.modes.read_lock("modes")?.operator {
        ctx.reply(
            ERR_NOPRIVILEGES,
            vec!["Permission Denied- You're not an IRC operator".into()],
        )?;
        return Err(Error::NoPrivileges);
    }
    Ok(())
}

/// Broadcast a WALLOPS message from the server.
fn broadcast_wallops(ctx: &HandlerContext, message: &str) -> Result<()> {
    let wallops_msg = Message::with_prefix(
        ctx.server_prefix(),
        Command::Wallops {
            message: message.to_string(),
        },
    );

    for entry in ctx.state.clients.iter() {
        let client = entry.value();
        if client.is_registered()? && client.modes.read_lock("modes")?.wallops {
            let _ = client.send(wallops_msg.clone());
        }
    }
    Ok(())
}

/// Handle REHASH - reload server configuration.
pub fn handle_rehash(ctx: &HandlerContext) -> Result<()> {
    require_oper(ctx)?;

    let nick = ctx.client.nickname()?.unwrap_or_default();
    tracing::info!(oper = %nick, "REHASH requested");

    // Send notice to the operator
    let notice = Message::with_prefix(
        ctx.server_prefix(),
        Command::Notice {
            target: nick.clone(),
            message: "Rehashing server configuration...".into(),
        },
    );
    ctx.client.send(notice)?;

    // Signal config reload
    ctx.state.request_rehash();

    Ok(())
}

/// Handle RESTART - restart the server.
pub fn handle_restart(ctx: &HandlerContext) -> Result<()> {
    require_oper(ctx)?;

    let nick = ctx.client.nickname()?.unwrap_or_default();
    tracing::warn!(oper = %nick, "RESTART command received");

    // Broadcast to operators
    broadcast_wallops(ctx, &format!("Server restarting by {} (RESTART)", nick))?;

    // Signal restart
    ctx.state.request_restart();

    Ok(())
}

/// Handle DIE - shut down the server.
pub fn handle_die(ctx: &HandlerContext) -> Result<()> {
    require_oper(ctx)?;

    let nick = ctx.client.nickname()?.unwrap_or_default();
    tracing::warn!(oper = %nick, "DIE command received");

    // Broadcast to operators
    broadcast_wallops(ctx, &format!("Server shutting down by {} (DIE)", nick))?;

    // Signal shutdown
    ctx.state.request_shutdown();

    Ok(())
}

// ========================================
// Server Ban Commands (KLINE/ZLINE)
// ========================================

/// Handle KLINE - ban by user@host mask.
pub fn handle_kline(
    ctx: &HandlerContext,
    duration: Option<&str>,
    mask: &str,
    reason: Option<&str>,
) -> Result<()> {
    require_oper(ctx)?;

    if mask.is_empty() {
        ctx.reply(
            ERR_NEEDMOREPARAMS,
            vec!["KLINE".into(), "Not enough parameters".into()],
        )?;
        return Ok(());
    }

    let expires_at = duration
        .and_then(bans::parse_duration)
        .map(|secs| Utc::now() + Duration::seconds(secs));

    let set_by = ctx.client.nickname()?.unwrap_or_else(|| "unknown".into());

    let ban = ServerBan::new(
        BanType::Kline,
        mask.to_string(),
        reason.map(String::from),
        set_by.clone(),
        expires_at,
    );

    // Add to cache
    ctx.state.add_kline(ban.clone());

    // Persist to database if available
    if let Some(ref db) = ctx.state.db
        && let Err(e) = bans::add_ban(&db.connection()?, &ban)
    {
        tracing::warn!(error = %e, "Failed to persist K-line to database");
    }

    // Notify the operator
    let notice_msg = if let Some(exp) = expires_at {
        format!(
            "K-Line added: {} (expires {})",
            mask,
            exp.format("%Y-%m-%d %H:%M:%S UTC")
        )
    } else {
        format!("K-Line added: {} (permanent)", mask)
    };

    let notice = Message::with_prefix(
        ctx.server_prefix(),
        Command::Notice {
            target: set_by,
            message: notice_msg,
        },
    );
    ctx.client.send(notice)?;

    tracing::info!(
        oper = ?ctx.client.nickname()?,
        mask = %mask,
        reason = ?reason,
        expires = ?expires_at,
        "K-Line added"
    );

    // Disconnect matching users
    disconnect_matching_kline(ctx, mask)?;

    Ok(())
}

/// Handle UNKLINE - remove a K-line.
pub fn handle_unkline(ctx: &HandlerContext, mask: &str) -> Result<()> {
    require_oper(ctx)?;

    if mask.is_empty() {
        ctx.reply(
            ERR_NEEDMOREPARAMS,
            vec!["UNKLINE".into(), "Not enough parameters".into()],
        )?;
        return Ok(());
    }

    let removed = ctx.state.remove_kline(mask);

    // Remove from database if available
    if let Some(ref db) = ctx.state.db
        && let Err(e) = bans::remove_ban(&db.connection()?, BanType::Kline, mask)
    {
        tracing::warn!(error = %e, "Failed to remove K-line from database");
    }

    let nick = ctx.client.nickname()?.unwrap_or_default();
    let msg = if removed {
        format!("K-Line removed: {}", mask)
    } else {
        format!("No K-Line found for: {}", mask)
    };

    let notice = Message::with_prefix(
        ctx.server_prefix(),
        Command::Notice {
            target: nick,
            message: msg,
        },
    );
    ctx.client.send(notice)?;

    Ok(())
}

/// Handle GLINE - alias for KLINE on single-server.
pub fn handle_gline(
    ctx: &HandlerContext,
    duration: Option<&str>,
    mask: &str,
    reason: Option<&str>,
) -> Result<()> {
    handle_kline(ctx, duration, mask, reason)
}

/// Handle UNGLINE - alias for UNKLINE.
pub fn handle_ungline(ctx: &HandlerContext, mask: &str) -> Result<()> {
    handle_unkline(ctx, mask)
}

/// Handle ZLINE - ban by IP.
pub fn handle_zline(
    ctx: &HandlerContext,
    duration: Option<&str>,
    mask: &str,
    reason: Option<&str>,
) -> Result<()> {
    require_oper(ctx)?;

    if mask.is_empty() {
        ctx.reply(
            ERR_NEEDMOREPARAMS,
            vec!["ZLINE".into(), "Not enough parameters".into()],
        )?;
        return Ok(());
    }

    let expires_at = duration
        .and_then(bans::parse_duration)
        .map(|secs| Utc::now() + Duration::seconds(secs));

    let set_by = ctx.client.nickname()?.unwrap_or_else(|| "unknown".into());

    let ban = ServerBan::new(
        BanType::Zline,
        mask.to_string(),
        reason.map(String::from),
        set_by.clone(),
        expires_at,
    );

    // Add to cache
    ctx.state.add_zline(ban.clone());

    // Persist to database if available
    if let Some(ref db) = ctx.state.db
        && let Err(e) = bans::add_ban(&db.connection()?, &ban)
    {
        tracing::warn!(error = %e, "Failed to persist Z-line to database");
    }

    // Notify the operator
    let notice_msg = if let Some(exp) = expires_at {
        format!(
            "Z-Line added: {} (expires {})",
            mask,
            exp.format("%Y-%m-%d %H:%M:%S UTC")
        )
    } else {
        format!("Z-Line added: {} (permanent)", mask)
    };

    let notice = Message::with_prefix(
        ctx.server_prefix(),
        Command::Notice {
            target: set_by,
            message: notice_msg,
        },
    );
    ctx.client.send(notice)?;

    tracing::info!(
        oper = ?ctx.client.nickname()?,
        mask = %mask,
        reason = ?reason,
        expires = ?expires_at,
        "Z-Line added"
    );

    // Disconnect matching users
    disconnect_matching_zline(ctx, mask)?;

    Ok(())
}

/// Handle UNZLINE - remove a Z-line.
pub fn handle_unzline(ctx: &HandlerContext, mask: &str) -> Result<()> {
    require_oper(ctx)?;

    if mask.is_empty() {
        ctx.reply(
            ERR_NEEDMOREPARAMS,
            vec!["UNZLINE".into(), "Not enough parameters".into()],
        )?;
        return Ok(());
    }

    let removed = ctx.state.remove_zline(mask);

    // Remove from database if available
    if let Some(ref db) = ctx.state.db
        && let Err(e) = bans::remove_ban(&db.connection()?, BanType::Zline, mask)
    {
        tracing::warn!(error = %e, "Failed to remove Z-line from database");
    }

    let nick = ctx.client.nickname()?.unwrap_or_default();
    let msg = if removed {
        format!("Z-Line removed: {}", mask)
    } else {
        format!("No Z-Line found for: {}", mask)
    };

    let notice = Message::with_prefix(
        ctx.server_prefix(),
        Command::Notice {
            target: nick,
            message: msg,
        },
    );
    ctx.client.send(notice)?;

    Ok(())
}

/// Disconnect all users matching a K-line mask.
fn disconnect_matching_kline(ctx: &HandlerContext, mask: &str) -> Result<()> {
    let mut to_disconnect = Vec::new();

    for entry in ctx.state.clients.iter() {
        let client = entry.value();
        if let Ok(hostmask) = client.hostmask()
            && matches_mask(mask, &hostmask)
        {
            to_disconnect.push(client.id);
        }
    }

    for client_id in to_disconnect {
        if let Some(client) = ctx.state.clients.get(&client_id) {
            // Send error message
            let error_msg = Message::new(Command::Unknown {
                command: "ERROR".into(),
                params: vec![format!("Closing Link: K-Lined ({})", mask)],
            });
            let _ = client.send(error_msg);
        }
    }

    Ok(())
}

/// Disconnect all users matching a Z-line mask.
fn disconnect_matching_zline(ctx: &HandlerContext, mask: &str) -> Result<()> {
    let mut to_disconnect = Vec::new();

    for entry in ctx.state.clients.iter() {
        let client = entry.value();
        let ip = client.addr.ip().to_string();
        if matches_mask(mask, &ip) {
            to_disconnect.push(client.id);
        }
    }

    for client_id in to_disconnect {
        if let Some(client) = ctx.state.clients.get(&client_id) {
            // Send error message
            let error_msg = Message::new(Command::Unknown {
                command: "ERROR".into(),
                params: vec![format!("Closing Link: Z-Lined ({})", mask)],
            });
            let _ = client.send(error_msg);
        }
    }

    Ok(())
}
