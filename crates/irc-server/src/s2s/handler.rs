//! S2S message handler.
//!
//! Dispatches incoming S2S commands to appropriate handlers.

// Allow unused variables and dead code in handler stubs - will be called from integration layer
#![allow(unused_variables, dead_code, clippy::too_many_arguments)]

use std::sync::Arc;

use irc_proto::{S2SCommand, S2SMessage};

use crate::error::Result;
use crate::lock::RwLockExt;
use crate::state::ServerState;

use super::burst::{process_bmask, process_sjoin, process_tb};
use super::state::{LinkState, ServerLink};

/// Handle an incoming S2S message.
pub fn handle_s2s_message(
    state: &Arc<ServerState>,
    link: &Arc<ServerLink>,
    msg: S2SMessage,
    our_sid: &str,
) -> Result<()> {
    let source = msg.source.as_deref();

    match msg.command {
        // === Burst Commands ===
        S2SCommand::Burst => {
            tracing::info!(sid = %link.sid, "Remote server starting BURST");
            link.set_state(LinkState::Bursting)?;
        }

        S2SCommand::EndBurst => {
            tracing::info!(sid = %link.sid, "Remote server finished BURST");
            link.set_state(LinkState::Ready)?;

            // Send acknowledgement
            let ack = S2SMessage::with_source(our_sid.to_string(), S2SCommand::EndBurstAck);
            link.send(ack)?;
        }

        S2SCommand::EndBurstAck => {
            tracing::info!(sid = %link.sid, "Remote server acknowledged our BURST");
        }

        S2SCommand::Sid {
            name,
            hopcount,
            sid,
            description: _,
        } => {
            tracing::info!(
                name = %name,
                sid = %sid,
                hopcount = hopcount,
                "Learned about remote server"
            );
            // TODO: Add to server state
        }

        S2SCommand::Uid {
            nick,
            hopcount,
            nick_ts,
            modes,
            user,
            host,
            ip,
            uid,
            realname,
        } => {
            handle_uid(
                state, link, &nick, hopcount, nick_ts, &modes, &user, &host, &ip, &uid, &realname,
            )?;
        }

        S2SCommand::Euid {
            nick,
            hopcount,
            nick_ts,
            modes,
            user,
            visible_host,
            ip,
            uid,
            real_host,
            account,
            realname,
        } => {
            handle_euid(
                state,
                link,
                &nick,
                hopcount,
                nick_ts,
                &modes,
                &user,
                &visible_host,
                &ip,
                &uid,
                &real_host,
                account.as_deref(),
                &realname,
            )?;
        }

        S2SCommand::Sjoin {
            channel_ts,
            channel,
            modes,
            mode_params,
            members,
        } => {
            let remote_sid = source.unwrap_or(&link.sid);
            process_sjoin(
                state,
                remote_sid,
                channel_ts,
                &channel,
                &modes,
                &mode_params,
                &members,
            )?;
        }

        S2SCommand::Tb {
            channel,
            topic_ts,
            setter,
            topic,
        } => {
            process_tb(state, &channel, topic_ts, setter.as_deref(), &topic)?;
        }

        S2SCommand::Bmask {
            channel_ts,
            channel,
            list_type,
            masks,
        } => {
            process_bmask(state, channel_ts, &channel, list_type, &masks)?;
        }

        // === Runtime Commands ===
        S2SCommand::Privmsg { target, text } => {
            handle_privmsg(state, source, &target, &text)?;
        }

        S2SCommand::Notice { target, text } => {
            handle_notice(state, source, &target, &text)?;
        }

        S2SCommand::Join {
            channel_ts,
            channel,
        } => {
            if let Some(uid) = source {
                handle_join(state, uid, channel_ts, &channel)?;
            }
        }

        S2SCommand::Part { channel, reason } => {
            if let Some(uid) = source {
                handle_part(state, uid, &channel, reason.as_deref())?;
            }
        }

        S2SCommand::Quit { reason } => {
            if let Some(uid) = source {
                handle_quit(state, link, uid, reason.as_deref())?;
            }
        }

        S2SCommand::Nick { nick, ts } => {
            if let Some(uid) = source {
                handle_nick_change(state, uid, &nick, ts)?;
            }
        }

        S2SCommand::Kill { uid, path, reason } => {
            handle_kill(state, source.unwrap_or("unknown"), &uid, &path, &reason)?;
        }

        S2SCommand::Kick {
            channel,
            uid,
            reason,
        } => {
            handle_kick(state, source, &channel, &uid, &reason)?;
        }

        S2SCommand::Tmode {
            channel_ts,
            channel,
            modes,
            params,
        } => {
            handle_tmode(state, source, channel_ts, &channel, &modes, &params)?;
        }

        S2SCommand::Topic {
            channel,
            setter,
            ts,
            topic,
        } => {
            handle_topic(state, &channel, &setter, ts, &topic)?;
        }

        S2SCommand::Squit { sid, reason } => {
            handle_squit(state, link, &sid, &reason)?;
        }

        S2SCommand::Ping {
            source: ping_src,
            target: _,
        } => {
            // Reply with PONG
            let pong = S2SMessage::with_source(
                our_sid.to_string(),
                S2SCommand::Pong {
                    source: our_sid.to_string(),
                    target: ping_src,
                },
            );
            link.send(pong)?;
        }

        S2SCommand::Pong { .. } => {
            // Just acknowledge, could track latency here
        }

        S2SCommand::Mode { target, modes } => {
            handle_mode(state, source, &target, &modes)?;
        }

        S2SCommand::Away { reason } => {
            if let Some(uid) = source {
                handle_away(state, uid, reason.as_deref())?;
            }
        }

        S2SCommand::Encap {
            target,
            subcommand,
            params,
        } => {
            handle_encap(state, source, &target, &subcommand, &params)?;
        }

        S2SCommand::Wallops { message } => {
            handle_wallops(state, source, &message)?;
        }

        _ => {
            tracing::debug!(
                command = %msg.command.name(),
                "Unhandled S2S command"
            );
        }
    }

    Ok(())
}

// === Handler Implementations ===

fn handle_uid(
    _state: &ServerState,
    link: &ServerLink,
    nick: &str,
    _hopcount: u32,
    _nick_ts: i64,
    _modes: &str,
    _user: &str,
    _host: &str,
    _ip: &str,
    uid: &str,
    _realname: &str,
) -> Result<()> {
    tracing::debug!(
        nick = %nick,
        uid = %uid,
        "Received UID from remote server"
    );

    // TODO: Check for nick collision
    // TODO: Create remote client in state

    // Track this UID on the link
    link.add_user(uid)?;

    Ok(())
}

fn handle_euid(
    _state: &ServerState,
    link: &ServerLink,
    nick: &str,
    _hopcount: u32,
    _nick_ts: i64,
    _modes: &str,
    _user: &str,
    _visible_host: &str,
    _ip: &str,
    uid: &str,
    _real_host: &str,
    account: Option<&str>,
    _realname: &str,
) -> Result<()> {
    tracing::debug!(
        nick = %nick,
        uid = %uid,
        account = ?account,
        "Received EUID from remote server"
    );

    // TODO: Check for nick collision
    // TODO: Create remote client in state with account info

    // Track this UID on the link
    link.add_user(uid)?;

    Ok(())
}

fn handle_privmsg(
    state: &ServerState,
    source: Option<&str>,
    target: &str,
    text: &str,
) -> Result<()> {
    use irc_proto::{Command, Message, Prefix};

    let source_uid = source.unwrap_or("unknown");

    if target.starts_with('#') || target.starts_with('&') {
        // Channel message - deliver to local members
        if let Some(channel_arc) = state.get_channel(target) {
            let channel = channel_arc.read_lock("channel")?;

            // TODO: Look up source nick from UID
            let source_nick = source_uid; // Placeholder

            let msg = Message::with_prefix(
                Prefix::from_nick(source_nick),
                Command::Privmsg {
                    target: target.to_string(),
                    message: text.to_string(),
                },
            );

            state.broadcast_to_channel(&channel, msg, None);
        }
    } else {
        // User message - deliver to local user by UID
        // TODO: Look up local client by UID and deliver
    }

    Ok(())
}

fn handle_notice(
    state: &ServerState,
    source: Option<&str>,
    target: &str,
    text: &str,
) -> Result<()> {
    // Similar to PRIVMSG but for notices
    // TODO: Implement
    Ok(())
}

fn handle_join(state: &ServerState, uid: &str, channel_ts: i64, channel: &str) -> Result<()> {
    tracing::debug!(uid = %uid, channel = %channel, "Remote user joined channel");
    // TODO: Add remote user to channel
    Ok(())
}

fn handle_part(state: &ServerState, uid: &str, channel: &str, reason: Option<&str>) -> Result<()> {
    tracing::debug!(uid = %uid, channel = %channel, "Remote user left channel");
    // TODO: Remove remote user from channel
    Ok(())
}

fn handle_quit(
    state: &ServerState,
    link: &ServerLink,
    uid: &str,
    reason: Option<&str>,
) -> Result<()> {
    tracing::debug!(uid = %uid, reason = ?reason, "Remote user quit");

    // Remove from link's user list
    link.remove_user(uid)?;

    // TODO: Remove from all channels, notify local users
    Ok(())
}

fn handle_nick_change(state: &ServerState, uid: &str, new_nick: &str, ts: i64) -> Result<()> {
    tracing::debug!(uid = %uid, new_nick = %new_nick, "Remote user changed nick");
    // TODO: Update nick in state, check for collision
    Ok(())
}

fn handle_kill(
    state: &ServerState,
    source: &str,
    uid: &str,
    path: &str,
    reason: &str,
) -> Result<()> {
    tracing::info!(uid = %uid, reason = %reason, "User killed");
    // TODO: If UID is local, disconnect the client
    // TODO: If UID is remote, remove from state
    Ok(())
}

fn handle_kick(
    state: &ServerState,
    source: Option<&str>,
    channel: &str,
    uid: &str,
    reason: &str,
) -> Result<()> {
    tracing::debug!(uid = %uid, channel = %channel, "Remote user kicked");
    // TODO: Remove user from channel, notify local members
    Ok(())
}

fn handle_tmode(
    state: &ServerState,
    source: Option<&str>,
    channel_ts: i64,
    channel: &str,
    modes: &str,
    params: &[String],
) -> Result<()> {
    use super::collision::handle_channel_ts;

    if let Some(channel_arc) = state.get_channel(channel) {
        let our_ts = channel_arc.read_lock("channel")?.created_at.timestamp();

        if handle_channel_ts(our_ts, channel_ts) {
            // Accept the mode change
            tracing::debug!(
                channel = %channel,
                modes = %modes,
                "Applying remote mode change"
            );
            // TODO: Apply modes to channel
        } else {
            tracing::debug!(
                channel = %channel,
                "Ignoring mode change with newer TS"
            );
        }
    }

    Ok(())
}

fn handle_topic(
    state: &ServerState,
    channel: &str,
    setter: &str,
    ts: i64,
    topic: &str,
) -> Result<()> {
    if let Some(channel_arc) = state.get_channel(channel) {
        let mut chan = channel_arc.write_lock("channel")?;

        let our_ts = chan.topic_set_at.map(|t| t.timestamp()).unwrap_or(i64::MAX);

        if ts <= our_ts {
            chan.topic = Some(topic.to_string());
            chan.topic_set_by = Some(setter.to_string());
            chan.topic_set_at = chrono::Utc.timestamp_opt(ts, 0).single();

            // TODO: Notify local channel members
        }
    }

    Ok(())
}

fn handle_squit(state: &ServerState, link: &ServerLink, sid: &str, reason: &str) -> Result<()> {
    tracing::info!(sid = %sid, reason = %reason, "Server disconnected");

    // TODO: Remove all users from this server
    // TODO: If this is the link itself, mark as disconnected
    // TODO: If it's a child server, remove from topology

    Ok(())
}

fn handle_mode(state: &ServerState, source: Option<&str>, target: &str, modes: &str) -> Result<()> {
    // User mode change from remote
    tracing::debug!(target = %target, modes = %modes, "Remote user mode change");
    // TODO: Update user modes in state
    Ok(())
}

fn handle_away(state: &ServerState, uid: &str, reason: Option<&str>) -> Result<()> {
    tracing::debug!(uid = %uid, away = reason.is_some(), "Remote user away status change");
    // TODO: Update away status in state
    Ok(())
}

fn handle_encap(
    state: &ServerState,
    source: Option<&str>,
    target: &str,
    subcommand: &str,
    params: &[String],
) -> Result<()> {
    tracing::debug!(
        subcommand = %subcommand,
        "Received ENCAP"
    );
    // ENCAP is used for extensible commands - could handle specific ones here
    Ok(())
}

fn handle_wallops(state: &ServerState, source: Option<&str>, message: &str) -> Result<()> {
    use irc_proto::{Command, Message, Prefix};

    // Deliver to all local operators with +w
    for entry in state.clients.iter() {
        let client = entry.value();

        if let Ok(modes) = client.modes.read_lock("modes")
            && modes.operator
            && modes.wallops
        {
            let source_nick = source.unwrap_or("unknown");
            let msg = Message::with_prefix(
                Prefix::from_nick(source_nick),
                Command::Wallops {
                    message: message.to_string(),
                },
            );
            let _ = client.send(msg);
        }
    }

    Ok(())
}

use chrono::TimeZone;
