//! Channel command handlers (JOIN, PART, TOPIC, NAMES, LIST, MODE, KICK, INVITE).

use std::collections::HashSet;

use irc_proto::{errors::*, replies::*, ChannelMode, Command, Message, ModeChanges};

use super::HandlerContext;
use crate::error::{Error, Result};
use crate::lock::RwLockExt;
use crate::state::{Channel, JoinError, MemberStatus};

/// Handle JOIN command.
///
/// Join one or more channels, creating them if they don't exist.
pub fn handle_join(ctx: &HandlerContext, channels: &[(String, Option<String>)]) -> Result<()> {
    if channels.is_empty() {
        ctx.reply(
            ERR_NEEDMOREPARAMS,
            vec!["JOIN".into(), "Not enough parameters".into()],
        )?;
        return Err(Error::NeedMoreParams("JOIN".into()));
    }

    for (channel_name, key) in channels {
        join_single_channel(ctx, channel_name, key.as_deref())?;
    }

    Ok(())
}

/// Join a single channel.
fn join_single_channel(ctx: &HandlerContext, channel_name: &str, key: Option<&str>) -> Result<()> {
    // Validate channel name
    if let Err(e) = irc_proto::validate_channel(channel_name) {
        ctx.reply(
            ERR_NOSUCHCHANNEL,
            vec![channel_name.to_string(), format!("Invalid channel name: {}", e)],
        )?;
        return Err(Error::InvalidChannel(channel_name.to_string()));
    }

    let client_id = ctx.client.id;
    let hostmask = ctx.client.hostmask()?;

    // Get or create the channel
    let (channel_arc, is_new) = ctx.state.get_or_create_channel(channel_name);

    // Track invited clients for this channel
    let mut invited_clients = HashSet::new();

    {
        let mut channel = channel_arc.write_lock("channel")?;

        // Check if already a member
        if channel.is_member(client_id) {
            return Ok(());
        }

        // For existing channels, get invited clients from invite list
        if !is_new {
            for entry in &channel.invites {
                // Check if this client matches any invite mask
                if crate::state::matches_mask(&entry.mask, &hostmask) {
                    invited_clients.insert(client_id);
                }
            }
        }

        // Check join restrictions (unless channel is new)
        if !is_new {
            match channel.can_join(&hostmask, key, &invited_clients, client_id) {
                Ok(()) => {}
                Err(JoinError::ChannelFull) => {
                    ctx.reply(
                        ERR_CHANNELISFULL,
                        vec![channel_name.to_string(), "Cannot join channel (+l)".into()],
                    )?;
                    return Err(Error::ChannelFull(channel_name.to_string()));
                }
                Err(JoinError::Banned) => {
                    ctx.reply(
                        ERR_BANNEDFROMCHAN,
                        vec![channel_name.to_string(), "Cannot join channel (+b)".into()],
                    )?;
                    return Err(Error::BannedFromChannel(channel_name.to_string()));
                }
                Err(JoinError::BadKey) => {
                    ctx.reply(
                        ERR_BADCHANNELKEY,
                        vec![channel_name.to_string(), "Cannot join channel (+k)".into()],
                    )?;
                    return Err(Error::BadChannelKey(channel_name.to_string()));
                }
                Err(JoinError::InviteOnly) => {
                    ctx.reply(
                        ERR_INVITEONLYCHAN,
                        vec![channel_name.to_string(), "Cannot join channel (+i)".into()],
                    )?;
                    return Err(Error::InviteOnlyChannel(channel_name.to_string()));
                }
            }
        }

        // Add member (first user gets ops)
        let status = if is_new {
            MemberStatus {
                operator: true,
                voice: false,
            }
        } else {
            MemberStatus::default()
        };
        channel.add_member(client_id, status);
    }

    // Update client's channel list
    ctx.client.join_channel(channel_name)?;

    // Build JOIN message
    let join_msg = Message::with_prefix(
        ctx.client.prefix()?,
        Command::Join {
            channels: vec![(channel_name.to_string(), None)],
        },
    );

    // Broadcast JOIN to all channel members (including self)
    {
        let channel = channel_arc.read_lock("channel")?;
        ctx.state.broadcast_to_channel(&channel, join_msg.clone(), None);
    }

    // Send topic to joining user
    {
        let channel = channel_arc.read_lock("channel")?;
        send_topic_to_client(ctx, &channel)?;
    }

    // Send names list to joining user
    {
        let channel = channel_arc.read_lock("channel")?;
        send_names_to_client(ctx, &channel)?;
    }

    tracing::debug!(
        client_id = %ctx.client.id,
        nick = ?ctx.client.nickname()?,
        channel = %channel_name,
        "Joined channel"
    );

    Ok(())
}

/// Handle PART command.
pub fn handle_part(
    ctx: &HandlerContext,
    channels: &[String],
    message: Option<&str>,
) -> Result<()> {
    if channels.is_empty() {
        ctx.reply(
            ERR_NEEDMOREPARAMS,
            vec!["PART".into(), "Not enough parameters".into()],
        )?;
        return Err(Error::NeedMoreParams("PART".into()));
    }

    for channel_name in channels {
        part_single_channel(ctx, channel_name, message)?;
    }

    Ok(())
}

/// Part a single channel.
fn part_single_channel(
    ctx: &HandlerContext,
    channel_name: &str,
    message: Option<&str>,
) -> Result<()> {
    let channel_arc = match ctx.state.get_channel(channel_name) {
        Some(c) => c,
        None => {
            ctx.reply(
                ERR_NOSUCHCHANNEL,
                vec![channel_name.to_string(), "No such channel".into()],
            )?;
            return Err(Error::NoSuchChannel(channel_name.to_string()));
        }
    };

    let client_id = ctx.client.id;

    // Check if member
    {
        let channel = channel_arc.read_lock("channel")?;
        if !channel.is_member(client_id) {
            ctx.reply(
                ERR_NOTONCHANNEL,
                vec![channel_name.to_string(), "You're not on that channel".into()],
            )?;
            return Err(Error::NotOnChannel(channel_name.to_string()));
        }
    }

    // Build PART message
    let part_msg = Message::with_prefix(
        ctx.client.prefix()?,
        Command::Part {
            channels: vec![channel_name.to_string()],
            message: message.map(String::from),
        },
    );

    // Broadcast PART to all channel members (including self)
    {
        let channel = channel_arc.read_lock("channel")?;
        ctx.state.broadcast_to_channel(&channel, part_msg, None);
    }

    // Remove member
    let should_remove = {
        let mut channel = channel_arc.write_lock("channel")?;
        channel.remove_member(client_id);
        channel.member_count() == 0
    };

    // Update client's channel list
    ctx.client.leave_channel(channel_name)?;

    // Remove empty channel
    if should_remove {
        ctx.state.remove_channel(channel_name);
    }

    tracing::debug!(
        client_id = %ctx.client.id,
        nick = ?ctx.client.nickname()?,
        channel = %channel_name,
        "Left channel"
    );

    Ok(())
}

/// Handle TOPIC command.
pub fn handle_topic(
    ctx: &HandlerContext,
    channel_name: &str,
    new_topic: Option<&str>,
) -> Result<()> {
    let channel_arc = match ctx.state.get_channel(channel_name) {
        Some(c) => c,
        None => {
            ctx.reply(
                ERR_NOSUCHCHANNEL,
                vec![channel_name.to_string(), "No such channel".into()],
            )?;
            return Err(Error::NoSuchChannel(channel_name.to_string()));
        }
    };

    let client_id = ctx.client.id;

    match new_topic {
        None => {
            // Query topic
            let channel = channel_arc.read_lock("channel")?;
            send_topic_to_client(ctx, &channel)?;
        }
        Some(topic) => {
            // Set topic
            let mut channel = channel_arc.write_lock("channel")?;

            // Check if member
            if !channel.is_member(client_id) {
                ctx.reply(
                    ERR_NOTONCHANNEL,
                    vec![channel_name.to_string(), "You're not on that channel".into()],
                )?;
                return Err(Error::NotOnChannel(channel_name.to_string()));
            }

            // Check +t mode (topic lock)
            if channel.modes.topic_locked && !channel.is_operator(client_id) {
                ctx.reply(
                    ERR_CHANOPRIVSNEEDED,
                    vec![
                        channel_name.to_string(),
                        "You're not channel operator".into(),
                    ],
                )?;
                return Err(Error::NotChannelOperator(channel_name.to_string()));
            }

            // Set the topic
            let setter = ctx.client.nickname()?.unwrap_or_else(|| "*".to_string());
            let topic = if topic.is_empty() {
                None
            } else {
                Some(topic.to_string())
            };
            channel.set_topic(topic.clone(), setter);

            // Broadcast TOPIC to channel
            let topic_msg = Message::with_prefix(
                ctx.client.prefix()?,
                Command::Topic {
                    channel: channel_name.to_string(),
                    topic,
                },
            );
            ctx.state.broadcast_to_channel(&channel, topic_msg, None);
        }
    }

    Ok(())
}

/// Handle NAMES command.
pub fn handle_names(ctx: &HandlerContext, channels: Option<&[String]>) -> Result<()> {
    match channels {
        Some(channel_list) if !channel_list.is_empty() => {
            for channel_name in channel_list {
                if let Some(channel_arc) = ctx.state.get_channel(channel_name) {
                    let channel = channel_arc.read_lock("channel")?;

                    // Check if channel is secret and user is not a member
                    if channel.modes.secret && !channel.is_member(ctx.client.id) {
                        // Don't show secret channels to non-members
                        ctx.reply(
                            RPL_ENDOFNAMES,
                            vec![channel_name.to_string(), "End of /NAMES list".into()],
                        )?;
                        continue;
                    }

                    send_names_to_client(ctx, &channel)?;
                } else {
                    // Channel doesn't exist, just send end of names
                    ctx.reply(
                        RPL_ENDOFNAMES,
                        vec![channel_name.to_string(), "End of /NAMES list".into()],
                    )?;
                }
            }
        }
        _ => {
            // List all visible channels
            for entry in ctx.state.channels.iter() {
                let channel = entry.value().read_lock("channel")?;

                // Skip secret channels unless member
                if channel.modes.secret && !channel.is_member(ctx.client.id) {
                    continue;
                }

                send_names_to_client(ctx, &channel)?;
            }

            // Also list users not in any channel (with * as channel)
            ctx.reply(RPL_ENDOFNAMES, vec!["*".into(), "End of /NAMES list".into()])?;
        }
    }

    Ok(())
}

/// Handle LIST command.
pub fn handle_list(ctx: &HandlerContext, channels: Option<&[String]>) -> Result<()> {
    // RPL_LISTSTART (321) - deprecated but some clients expect it
    ctx.reply(RPL_LISTSTART, vec!["Channel".into(), "Users  Name".into()])?;

    let list_channels: Vec<_> = match channels {
        Some(channel_list) if !channel_list.is_empty() => {
            channel_list
                .iter()
                .filter_map(|name| ctx.state.get_channel(name))
                .collect()
        }
        _ => ctx.state.channels.iter().map(|e| e.value().clone()).collect(),
    };

    for channel_arc in list_channels {
        let channel = channel_arc.read_lock("channel")?;

        // Skip secret channels unless member
        if channel.modes.secret && !channel.is_member(ctx.client.id) {
            continue;
        }

        let topic = channel.topic.clone().unwrap_or_default();
        ctx.reply(
            RPL_LIST,
            vec![
                channel.name.clone(),
                channel.member_count().to_string(),
                topic,
            ],
        )?;
    }

    ctx.reply(RPL_LISTEND, vec!["End of /LIST".into()])?;

    Ok(())
}

/// Handle channel MODE command.
pub fn handle_channel_mode(
    ctx: &HandlerContext,
    channel_name: &str,
    modes: Option<&str>,
    params: &[String],
) -> Result<()> {
    let channel_arc = match ctx.state.get_channel(channel_name) {
        Some(c) => c,
        None => {
            ctx.reply(
                ERR_NOSUCHCHANNEL,
                vec![channel_name.to_string(), "No such channel".into()],
            )?;
            return Err(Error::NoSuchChannel(channel_name.to_string()));
        }
    };

    let client_id = ctx.client.id;

    match modes {
        None => {
            // Query channel modes
            let channel = channel_arc.read_lock("channel")?;
            let mode_str = channel.mode_string();
            ctx.reply(
                RPL_CHANNELMODEIS,
                vec![channel_name.to_string(), mode_str],
            )?;
            ctx.reply(
                RPL_CREATIONTIME,
                vec![
                    channel_name.to_string(),
                    channel.created_at.timestamp().to_string(),
                ],
            )?;
        }
        Some(mode_str) => {
            // Check if just querying a list mode (no params, e.g., "MODE #chan +b")
            if params.is_empty() {
                let mode_chars: Vec<char> = mode_str.chars().filter(|c| *c != '+' && *c != '-').collect();
                if mode_chars.len() == 1 {
                    let channel = channel_arc.read_lock("channel")?;
                    match mode_chars[0] {
                        'b' => {
                            // Return ban list
                            for entry in &channel.bans {
                                ctx.reply(
                                    RPL_BANLIST,
                                    vec![
                                        channel_name.to_string(),
                                        entry.mask.clone(),
                                        entry.set_by.clone(),
                                        entry.set_at.timestamp().to_string(),
                                    ],
                                )?;
                            }
                            ctx.reply(
                                RPL_ENDOFBANLIST,
                                vec![channel_name.to_string(), "End of channel ban list".into()],
                            )?;
                            return Ok(());
                        }
                        'e' => {
                            // Return exception list
                            for entry in &channel.exceptions {
                                ctx.reply(
                                    RPL_EXCEPTLIST,
                                    vec![
                                        channel_name.to_string(),
                                        entry.mask.clone(),
                                        entry.set_by.clone(),
                                        entry.set_at.timestamp().to_string(),
                                    ],
                                )?;
                            }
                            ctx.reply(
                                RPL_ENDOFEXCEPTLIST,
                                vec![channel_name.to_string(), "End of channel exception list".into()],
                            )?;
                            return Ok(());
                        }
                        'I' => {
                            // Return invite list
                            for entry in &channel.invites {
                                ctx.reply(
                                    RPL_INVITELIST,
                                    vec![
                                        channel_name.to_string(),
                                        entry.mask.clone(),
                                        entry.set_by.clone(),
                                        entry.set_at.timestamp().to_string(),
                                    ],
                                )?;
                            }
                            ctx.reply(
                                RPL_ENDOFINVITELIST,
                                vec![channel_name.to_string(), "End of channel invite list".into()],
                            )?;
                            return Ok(());
                        }
                        _ => {}
                    }
                }
            }

            // Parse and apply mode changes
            let params_refs: Vec<&str> = params.iter().map(|s| s.as_str()).collect();
            let changes = ModeChanges::parse(mode_str, &params_refs);

            if changes.is_empty() {
                return Ok(());
            }

            let mut channel = channel_arc.write_lock("channel")?;

            // Check membership
            if !channel.is_member(client_id) {
                ctx.reply(
                    ERR_NOTONCHANNEL,
                    vec![channel_name.to_string(), "You're not on that channel".into()],
                )?;
                return Err(Error::NotOnChannel(channel_name.to_string()));
            }

            // Check operator status for most modes
            let is_op = channel.is_operator(client_id);

            let setter = ctx.client.nickname()?.unwrap_or_else(|| "*".to_string());
            let mut applied_changes = ModeChanges::new();

            for change in &changes.changes {
                let mode = match ChannelMode::from_char(change.mode) {
                    Some(m) => m,
                    None => {
                        ctx.reply(
                            ERR_UNKNOWNMODE,
                            vec![
                                change.mode.to_string(),
                                "is unknown mode char to me".into(),
                            ],
                        )?;
                        continue;
                    }
                };

                // Check if op required
                if !is_op {
                    ctx.reply(
                        ERR_CHANOPRIVSNEEDED,
                        vec![
                            channel_name.to_string(),
                            "You're not channel operator".into(),
                        ],
                    )?;
                    return Err(Error::NotChannelOperator(channel_name.to_string()));
                }

                // Apply the mode change
                if apply_mode_change(ctx, &mut channel, &setter, &change, mode)? {
                    applied_changes.changes.push(change.clone());
                }
            }

            // Broadcast applied changes
            if !applied_changes.is_empty() {
                let mode_msg = Message::with_prefix(
                    ctx.client.prefix()?,
                    Command::Mode {
                        target: channel_name.to_string(),
                        modes: Some(applied_changes.to_string()),
                        params: vec![],
                    },
                );
                ctx.state.broadcast_to_channel(&channel, mode_msg, None);
            }
        }
    }

    Ok(())
}

/// Apply a single mode change to a channel.
fn apply_mode_change(
    ctx: &HandlerContext,
    channel: &mut Channel,
    setter: &str,
    change: &irc_proto::ModeChange,
    mode: ChannelMode,
) -> Result<bool> {
    match mode {
        ChannelMode::InviteOnly => {
            channel.modes.invite_only = change.adding;
            Ok(true)
        }
        ChannelMode::Moderated => {
            channel.modes.moderated = change.adding;
            Ok(true)
        }
        ChannelMode::NoExternal => {
            channel.modes.no_external = change.adding;
            Ok(true)
        }
        ChannelMode::Secret => {
            channel.modes.secret = change.adding;
            Ok(true)
        }
        ChannelMode::Private => {
            // Treat private same as secret for now
            channel.modes.secret = change.adding;
            Ok(true)
        }
        ChannelMode::TopicLock => {
            channel.modes.topic_locked = change.adding;
            Ok(true)
        }
        ChannelMode::Key => {
            if change.adding {
                if let Some(ref key) = change.param {
                    channel.modes.key = Some(key.clone());
                    return Ok(true);
                }
            } else {
                channel.modes.key = None;
                return Ok(true);
            }
            Ok(false)
        }
        ChannelMode::Limit => {
            if change.adding {
                if let Some(ref param) = change.param {
                    if let Ok(limit) = param.parse::<u32>() {
                        channel.modes.limit = Some(limit);
                        return Ok(true);
                    }
                }
            } else {
                channel.modes.limit = None;
                return Ok(true);
            }
            Ok(false)
        }
        ChannelMode::Operator => {
            if let Some(ref nick) = change.param {
                if let Some(target) = ctx.state.find_client_by_nick(nick) {
                    if let Some(status) = channel.get_member_status_mut(target.id) {
                        status.operator = change.adding;
                        return Ok(true);
                    } else {
                        ctx.reply(
                            ERR_USERNOTINCHANNEL,
                            vec![
                                nick.clone(),
                                channel.name.clone(),
                                "They aren't on that channel".into(),
                            ],
                        )?;
                    }
                } else {
                    ctx.reply(
                        ERR_NOSUCHNICK,
                        vec![nick.clone(), "No such nick/channel".into()],
                    )?;
                }
            }
            Ok(false)
        }
        ChannelMode::Voice => {
            if let Some(ref nick) = change.param {
                if let Some(target) = ctx.state.find_client_by_nick(nick) {
                    if let Some(status) = channel.get_member_status_mut(target.id) {
                        status.voice = change.adding;
                        return Ok(true);
                    } else {
                        ctx.reply(
                            ERR_USERNOTINCHANNEL,
                            vec![
                                nick.clone(),
                                channel.name.clone(),
                                "They aren't on that channel".into(),
                            ],
                        )?;
                    }
                } else {
                    ctx.reply(
                        ERR_NOSUCHNICK,
                        vec![nick.clone(), "No such nick/channel".into()],
                    )?;
                }
            }
            Ok(false)
        }
        ChannelMode::Ban => {
            if let Some(ref mask) = change.param {
                if change.adding {
                    channel.add_ban(mask.clone(), setter.to_string());
                } else {
                    channel.remove_ban(mask);
                }
                return Ok(true);
            }
            Ok(false)
        }
        ChannelMode::Exception => {
            if let Some(ref mask) = change.param {
                if change.adding {
                    channel.add_exception(mask.clone(), setter.to_string());
                } else {
                    channel.remove_exception(mask);
                }
                return Ok(true);
            }
            Ok(false)
        }
        ChannelMode::InviteException => {
            if let Some(ref mask) = change.param {
                if change.adding {
                    channel.add_invite_exception(mask.clone(), setter.to_string());
                } else {
                    channel.remove_invite_exception(mask);
                }
                return Ok(true);
            }
            Ok(false)
        }
    }
}

/// Handle KICK command.
pub fn handle_kick(
    ctx: &HandlerContext,
    channel_name: &str,
    users: &[String],
    comment: Option<&str>,
) -> Result<()> {
    let channel_arc = match ctx.state.get_channel(channel_name) {
        Some(c) => c,
        None => {
            ctx.reply(
                ERR_NOSUCHCHANNEL,
                vec![channel_name.to_string(), "No such channel".into()],
            )?;
            return Err(Error::NoSuchChannel(channel_name.to_string()));
        }
    };

    let client_id = ctx.client.id;

    // Check if kicker is on channel and is op
    {
        let channel = channel_arc.read_lock("channel")?;
        if !channel.is_member(client_id) {
            ctx.reply(
                ERR_NOTONCHANNEL,
                vec![channel_name.to_string(), "You're not on that channel".into()],
            )?;
            return Err(Error::NotOnChannel(channel_name.to_string()));
        }
        if !channel.is_operator(client_id) {
            ctx.reply(
                ERR_CHANOPRIVSNEEDED,
                vec![
                    channel_name.to_string(),
                    "You're not channel operator".into(),
                ],
            )?;
            return Err(Error::NotChannelOperator(channel_name.to_string()));
        }
    }

    let kicker_nick = ctx.client.nickname()?.unwrap_or_else(|| "*".to_string());

    for nick in users {
        let target = match ctx.state.find_client_by_nick(nick) {
            Some(c) => c,
            None => {
                ctx.reply(
                    ERR_NOSUCHNICK,
                    vec![nick.clone(), "No such nick/channel".into()],
                )?;
                continue;
            }
        };

        let mut channel = channel_arc.write_lock("channel")?;
        if !channel.is_member(target.id) {
            ctx.reply(
                ERR_USERNOTINCHANNEL,
                vec![
                    nick.clone(),
                    channel_name.to_string(),
                    "They aren't on that channel".into(),
                ],
            )?;
            continue;
        }

        // Build KICK message
        let kick_comment = comment.unwrap_or(&kicker_nick);
        let kick_msg = Message::with_prefix(
            ctx.client.prefix()?,
            Command::Kick {
                channel: channel_name.to_string(),
                users: vec![nick.clone()],
                comment: Some(kick_comment.to_string()),
            },
        );

        // Broadcast KICK to channel
        ctx.state.broadcast_to_channel(&channel, kick_msg, None);

        // Remove from channel
        channel.remove_member(target.id);
        target.leave_channel(channel_name)?;

        tracing::debug!(
            kicker = %kicker_nick,
            target = %nick,
            channel = %channel_name,
            "User kicked from channel"
        );
    }

    // Clean up empty channel
    let should_remove = channel_arc.read_lock("channel")?.member_count() == 0;
    if should_remove {
        ctx.state.remove_channel(channel_name);
    }

    Ok(())
}

/// Handle INVITE command.
pub fn handle_invite(ctx: &HandlerContext, nickname: &str, channel_name: &str) -> Result<()> {
    let client_id = ctx.client.id;
    let inviter_nick = ctx.client.nickname()?.unwrap_or_else(|| "*".to_string());

    // Find target user
    let target = match ctx.state.find_client_by_nick(nickname) {
        Some(c) => c,
        None => {
            ctx.reply(
                ERR_NOSUCHNICK,
                vec![nickname.to_string(), "No such nick/channel".into()],
            )?;
            return Err(Error::NoSuchNick(nickname.to_string()));
        }
    };

    // Get or check channel
    let channel_arc = match ctx.state.get_channel(channel_name) {
        Some(c) => c,
        None => {
            // Channel doesn't exist - can still send invite for when it's created
            // Send RPL_INVITING to inviter
            ctx.reply(
                RPL_INVITING,
                vec![nickname.to_string(), channel_name.to_string()],
            )?;

            // Send INVITE to target
            let invite_msg = Message::with_prefix(
                ctx.client.prefix()?,
                Command::Invite {
                    nickname: nickname.to_string(),
                    channel: channel_name.to_string(),
                },
            );
            target.send(invite_msg)?;

            return Ok(());
        }
    };

    {
        let channel = channel_arc.read_lock("channel")?;

        // Check if inviter is on channel
        if !channel.is_member(client_id) {
            ctx.reply(
                ERR_NOTONCHANNEL,
                vec![channel_name.to_string(), "You're not on that channel".into()],
            )?;
            return Err(Error::NotOnChannel(channel_name.to_string()));
        }

        // For +i channels, require op
        if channel.modes.invite_only && !channel.is_operator(client_id) {
            ctx.reply(
                ERR_CHANOPRIVSNEEDED,
                vec![
                    channel_name.to_string(),
                    "You're not channel operator".into(),
                ],
            )?;
            return Err(Error::NotChannelOperator(channel_name.to_string()));
        }

        // Check if target is already on channel
        if channel.is_member(target.id) {
            ctx.reply(
                ERR_USERONCHANNEL,
                vec![
                    nickname.to_string(),
                    channel_name.to_string(),
                    "is already on channel".into(),
                ],
            )?;
            return Err(Error::UserOnChannel(
                nickname.to_string(),
                channel_name.to_string(),
            ));
        }
    }

    // Add invite exception for this user
    {
        let mut channel = channel_arc.write_lock("channel")?;
        let target_mask = target.hostmask()?;
        channel.add_invite_exception(target_mask, inviter_nick.clone());
    }

    // Send RPL_INVITING to inviter
    ctx.reply(
        RPL_INVITING,
        vec![nickname.to_string(), channel_name.to_string()],
    )?;

    // Send INVITE to target
    let invite_msg = Message::with_prefix(
        ctx.client.prefix()?,
        Command::Invite {
            nickname: nickname.to_string(),
            channel: channel_name.to_string(),
        },
    );
    target.send(invite_msg)?;

    // Check if target is away
    if let Some(away_msg) = target.away_message()? {
        ctx.reply(
            RPL_AWAY,
            vec![nickname.to_string(), away_msg],
        )?;
    }

    tracing::debug!(
        inviter = %inviter_nick,
        target = %nickname,
        channel = %channel_name,
        "User invited to channel"
    );

    Ok(())
}

/// Send topic information to a client.
fn send_topic_to_client(ctx: &HandlerContext, channel: &Channel) -> Result<()> {
    match &channel.topic {
        Some(topic) => {
            ctx.reply(
                RPL_TOPIC,
                vec![channel.name.clone(), topic.clone()],
            )?;
            if let (Some(set_by), Some(set_at)) = (&channel.topic_set_by, &channel.topic_set_at) {
                ctx.reply(
                    RPL_TOPICWHOTIME,
                    vec![
                        channel.name.clone(),
                        set_by.clone(),
                        set_at.timestamp().to_string(),
                    ],
                )?;
            }
        }
        None => {
            ctx.reply(
                RPL_NOTOPIC,
                vec![channel.name.clone(), "No topic is set".into()],
            )?;
        }
    }
    Ok(())
}

/// Send names list to a client.
fn send_names_to_client(ctx: &HandlerContext, channel: &Channel) -> Result<()> {
    // Build names list with prefixes
    let mut names = Vec::new();
    for (client_id, status) in &channel.members {
        if let Some(client) = ctx.state.clients.get(client_id) {
            if let Some(nick) = client.nickname()? {
                let prefix = status.prefix_char().map(|c| c.to_string()).unwrap_or_default();
                names.push(format!("{}{}", prefix, nick));
            }
        }
    }

    // Channel symbol: @ for secret, * for private, = for public
    let symbol = if channel.modes.secret {
        "@"
    } else {
        "="
    };

    // Send names (may need to split if too long)
    let names_str = names.join(" ");
    ctx.reply(
        RPL_NAMREPLY,
        vec![symbol.into(), channel.name.clone(), names_str],
    )?;

    ctx.reply(
        RPL_ENDOFNAMES,
        vec![channel.name.clone(), "End of /NAMES list".into()],
    )?;
    Ok(())
}
