//! BURST state synchronization.
//!
// Allow dead code during initial development - functions will be called from integration layer
#![allow(dead_code)]
//!
//! When two servers link, they exchange their state via BURST messages:
//! - SID: Introduce other servers
//! - EUID: Introduce users
//! - SJOIN: Introduce channel memberships
//! - TB: Topic burst
//! - BMASK: Ban/exception/invex lists

use std::sync::Arc;

use chrono::Utc;
use irc_proto::{S2SCommand, S2SMessage, SjoinMember};
use tokio::io::{AsyncWrite, AsyncWriteExt};

use crate::error::Result;
use crate::state::{ServerState, Channel};
use crate::lock::RwLockExt;


/// Send our state to a newly linked server.
pub async fn send_burst<W>(
    writer: &mut W,
    state: &Arc<ServerState>,
    our_sid: &str,
) -> Result<()>
where
    W: AsyncWrite + Unpin,
{
    // Send BURST to signal start
    let burst_msg = S2SMessage::with_source(our_sid.to_string(), S2SCommand::Burst);
    writer.write_all(&burst_msg.to_bytes()).await?;

    // Send SID for each known server (none for single server)
    // In a mesh network, we'd iterate state.servers here

    // Send EUID for each local user
    for entry in state.clients.iter() {
        let client = entry.value();

        // Only send registered local clients
        if !client.is_registered()? {
            continue;
        }

        let nick = match client.nickname()? {
            Some(n) => n,
            None => continue,
        };

        let username = client.username()?.unwrap_or_else(|| "unknown".to_string());
        let hostname = client.hostname()?;
        let realname = client.realname()?.unwrap_or_else(|| "Unknown".to_string());
        let account = client.account()?;

        // Build user modes string
        let modes_str = client.modes.read_lock("modes")?.to_string();
        let modes = if modes_str.is_empty() { "+".to_string() } else { modes_str };

        // Generate a UID for this client if not already assigned
        // For now, we use a placeholder - real implementation would track UIDs
        let uid = format!("{}{:06}", our_sid, client.id.0 % 1_000_000);

        let euid = S2SMessage::with_source(
            our_sid.to_string(),
            S2SCommand::Euid {
                nick: nick.clone(),
                hopcount: 1,
                nick_ts: client.connected_at.timestamp(),
                modes,
                user: username.clone(),
                visible_host: hostname.clone(),
                ip: client.addr.ip().to_string(),
                uid: uid.clone(),
                real_host: hostname,
                account,
                realname,
            },
        );
        writer.write_all(&euid.to_bytes()).await?;
    }

    // Collect channel data first, then send (to avoid holding lock across await)
    struct ChannelBurstData {
        channel_name: String,
        channel_ts: i64,
        modes: String,
        mode_params: Vec<String>,
        members: Vec<SjoinMember>,
        topic: Option<(String, i64, Option<String>)>, // (topic, ts, setter)
        bans: Vec<String>,
        exceptions: Vec<String>,
        invites: Vec<String>,
    }

    let mut channel_data = Vec::new();

    for entry in state.channels.iter() {
        let channel_name = entry.key().to_string();
        let channel = entry.value().read_lock("channel")?;

        // Build member list with prefixes
        let mut members = Vec::new();
        for (&client_id, status) in &channel.members {
            if let Some(client) = state.clients.get(&client_id) {
                let uid = format!("{}{:06}", our_sid, client.id.0 % 1_000_000);
                let mut prefixes = String::new();
                if status.operator {
                    prefixes.push('@');
                }
                if status.voice {
                    prefixes.push('+');
                }
                members.push(SjoinMember::with_prefixes(prefixes, uid));
            }
        }

        if members.is_empty() {
            continue;
        }

        let mode_string = channel.mode_string();
        let (modes, mode_params) = parse_mode_string(&mode_string);

        let topic = channel.topic.as_ref().map(|t| {
            let ts = channel.topic_set_at.map(|dt| dt.timestamp()).unwrap_or_else(|| Utc::now().timestamp());
            (t.clone(), ts, channel.topic_set_by.clone())
        });

        channel_data.push(ChannelBurstData {
            channel_name,
            channel_ts: channel.created_at.timestamp(),
            modes,
            mode_params,
            members,
            topic,
            bans: channel.bans.iter().map(|e| e.mask.clone()).collect(),
            exceptions: channel.exceptions.iter().map(|e| e.mask.clone()).collect(),
            invites: channel.invites.iter().map(|e| e.mask.clone()).collect(),
        });
    }

    // Now send all channel data (lock dropped)
    for data in channel_data {
        let sjoin = S2SMessage::with_source(
            our_sid.to_string(),
            S2SCommand::Sjoin {
                channel_ts: data.channel_ts,
                channel: data.channel_name.clone(),
                modes: data.modes,
                mode_params: data.mode_params,
                members: data.members,
            },
        );
        writer.write_all(&sjoin.to_bytes()).await?;

        if let Some((topic, topic_ts, setter)) = data.topic {
            let tb = S2SMessage::with_source(
                our_sid.to_string(),
                S2SCommand::Tb {
                    channel: data.channel_name.clone(),
                    topic_ts,
                    setter,
                    topic,
                },
            );
            writer.write_all(&tb.to_bytes()).await?;
        }

        if !data.bans.is_empty() {
            let bmask = S2SMessage::with_source(
                our_sid.to_string(),
                S2SCommand::Bmask {
                    channel_ts: data.channel_ts,
                    channel: data.channel_name.clone(),
                    list_type: 'b',
                    masks: data.bans,
                },
            );
            writer.write_all(&bmask.to_bytes()).await?;
        }

        if !data.exceptions.is_empty() {
            let bmask = S2SMessage::with_source(
                our_sid.to_string(),
                S2SCommand::Bmask {
                    channel_ts: data.channel_ts,
                    channel: data.channel_name.clone(),
                    list_type: 'e',
                    masks: data.exceptions,
                },
            );
            writer.write_all(&bmask.to_bytes()).await?;
        }

        if !data.invites.is_empty() {
            let bmask = S2SMessage::with_source(
                our_sid.to_string(),
                S2SCommand::Bmask {
                    channel_ts: data.channel_ts,
                    channel: data.channel_name.clone(),
                    list_type: 'I',
                    masks: data.invites,
                },
            );
            writer.write_all(&bmask.to_bytes()).await?;
        }
    }

    // Send ENDBURST to signal completion
    let endburst_msg = S2SMessage::with_source(our_sid.to_string(), S2SCommand::EndBurst);
    writer.write_all(&endburst_msg.to_bytes()).await?;

    Ok(())
}

/// Process an SJOIN from a remote server.
///
/// Handles timestamp-based merging:
/// - If remote TS < our TS: Remote wins, reset our modes
/// - If remote TS > our TS: We win, members join without status
/// - If equal: Merge normally
pub fn process_sjoin(
    state: &Arc<ServerState>,
    _remote_sid: &str,
    channel_ts: i64,
    channel_name: &str,
    modes: &str,
    mode_params: &[String],
    members: &[SjoinMember],
) -> Result<()> {
    use chrono::TimeZone;

    let (channel, created) = state.get_or_create_channel(channel_name);
    let mut channel = channel.write_lock("channel")?;

    let our_ts = channel.created_at.timestamp();
    let remote_ts_dt = Utc.timestamp_opt(channel_ts, 0).single()
        .unwrap_or_else(Utc::now);

    if channel_ts < our_ts || created {
        // Remote is older or channel is new - accept their modes
        channel.created_at = remote_ts_dt;

        // Reset our modes and apply theirs
        channel.modes = Default::default();
        apply_modes(&mut channel, modes, mode_params);

        // Add members with their status
        for member in members {
            // TODO: Look up client by UID and add to channel
            // For now, we just log
            tracing::debug!(
                uid = %member.uid,
                prefixes = %member.prefixes,
                channel = %channel_name,
                "Adding remote member via SJOIN"
            );
        }
    } else if channel_ts > our_ts {
        // We're older - members join without status
        for member in members {
            tracing::debug!(
                uid = %member.uid,
                channel = %channel_name,
                "Remote member joins without status (our TS is older)"
            );
            // TODO: Add member without op/voice
        }
    } else {
        // Same TS - merge
        for member in members {
            tracing::debug!(
                uid = %member.uid,
                prefixes = %member.prefixes,
                channel = %channel_name,
                "Merging remote member via SJOIN"
            );
            // TODO: Add member with their status
        }
    }

    Ok(())
}

/// Process a TB (topic burst) from a remote server.
pub fn process_tb(
    state: &Arc<ServerState>,
    channel_name: &str,
    topic_ts: i64,
    setter: Option<&str>,
    topic: &str,
) -> Result<()> {
    use chrono::TimeZone;

    if let Some(channel_arc) = state.get_channel(channel_name) {
        let mut channel = channel_arc.write_lock("channel")?;

        // Accept topic if we don't have one or theirs is older
        let our_ts = channel.topic_set_at.map(|t| t.timestamp()).unwrap_or(i64::MAX);

        if topic_ts <= our_ts {
            channel.topic = Some(topic.to_string());
            channel.topic_set_by = setter.map(String::from);
            channel.topic_set_at = Utc.timestamp_opt(topic_ts, 0).single();

            tracing::debug!(
                channel = %channel_name,
                topic = %topic,
                "Applied topic from TB"
            );
        }
    }

    Ok(())
}

/// Process a BMASK (ban/exception/invite list) from a remote server.
pub fn process_bmask(
    state: &Arc<ServerState>,
    channel_ts: i64,
    channel_name: &str,
    list_type: char,
    masks: &[String],
) -> Result<()> {
    if let Some(channel_arc) = state.get_channel(channel_name) {
        let mut channel = channel_arc.write_lock("channel")?;

        // Only accept if their TS matches ours
        let our_ts = channel.created_at.timestamp();
        if channel_ts > our_ts {
            tracing::debug!(
                channel = %channel_name,
                list_type = %list_type,
                "Ignoring BMASK with newer TS"
            );
            return Ok(());
        }

        let set_by = "remote".to_string();

        for mask in masks {
            match list_type {
                'b' => channel.add_ban(mask.clone(), set_by.clone()),
                'e' => channel.add_exception(mask.clone(), set_by.clone()),
                'I' => channel.add_invite_exception(mask.clone(), set_by.clone()),
                _ => {
                    tracing::warn!(list_type = %list_type, "Unknown BMASK list type");
                }
            }
        }
    }

    Ok(())
}

/// Parse a mode string like "+ntk secret" into modes and params.
fn parse_mode_string(mode_string: &str) -> (String, Vec<String>) {
    let parts: Vec<_> = mode_string.split_whitespace().collect();
    if parts.is_empty() {
        return ("+".to_string(), Vec::new());
    }

    let modes = parts[0].to_string();
    let params = parts[1..].iter().map(|s| s.to_string()).collect();

    (modes, params)
}

/// Apply mode string to a channel.
fn apply_modes(channel: &mut Channel, modes: &str, params: &[String]) {
    let mut param_iter = params.iter();
    let mut adding = true;

    for c in modes.chars() {
        match c {
            '+' => adding = true,
            '-' => adding = false,
            'i' => channel.modes.invite_only = adding,
            'm' => channel.modes.moderated = adding,
            'n' => channel.modes.no_external = adding,
            's' => channel.modes.secret = adding,
            't' => channel.modes.topic_locked = adding,
            'k' => {
                if adding {
                    channel.modes.key = param_iter.next().cloned();
                } else {
                    channel.modes.key = None;
                }
            }
            'l' => {
                if adding {
                    channel.modes.limit = param_iter.next().and_then(|s| s.parse().ok());
                } else {
                    channel.modes.limit = None;
                }
            }
            _ => {}
        }
    }
}
