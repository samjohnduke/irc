//! Server query command handlers (MOTD, LUSERS, VERSION, TIME, ADMIN, INFO, STATS).

use irc_proto::{errors::*, replies::*};

use super::HandlerContext;
use crate::error::Result;
use crate::reply::ReplyBuilder;

/// Handle MOTD command - request message of the day.
pub fn handle_motd(ctx: &HandlerContext) -> Result<()> {
    // Spawn async task to send MOTD (it needs to read the async MOTD lock)
    let client = ctx.client.clone();
    let state = ctx.state.clone();

    tokio::spawn(async move {
        let config = &state.config;
        let rb = ReplyBuilder::new(&config.server_name, &client);

        let motd = state.motd.read().await;

        if let Some(ref lines) = *motd {
            // 375 RPL_MOTDSTART
            rb.send(&client, RPL_MOTDSTART, vec![format!("- {} Message of the Day -", config.server_name)]);

            // 372 RPL_MOTD for each line
            for line in lines {
                rb.send(&client, RPL_MOTD, vec![format!("- {}", line)]);
            }

            // 376 RPL_ENDOFMOTD
            rb.send(&client, RPL_ENDOFMOTD, vec!["End of /MOTD command.".into()]);
        } else {
            // 422 ERR_NOMOTD
            rb.send(&client, ERR_NOMOTD, vec!["MOTD File is missing".into()]);
        }
    });

    Ok(())
}

/// Handle LUSERS command - request user statistics.
pub fn handle_lusers(ctx: &HandlerContext) -> Result<()> {
    let config = &ctx.state.config;
    let rb = ReplyBuilder::new(&config.server_name, ctx.client);

    let total = ctx.state.client_count();
    let invisible = ctx.state.invisible_count()?;
    let visible = total.saturating_sub(invisible);
    let operators = ctx.state.operator_count()?;
    let channels = ctx.state.channel_count();
    let unknown = total.saturating_sub(ctx.state.registered_count()?);

    // 251 RPL_LUSERCLIENT
    rb.send(
        ctx.client,
        RPL_LUSERCLIENT,
        vec![format!(
            "There are {} users and {} invisible on 1 servers",
            visible, invisible
        )],
    );

    // 252 RPL_LUSEROP
    if operators > 0 {
        rb.send(
            ctx.client,
            RPL_LUSEROP,
            vec![operators.to_string(), "operator(s) online".into()],
        );
    }

    // 253 RPL_LUSERUNKNOWN
    if unknown > 0 {
        rb.send(
            ctx.client,
            RPL_LUSERUNKNOWN,
            vec![unknown.to_string(), "unknown connection(s)".into()],
        );
    }

    // 254 RPL_LUSERCHANNELS
    if channels > 0 {
        rb.send(
            ctx.client,
            RPL_LUSERCHANNELS,
            vec![channels.to_string(), "channels formed".into()],
        );
    }

    // 255 RPL_LUSERME
    rb.send(
        ctx.client,
        RPL_LUSERME,
        vec![format!("I have {} clients and 0 servers", total)],
    );

    // 265 RPL_LOCALUSERS
    rb.send(
        ctx.client,
        RPL_LOCALUSERS,
        vec![
            total.to_string(),
            total.to_string(),
            format!("Current local users {}, max {}", total, total),
        ],
    );

    // 266 RPL_GLOBALUSERS
    rb.send(
        ctx.client,
        RPL_GLOBALUSERS,
        vec![
            total.to_string(),
            total.to_string(),
            format!("Current global users {}, max {}", total, total),
        ],
    );

    Ok(())
}

/// Handle VERSION command - request server version.
pub fn handle_version(ctx: &HandlerContext) -> Result<()> {
    let server_name = &ctx.state.config.server_name;

    // 351 RPL_VERSION
    // <version>.<debuglevel> <server> :<comments>
    ctx.reply(
        RPL_VERSION,
        vec![
            "irc-server-0.1.0".into(),
            server_name.clone(),
            "A modern IRC server implementation in Rust".into(),
        ],
    )?;

    Ok(())
}

/// Handle TIME command - request server time.
pub fn handle_time(ctx: &HandlerContext) -> Result<()> {
    use chrono::Utc;

    let server_name = &ctx.state.config.server_name;
    let now = Utc::now();

    // 391 RPL_TIME
    // <server> :<time string>
    ctx.reply(
        RPL_TIME,
        vec![
            server_name.clone(),
            now.format("%A %B %d %Y -- %H:%M:%S %z").to_string(),
        ],
    )?;

    Ok(())
}

/// Handle ADMIN command - request admin info.
pub fn handle_admin(ctx: &HandlerContext) -> Result<()> {
    let config = &ctx.state.config;
    let admin = &config.admin;

    // Check if any admin info is configured
    if admin.location1.is_none() && admin.location2.is_none() && admin.email.is_none() {
        ctx.reply(
            ERR_NOADMININFO,
            vec![
                config.server_name.clone(),
                "No administrative info available".into(),
            ],
        )?;
        return Ok(());
    }

    // 256 RPL_ADMINME
    ctx.reply(
        RPL_ADMINME,
        vec![
            config.server_name.clone(),
            "Administrative info".into(),
        ],
    )?;

    // 257 RPL_ADMINLOC1
    if let Some(ref loc1) = admin.location1 {
        ctx.reply(RPL_ADMINLOC1, vec![loc1.clone()])?;
    }

    // 258 RPL_ADMINLOC2
    if let Some(ref loc2) = admin.location2 {
        ctx.reply(RPL_ADMINLOC2, vec![loc2.clone()])?;
    }

    // 259 RPL_ADMINEMAIL
    if let Some(ref email) = admin.email {
        ctx.reply(RPL_ADMINEMAIL, vec![email.clone()])?;
    }

    Ok(())
}

/// Handle INFO command - request server info.
pub fn handle_info(ctx: &HandlerContext) -> Result<()> {
    let info_lines = [
        "irc-server - A modern IRC server in Rust",
        "",
        "Version: 0.1.0",
        "Author: IRC Server Development Team",
        "",
        "This server implements IRC protocol RFC 2812",
        "with support for common IRCv3 extensions.",
        "",
        "Features:",
        "  - TLS encryption",
        "  - Channel modes: +imnstklbeo",
        "  - User modes: +iow",
        "  - Operator commands",
    ];

    // 371 RPL_INFO for each line
    for line in &info_lines {
        ctx.reply(RPL_INFO, vec![(*line).to_string()])?;
    }

    // 374 RPL_ENDOFINFO
    ctx.reply(RPL_ENDOFINFO, vec!["End of /INFO list.".into()])?;

    Ok(())
}

/// Handle STATS command - request server statistics.
pub fn handle_stats(ctx: &HandlerContext, query: Option<char>) -> Result<()> {
    match query {
        Some('u') | Some('U') => {
            // Uptime statistics
            let uptime = chrono::Utc::now() - ctx.state.created_at;
            let days = uptime.num_days();
            let hours = uptime.num_hours() % 24;
            let minutes = uptime.num_minutes() % 60;
            let seconds = uptime.num_seconds() % 60;

            // 242 RPL_STATSUPTIME
            ctx.reply(
                RPL_STATSUPTIME,
                vec![format!(
                    "Server Up {} days {}:{:02}:{:02}",
                    days, hours, minutes, seconds
                )],
            )?;
        }
        Some('m') | Some('M') => {
            // Command statistics (stub - we don't track command usage yet)
            // 212 RPL_STATSCOMMANDS would go here
        }
        Some('l') | Some('L') => {
            // Link statistics (stub - single server for now)
            // 211 RPL_STATSLINKINFO would go here
        }
        _ => {
            // Unknown or no query - just send end of stats
        }
    }

    // 219 RPL_ENDOFSTATS
    let query_char = query.map(|c| c.to_string()).unwrap_or_else(|| "*".into());
    ctx.reply(
        RPL_ENDOFSTATS,
        vec![query_char, "End of /STATS report".into()],
    )?;

    Ok(())
}
