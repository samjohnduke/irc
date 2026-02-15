//! User query command handlers (WHO, WHOIS, WHOWAS).

use irc_proto::{errors::*, is_channel, replies::*};

use super::HandlerContext;
use crate::error::Result;
use crate::lock::RwLockExt;
use crate::state::matches_mask;

/// Handle WHO command - list users matching mask.
pub fn handle_who(ctx: &HandlerContext, mask: &str, operators_only: bool) -> Result<()> {
    let server_name = &ctx.state.config.server_name;

    if is_channel(mask) {
        // WHO for a channel
        if let Some(channel_lock) = ctx.state.get_channel(mask) {
            let channel = channel_lock.read_lock("channel")?;

            // Check if channel is secret and we're not a member
            let is_member = channel.is_member(ctx.client.id);
            if channel.modes.secret && !is_member {
                // Don't show secret channel members to non-members
                ctx.reply(
                    RPL_ENDOFWHO,
                    vec![mask.to_string(), "End of /WHO list.".into()],
                )?;
                return Ok(());
            }

            for (member_id, status) in &channel.members {
                if let Some(member) = ctx.state.clients.get(member_id) {
                    let is_oper = member.modes.read_lock("modes")?.operator;

                    // Skip if operators_only and not oper
                    if operators_only && !is_oper {
                        continue;
                    }

                    let nick = member.nickname()?.unwrap_or_default();
                    let user = member.username()?.unwrap_or_default();
                    let host = member.hostname()?;
                    let realname = member.realname()?.unwrap_or_default();

                    // Build flags: H=here, G=away, *=oper, @=chanop, +=voice
                    let mut flags = if member.is_away()? { "G" } else { "H" }.to_string();
                    if is_oper {
                        flags.push('*');
                    }
                    if status.operator {
                        flags.push('@');
                    } else if status.voice {
                        flags.push('+');
                    }

                    // 352 RPL_WHOREPLY
                    // <channel> <user> <host> <server> <nick> <H|G>[*][@|+] :<hopcount> <real name>
                    ctx.reply(
                        RPL_WHOREPLY,
                        vec![
                            mask.to_string(),
                            user,
                            host,
                            server_name.clone(),
                            nick,
                            flags,
                            format!("0 {}", realname),
                        ],
                    )?;
                }
            }
        }
    } else {
        // WHO for a mask pattern - iterate all visible clients
        for entry in ctx.state.clients.iter() {
            let client = entry.value();

            // Only include registered users
            if !client.is_registered()? {
                continue;
            }

            let nick = client.nickname()?.unwrap_or_default();
            let user = client.username()?.unwrap_or_default();
            let host = client.hostname()?;
            let realname = client.realname()?.unwrap_or_default();
            let is_oper = client.modes.read_lock("modes")?.operator;

            // Skip if operators_only and not oper
            if operators_only && !is_oper {
                continue;
            }

            // Check if user matches the mask
            let hostmask = format!("{}!{}@{}", nick, user, host);
            if mask != "*"
                && mask != "0"
                && !matches_mask(mask, &hostmask)
                && !matches_mask(mask, &nick)
            {
                continue;
            }

            // Check visibility (invisible users only visible if sharing a channel)
            let is_invisible = client.modes.read_lock("modes")?.invisible;
            if is_invisible && client.id != ctx.client.id {
                // Check if we share a channel with them
                let our_channels = ctx.client.channel_names()?;
                let their_channels = client.channel_names()?;
                let shares_channel = our_channels
                    .iter()
                    .any(|c| their_channels.iter().any(|tc| tc.eq_ignore_ascii_case(c)));
                if !shares_channel {
                    continue;
                }
            }

            // Build flags
            let mut flags = if client.is_away()? { "G" } else { "H" }.to_string();
            if is_oper {
                flags.push('*');
            }

            // 352 RPL_WHOREPLY
            ctx.reply(
                RPL_WHOREPLY,
                vec![
                    "*".to_string(), // No specific channel
                    user,
                    host,
                    server_name.clone(),
                    nick,
                    flags,
                    format!("0 {}", realname),
                ],
            )?;
        }
    }

    // 315 RPL_ENDOFWHO
    ctx.reply(
        RPL_ENDOFWHO,
        vec![mask.to_string(), "End of /WHO list.".into()],
    )?;

    Ok(())
}

/// Handle WHOIS command - query user information.
pub fn handle_whois(ctx: &HandlerContext, nicknames: &[String]) -> Result<()> {
    let server_name = &ctx.state.config.server_name;

    for nick in nicknames {
        if let Some(target) = ctx.state.find_client_by_nick(nick) {
            let target_nick = target.nickname()?.unwrap_or_default();
            let user = target.username()?.unwrap_or_default();
            let host = target.hostname()?;
            let realname = target.realname()?.unwrap_or_default();

            // 311 RPL_WHOISUSER
            // <nick> <user> <host> * :<real name>
            ctx.reply(
                RPL_WHOISUSER,
                vec![target_nick.clone(), user, host, "*".into(), realname],
            )?;

            // 312 RPL_WHOISSERVER
            // <nick> <server> :<server info>
            ctx.reply(
                RPL_WHOISSERVER,
                vec![
                    target_nick.clone(),
                    server_name.clone(),
                    "IRC Server".into(),
                ],
            )?;

            // 313 RPL_WHOISOPERATOR (if oper)
            let modes = target.modes.read_lock("modes")?;
            if modes.operator {
                ctx.reply(
                    RPL_WHOISOPERATOR,
                    vec![target_nick.clone(), "is an IRC operator".into()],
                )?;
            }

            // 335 RPL_WHOISBOT (if bot mode is set)
            if modes.bot {
                ctx.reply(RPL_WHOISBOT, vec![target_nick.clone(), "is a Bot".into()])?;
            }
            drop(modes);

            // 317 RPL_WHOISIDLE
            // <nick> <integer> <integer> :seconds idle, signon time
            let idle_seconds = 0u64; // We don't track idle time yet
            let signon_time = target.connected_at.timestamp() as u64;
            ctx.reply(
                RPL_WHOISIDLE,
                vec![
                    target_nick.clone(),
                    idle_seconds.to_string(),
                    signon_time.to_string(),
                    "seconds idle, signon time".into(),
                ],
            )?;

            // 319 RPL_WHOISCHANNELS
            // <nick> :<channels with prefixes>
            let mut channel_list = Vec::new();
            for channel_entry in ctx.state.channels.iter() {
                let channel = channel_entry.value().read_lock("channel")?;

                // Skip secret channels unless we're a member or it's our own WHOIS
                if channel.modes.secret {
                    let we_are_member = channel.is_member(ctx.client.id);
                    let target_is_us = target.id == ctx.client.id;
                    if !we_are_member && !target_is_us {
                        continue;
                    }
                }

                if let Some(status) = channel.get_member_status(target.id) {
                    let prefix = if status.operator {
                        "@"
                    } else if status.voice {
                        "+"
                    } else {
                        ""
                    };
                    channel_list.push(format!("{}{}", prefix, channel.name));
                }
            }

            if !channel_list.is_empty() {
                ctx.reply(
                    RPL_WHOISCHANNELS,
                    vec![target_nick.clone(), channel_list.join(" ")],
                )?;
            }

            // 301 RPL_AWAY (if away)
            if let Some(away_msg) = target.away_message()? {
                ctx.reply(RPL_AWAY, vec![target_nick.clone(), away_msg])?;
            }

            // 318 RPL_ENDOFWHOIS
            ctx.reply(
                RPL_ENDOFWHOIS,
                vec![target_nick, "End of /WHOIS list.".into()],
            )?;
        } else {
            // 401 ERR_NOSUCHNICK
            ctx.reply(
                ERR_NOSUCHNICK,
                vec![nick.clone(), "No such nick/channel".into()],
            )?;
        }
    }

    Ok(())
}

/// Handle WHOWAS command - query disconnected user info.
pub fn handle_whowas(ctx: &HandlerContext, nickname: &str, count: Option<u32>) -> Result<()> {
    let server_name = &ctx.state.config.server_name;
    let entries = ctx.state.lookup_whowas(nickname, count)?;

    if entries.is_empty() {
        // 406 ERR_WASNOSUCHNICK
        ctx.reply(
            ERR_WASNOSUCHNICK,
            vec![nickname.to_string(), "There was no such nickname".into()],
        )?;
    } else {
        for entry in entries {
            // 314 RPL_WHOWASUSER
            // <nick> <user> <host> * :<real name>
            ctx.reply(
                RPL_WHOWASUSER,
                vec![
                    entry.nickname.clone(),
                    entry.username,
                    entry.hostname,
                    "*".into(),
                    entry.realname,
                ],
            )?;

            // 312 RPL_WHOISSERVER (reused for WHOWAS with quit time)
            ctx.reply(
                RPL_WHOISSERVER,
                vec![
                    entry.nickname,
                    server_name.clone(),
                    entry.quit_time.format("%a %b %d %H:%M:%S %Y").to_string(),
                ],
            )?;
        }
    }

    // 369 RPL_ENDOFWHOWAS
    ctx.reply(
        RPL_ENDOFWHOWAS,
        vec![nickname.to_string(), "End of WHOWAS".into()],
    )?;

    Ok(())
}
