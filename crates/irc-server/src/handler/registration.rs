//! Registration command handlers (PASS, NICK, USER, QUIT).

use chrono::Utc;
use irc_proto::{errors::*, Command, Message};

use super::HandlerContext;
use crate::error::{Error, Result};
use crate::reply::send_welcome_burst;
use crate::state::{RegistrationPhase, WhowasEntry};

/// Handle PASS command.
///
/// Sets the connection password before registration completes.
pub fn handle_pass(ctx: &HandlerContext, password: &str) -> Result<()> {
    if ctx.client.is_registered()? {
        ctx.reply(ERR_ALREADYREGISTERED, vec!["You may not reregister".into()])?;
        return Err(Error::AlreadyRegistered);
    }

    if password.is_empty() {
        ctx.reply(
            ERR_NEEDMOREPARAMS,
            vec!["PASS".into(), "Not enough parameters".into()],
        )?;
        return Err(Error::NeedMoreParams("PASS".into()));
    }

    ctx.client.set_password(password.to_string())?;
    Ok(())
}

/// Handle NICK command.
///
/// Sets or changes the client's nickname.
pub fn handle_nick(ctx: &HandlerContext, nickname: &str) -> Result<()> {
    // Check if nickname is provided
    if nickname.is_empty() {
        ctx.reply(ERR_NONICKNAMEGIVEN, vec!["No nickname given".into()])?;
        return Err(Error::NeedMoreParams("NICK".into()));
    }

    // Validate nickname format
    if let Err(e) = irc_proto::validate_nickname(nickname) {
        ctx.reply(
            ERR_ERRONEUSNICKNAME,
            vec![nickname.to_string(), format!("Erroneous nickname: {}", e)],
        )?;
        return Err(Error::InvalidNickname(nickname.to_string()));
    }

    let old_nick = ctx.client.nickname()?;
    let is_registered = ctx.client.is_registered()?;

    // Check if nickname is already in use (by someone else)
    if let Some(existing) = ctx.state.find_client_by_nick(nickname) {
        if existing.id != ctx.client.id {
            ctx.reply(
                ERR_NICKNAMEINUSE,
                vec![nickname.to_string(), "Nickname is already in use".into()],
            )?;
            return Err(Error::NicknameInUse(nickname.to_string()));
        }
        // Same client, same nick (possibly different case) - allow it
    }

    // Unregister old nickname if we had one
    if let Some(ref old) = old_nick {
        ctx.state.unregister_nickname(old);
    }

    // Register new nickname
    if !ctx.state.register_nickname(nickname, ctx.client.id) {
        // Race condition - someone else grabbed it
        // Re-register old nickname if we had one
        if let Some(ref old) = old_nick {
            ctx.state.register_nickname(old, ctx.client.id);
        }
        ctx.reply(
            ERR_NICKNAMEINUSE,
            vec![nickname.to_string(), "Nickname is already in use".into()],
        )?;
        return Err(Error::NicknameInUse(nickname.to_string()));
    }

    // Update client state
    ctx.client.set_nickname(nickname.to_string())?;
    ctx.client.got_nick()?;

    // If already registered, broadcast nick change
    if is_registered {
        if let Some(old) = old_nick {
            // Send NICK message to the client (and in Phase 2, to channels)
            let msg = Message::with_prefix(
                ctx.client.prefix()?,
                Command::Nick {
                    nickname: nickname.to_string(),
                },
            );
            ctx.client.send(msg)?;

            tracing::info!(
                client_id = %ctx.client.id,
                old_nick = %old,
                new_nick = %nickname,
                "Nick change"
            );
        }
    } else {
        // Check if registration is now complete
        check_registration_complete(ctx)?;
    }

    Ok(())
}

/// Handle USER command.
///
/// Sets the username and realname for the connection.
pub fn handle_user(ctx: &HandlerContext, username: &str, realname: &str) -> Result<()> {
    if ctx.client.is_registered()? {
        ctx.reply(ERR_ALREADYREGISTERED, vec!["You may not reregister".into()])?;
        return Err(Error::AlreadyRegistered);
    }

    if username.is_empty() {
        ctx.reply(
            ERR_NEEDMOREPARAMS,
            vec!["USER".into(), "Not enough parameters".into()],
        )?;
        return Err(Error::NeedMoreParams("USER".into()));
    }

    // Sanitize username (remove invalid characters)
    let clean_username: String = username
        .chars()
        .filter(|c| c.is_ascii_alphanumeric() || *c == '_' || *c == '-')
        .take(10)
        .collect();

    let clean_username = if clean_username.is_empty() {
        "unknown".to_string()
    } else {
        clean_username
    };

    ctx.client
        .set_user(clean_username, realname.to_string())?;
    ctx.client.got_user()?;

    // Check if registration is now complete
    check_registration_complete(ctx)?;

    Ok(())
}

/// Handle QUIT command.
///
/// Disconnects the client with an optional message.
pub fn handle_quit(ctx: &HandlerContext, message: Option<&str>) -> Result<()> {
    let quit_msg = message.unwrap_or("Client Quit");

    tracing::info!(
        client_id = %ctx.client.id,
        nick = ?ctx.client.nickname()?,
        message = %quit_msg,
        "Client quit"
    );

    // Record WHOWAS entry for registered clients
    if ctx.client.is_registered()? {
        if let (Some(nick), Some(user), Some(realname)) = (
            ctx.client.nickname()?,
            ctx.client.username()?,
            ctx.client.realname()?,
        ) {
            let entry = WhowasEntry {
                nickname: nick,
                username: user,
                hostname: ctx.client.hostname()?,
                realname,
                quit_time: Utc::now(),
                server: ctx.state.config.server_name.clone(),
            };
            if let Err(e) = ctx.state.record_whowas(entry) {
                tracing::warn!(error = %e, "Failed to record WHOWAS entry");
            }
        }
    }

    // Broadcast QUIT to all users sharing channels with this client
    let quit_broadcast = Message::with_prefix(
        ctx.client.prefix()?,
        Command::Quit {
            message: Some(quit_msg.to_string()),
        },
    );

    // Get all users who share channels with this client
    let common_members = ctx.state.get_common_channel_members(ctx.client.id)?;
    for member_id in common_members {
        if let Some(member) = ctx.state.clients.get(&member_id) {
            // Log but continue if send fails - don't let one slow client block others
            if let Err(e) = member.send(quit_broadcast.clone()) {
                tracing::debug!(member_id = %member_id, error = %e, "Failed to send QUIT broadcast");
            }
        }
    }

    // Remove client from all channels
    ctx.state.remove_client_from_all_channels(ctx.client.id)?;

    // Send ERROR message to client before disconnecting
    let error_msg = Message::new(Command::Unknown {
        command: "ERROR".into(),
        params: vec![format!("Closing Link: {} ({})", ctx.client.hostname()?, quit_msg)],
    });
    // Ignore send errors - client is quitting anyway
    let _ = ctx.client.send(error_msg);

    // The connection handler will clean up when the client disconnects
    // We signal disconnection by returning an error
    Err(Error::Disconnected)
}

/// Check if registration is complete and send welcome burst if so.
fn check_registration_complete(ctx: &HandlerContext) -> Result<()> {
    if ctx.client.registration_phase()? == RegistrationPhase::Registered {
        tracing::info!(
            client_id = %ctx.client.id,
            nick = ?ctx.client.nickname()?,
            user = ?ctx.client.username()?,
            "Client registered"
        );

        // Send welcome burst in a spawned task to avoid blocking
        let client = ctx.client.clone();
        let state = ctx.state.clone();
        tokio::spawn(async move {
            send_welcome_burst(&client, &state).await;
        });
    }
    Ok(())
}
