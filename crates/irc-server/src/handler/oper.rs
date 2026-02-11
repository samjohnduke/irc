//! Operator command handlers (OPER, KILL, WALLOPS).

use irc_proto::{errors::*, replies::*, Command, Message};

use super::HandlerContext;
use crate::error::{Error, Result};
use crate::lock::RwLockExt;
use crate::state::matches_mask;

/// Handle OPER command - obtain operator privileges.
pub fn handle_oper(ctx: &HandlerContext, name: &str, password: &str) -> Result<()> {
    // Find the operator config by name
    let oper_config = ctx
        .state
        .config
        .operators
        .iter()
        .find(|op| op.name == name);

    let oper_config = match oper_config {
        Some(config) => config,
        None => {
            // No such operator name
            ctx.reply(
                ERR_PASSWDMISMATCH,
                vec!["Password incorrect".into()],
            )?;
            return Err(Error::PasswordMismatch);
        }
    };

    // Check host mask if configured
    if let Some(ref host_mask) = oper_config.host_mask {
        let client_hostmask = ctx.client.hostmask()?;
        if !matches_mask(host_mask, &client_hostmask) {
            ctx.reply(
                ERR_NOOPERHOST,
                vec!["No O-lines for your host".into()],
            )?;
            return Err(Error::NoOperHost);
        }
    }

    // Verify password using argon2
    let password_hash = &oper_config.password_hash;
    let parsed_hash = match argon2::PasswordHash::new(password_hash) {
        Ok(hash) => hash,
        Err(_) => {
            tracing::error!(oper_name = %name, "Invalid password hash in config");
            ctx.reply(
                ERR_PASSWDMISMATCH,
                vec!["Password incorrect".into()],
            )?;
            return Err(Error::PasswordMismatch);
        }
    };

    use argon2::PasswordVerifier;
    let argon2 = argon2::Argon2::default();
    if argon2.verify_password(password.as_bytes(), &parsed_hash).is_err() {
        ctx.reply(
            ERR_PASSWDMISMATCH,
            vec!["Password incorrect".into()],
        )?;
        return Err(Error::PasswordMismatch);
    }

    // Set operator mode
    ctx.client.modes.write_lock("modes")?.operator = true;

    // 381 RPL_YOUREOPER
    ctx.reply(
        RPL_YOUREOPER,
        vec!["You are now an IRC operator".into()],
    )?;

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

    let killer_nick = ctx.client.nickname()?.unwrap_or_else(|| "operator".to_string());
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
        params: vec![format!("Closing Link: {} (Killed ({}: {}))",
            target.hostname()?, killer_nick, comment)],
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
