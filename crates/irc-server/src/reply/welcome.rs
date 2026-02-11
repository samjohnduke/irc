//! Welcome burst (001-005, LUSERS, MOTD).

use std::sync::Arc;

use irc_proto::replies::*;

use super::ReplyBuilder;
use crate::state::{Client, ServerState};

/// Send the registration welcome burst to a client.
pub async fn send_welcome_burst(client: &Arc<Client>, state: &Arc<ServerState>) {
    let config = &state.config;
    let rb = ReplyBuilder::new(&config.server_name, client);
    let nick = client.nickname().unwrap_or_else(|| "*".to_string());
    let user = client.username().unwrap_or_else(|| "unknown".to_string());
    let host = client.hostname();

    // 001 RPL_WELCOME
    rb.send(
        client,
        RPL_WELCOME,
        vec![format!(
            "Welcome to the {} Internet Relay Chat Network {}!{}@{}",
            config.network_name, nick, user, host
        )],
    );

    // 002 RPL_YOURHOST
    rb.send(
        client,
        RPL_YOURHOST,
        vec![format!(
            "Your host is {}, running version irc-server-0.1.0",
            config.server_name
        )],
    );

    // 003 RPL_CREATED
    rb.send(
        client,
        RPL_CREATED,
        vec![format!(
            "This server was created {}",
            state.created_at.format("%a %b %d %Y at %H:%M:%S UTC")
        )],
    );

    // 004 RPL_MYINFO
    // <servername> <version> <available user modes> <available channel modes>
    rb.send(
        client,
        RPL_MYINFO,
        vec![
            config.server_name.clone(),
            "irc-server-0.1.0".into(),
            "iow".into(),   // user modes
            "imnst".into(), // channel modes (basic set)
        ],
    );

    // 005 RPL_ISUPPORT
    send_isupport(client, state).await;

    // LUSERS
    send_lusers(client, state).await;

    // MOTD
    send_motd(client, state).await;
}

/// Send ISUPPORT (005) messages.
async fn send_isupport(client: &Arc<Client>, state: &Arc<ServerState>) {
    let config = &state.config;
    let rb = ReplyBuilder::new(&config.server_name, client);

    // First ISUPPORT line
    let isupport1 = vec![
        format!("NETWORK={}", config.network_name),
        format!("NICKLEN={}", config.limits.max_nick_length),
        format!("CHANNELLEN={}", config.limits.max_channel_length),
        "CASEMAPPING=rfc1459".into(),
        format!("CHANTYPES={}", "#&"),
        format!("PREFIX={}", "(ov)@+"),
        "are supported by this server".into(),
    ];
    rb.send(client, RPL_ISUPPORT, isupport1);

    // Second ISUPPORT line
    let isupport2 = vec![
        format!("CHANMODES={}", "b,k,l,imnst"),
        format!("MODES={}", 4),
        format!("TOPICLEN={}", config.limits.max_topic_length),
        format!("KICKLEN={}", config.limits.max_kick_length),
        format!("AWAYLEN={}", config.limits.max_away_length),
        "are supported by this server".into(),
    ];
    rb.send(client, RPL_ISUPPORT, isupport2);
}

/// Send LUSERS information.
async fn send_lusers(client: &Arc<Client>, state: &Arc<ServerState>) {
    let config = &state.config;
    let rb = ReplyBuilder::new(&config.server_name, client);

    let total = state.client_count();
    let invisible = state.invisible_count();
    let visible = total.saturating_sub(invisible);
    let operators = state.operator_count();
    let channels = state.channel_count();
    let unknown = total.saturating_sub(state.registered_count());

    // 251 RPL_LUSERCLIENT
    rb.send(
        client,
        RPL_LUSERCLIENT,
        vec![format!(
            "There are {} users and {} invisible on 1 servers",
            visible, invisible
        )],
    );

    // 252 RPL_LUSEROP
    if operators > 0 {
        rb.send(
            client,
            RPL_LUSEROP,
            vec![operators.to_string(), "operator(s) online".into()],
        );
    }

    // 253 RPL_LUSERUNKNOWN
    if unknown > 0 {
        rb.send(
            client,
            RPL_LUSERUNKNOWN,
            vec![unknown.to_string(), "unknown connection(s)".into()],
        );
    }

    // 254 RPL_LUSERCHANNELS
    if channels > 0 {
        rb.send(
            client,
            RPL_LUSERCHANNELS,
            vec![channels.to_string(), "channels formed".into()],
        );
    }

    // 255 RPL_LUSERME
    rb.send(
        client,
        RPL_LUSERME,
        vec![format!("I have {} clients and 0 servers", total)],
    );

    // 265 RPL_LOCALUSERS
    rb.send(
        client,
        RPL_LOCALUSERS,
        vec![
            total.to_string(),
            total.to_string(),
            format!("Current local users {}, max {}", total, total),
        ],
    );

    // 266 RPL_GLOBALUSERS
    rb.send(
        client,
        RPL_GLOBALUSERS,
        vec![
            total.to_string(),
            total.to_string(),
            format!("Current global users {}, max {}", total, total),
        ],
    );
}

/// Send MOTD.
async fn send_motd(client: &Arc<Client>, state: &Arc<ServerState>) {
    let config = &state.config;
    let rb = ReplyBuilder::new(&config.server_name, client);

    let motd = state.motd.read().await;

    if let Some(ref lines) = *motd {
        // 375 RPL_MOTDSTART
        rb.send(
            client,
            RPL_MOTDSTART,
            vec![format!("- {} Message of the Day -", config.server_name)],
        );

        // 372 RPL_MOTD for each line
        for line in lines {
            rb.send(client, RPL_MOTD, vec![format!("- {}", line)]);
        }

        // 376 RPL_ENDOFMOTD
        rb.send(client, RPL_ENDOFMOTD, vec!["End of /MOTD command.".into()]);
    } else {
        // 422 ERR_NOMOTD
        rb.send(
            client,
            irc_proto::errors::ERR_NOMOTD,
            vec!["MOTD File is missing".into()],
        );
    }
}
