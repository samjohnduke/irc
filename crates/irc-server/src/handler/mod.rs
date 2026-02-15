//! Command handlers.
//!
//! This module contains handlers for all IRC commands, organized by category.

mod cap;
mod channel;
mod help;
mod history;
mod messaging;
mod misc;
mod monitor;
mod oper;
mod query;
mod read_marker;
mod register;
mod registration;
mod sasl;
mod server;

use std::sync::Arc;

use irc_proto::{Command, Message, Prefix, is_channel};

use crate::error::{Error, Result};
use crate::lock::RwLockExt;
use crate::state::{Client, ServerState};

pub use channel::{
    handle_channel_mode, handle_invite, handle_join, handle_kick, handle_list, handle_names,
    handle_part, handle_topic,
};
pub use help::handle_help;
pub use history::handle_chathistory;
pub use messaging::{handle_notice, handle_privmsg};
pub use misc::{handle_away, handle_ping, handle_pong, handle_setname};
pub use monitor::handle_monitor;
pub use oper::{
    handle_die, handle_gline, handle_kill, handle_kline, handle_oper, handle_rehash,
    handle_restart, handle_ungline, handle_unkline, handle_unzline, handle_wallops, handle_zline,
};
pub use query::{handle_who, handle_whois, handle_whowas};
pub use registration::{handle_nick, handle_pass, handle_quit, handle_user};
pub use server::{
    handle_admin, handle_info, handle_lusers, handle_motd, handle_stats, handle_time,
    handle_version,
};

/// Context for command handlers.
pub struct HandlerContext<'a> {
    /// The client that sent the message.
    pub client: &'a Arc<Client>,
    /// Server state.
    pub state: &'a Arc<ServerState>,
    /// Label tag from incoming message (for labeled-response capability).
    pub label: Option<String>,
}

impl<'a> HandlerContext<'a> {
    /// Create a new handler context.
    pub fn new(client: &'a Arc<Client>, state: &'a Arc<ServerState>) -> Self {
        Self {
            client,
            state,
            label: None,
        }
    }

    /// Create a new handler context with a label.
    pub fn with_label(
        client: &'a Arc<Client>,
        state: &'a Arc<ServerState>,
        label: Option<String>,
    ) -> Self {
        Self {
            client,
            state,
            label,
        }
    }

    /// Get the server name.
    pub fn server_name(&self) -> &str {
        &self.state.config.server_name
    }

    /// Get the server prefix.
    pub fn server_prefix(&self) -> Prefix {
        Prefix::from_server(&self.state.config.server_name)
    }

    /// Send a numeric reply to the client.
    pub fn reply(&self, code: u16, params: Vec<String>) -> Result<()> {
        let target = self.client.nickname()?.unwrap_or_else(|| "*".to_string());
        let mut msg = Message::with_prefix(
            self.server_prefix(),
            Command::Numeric {
                code,
                target,
                params,
            },
        );
        // Add label tag if client supports labeled-response
        if let Some(ref label) = self.label
            && self.client.has_cap("labeled-response")?
        {
            let tags = msg.tags.get_or_insert_with(irc_proto::Tags::new);
            tags.set("label", label);
        }
        self.client.send(msg)?;
        Ok(())
    }

    /// Send an error reply to the client.
    pub fn error(&self, error: &Error) -> Result<()> {
        if let Some(code) = error.numeric_code() {
            let message = error.to_string();
            self.reply(code, vec![message])?;
        }
        Ok(())
    }

    /// Check if the client is registered, returning an error if not.
    pub fn require_registered(&self) -> Result<()> {
        if self.client.is_registered()? {
            Ok(())
        } else {
            let err = Error::NotRegistered;
            self.error(&err)?;
            Err(err)
        }
    }

    /// Send a message from the server to the client.
    pub fn send_server_message(&self, command: Command) -> Result<()> {
        let mut msg = Message::with_prefix(self.server_prefix(), command);
        // Add label tag if client supports labeled-response
        if let Some(ref label) = self.label
            && self.client.has_cap("labeled-response")?
        {
            let tags = msg.tags.get_or_insert_with(irc_proto::Tags::new);
            tags.set("label", label);
        }
        self.client.send(msg)?;
        Ok(())
    }
}

/// Main message handler - dispatches to specific command handlers.
pub async fn handle_message(
    client: &Arc<Client>,
    state: &Arc<ServerState>,
    message: Message,
) -> Result<()> {
    // Extract label tag from incoming message for labeled-response
    let label = message
        .tags
        .as_ref()
        .and_then(|tags| tags.get("label").map(|s| s.to_string()));
    let ctx = HandlerContext::with_label(client, state, label);

    match &message.command {
        // Registration commands (allowed before registration)
        Command::Pass { password } => handle_pass(&ctx, password),
        Command::Nick { nickname } => handle_nick(&ctx, nickname),
        Command::User {
            username, realname, ..
        } => handle_user(&ctx, username, realname),
        Command::Quit { message: quit_msg } => handle_quit(&ctx, quit_msg.as_deref()),

        // Connection commands (allowed before registration)
        Command::Ping { server1, server2 } => handle_ping(&ctx, server1, server2.as_deref()),
        Command::Pong { .. } => handle_pong(&ctx),

        // CAP negotiation (IRCv3)
        Command::Cap { subcommand, params } => cap::handle_cap(&ctx, subcommand, params),

        // SASL authentication (IRCv3)
        Command::Authenticate { data } => sasl::handle_authenticate(&ctx, data),

        // Account registration (draft/account-registration, allowed before connect)
        Command::Register {
            account,
            email,
            password,
        } => register::handle_register(&ctx, account, email, password),

        // Commands requiring registration
        _ => {
            ctx.require_registered()?;

            match &message.command {
                Command::Privmsg { target, message } => handle_privmsg(&ctx, target, message),
                Command::Notice { target, message } => handle_notice(&ctx, target, message),
                Command::Away { message: away_msg } => handle_away(&ctx, away_msg.as_deref()),

                // Channel commands
                Command::Join { channels } => handle_join(&ctx, channels),
                Command::Part {
                    channels,
                    message: part_msg,
                } => handle_part(&ctx, channels, part_msg.as_deref()),
                Command::Topic { channel, topic } => handle_topic(&ctx, channel, topic.as_deref()),
                Command::Names { channels } => handle_names(&ctx, channels.as_deref()),
                Command::List { channels } => handle_list(&ctx, channels.as_deref()),
                Command::Kick {
                    channel,
                    users,
                    comment,
                } => handle_kick(&ctx, channel, users, comment.as_deref()),
                Command::Invite { nickname, channel } => handle_invite(&ctx, nickname, channel),

                // Server query commands
                Command::Motd { .. } => handle_motd(&ctx),
                Command::Lusers { .. } => handle_lusers(&ctx),
                Command::Version { .. } => handle_version(&ctx),
                Command::Time { .. } => handle_time(&ctx),
                Command::Admin { .. } => handle_admin(&ctx),
                Command::Info { .. } => handle_info(&ctx),
                Command::Stats { query, .. } => handle_stats(&ctx, *query),

                // User query commands
                Command::Who {
                    mask,
                    operators_only,
                } => handle_who(&ctx, mask, *operators_only),
                Command::Whois { nicknames, .. } => handle_whois(&ctx, nicknames),
                Command::Whowas {
                    nickname, count, ..
                } => handle_whowas(&ctx, nickname, *count),

                // Operator commands
                Command::Oper { name, password } => handle_oper(&ctx, name, password),
                Command::Kill { nickname, comment } => handle_kill(&ctx, nickname, comment),
                Command::Wallops {
                    message: wallops_msg,
                } => handle_wallops(&ctx, wallops_msg),
                Command::Rehash => handle_rehash(&ctx),
                Command::Restart => handle_restart(&ctx),
                Command::Die => handle_die(&ctx),
                Command::Kline {
                    duration,
                    mask,
                    reason,
                } => handle_kline(&ctx, duration.as_deref(), mask, reason.as_deref()),
                Command::Unkline { mask } => handle_unkline(&ctx, mask),
                Command::Gline {
                    duration,
                    mask,
                    reason,
                } => handle_gline(&ctx, duration.as_deref(), mask, reason.as_deref()),
                Command::Ungline { mask } => handle_ungline(&ctx, mask),
                Command::Zline {
                    duration,
                    mask,
                    reason,
                } => handle_zline(&ctx, duration.as_deref(), mask, reason.as_deref()),
                Command::Unzline { mask } => handle_unzline(&ctx, mask),

                // MONITOR command
                Command::Monitor {
                    subcommand,
                    targets,
                } => handle_monitor(&ctx, *subcommand, targets.as_deref()),

                // CHATHISTORY command
                Command::Chathistory {
                    subcommand,
                    target,
                    params,
                } => handle_chathistory(&ctx, subcommand, target, params),

                // HELP command
                Command::Help { topic } => handle_help(&ctx, topic.as_deref()),

                // MARKREAD command (draft/read-marker)
                Command::Markread { target, timestamp } => {
                    read_marker::handle_markread(&ctx, target, timestamp.as_deref())
                }

                Command::Mode {
                    target,
                    modes,
                    params,
                } => {
                    if is_channel(target) {
                        handle_channel_mode(&ctx, target, modes.as_deref(), params)
                    } else {
                        // User mode - must be targeting self
                        let client_nick = client.nickname()?.unwrap_or_default();
                        if !irc_proto::irc_eq(target, &client_nick) {
                            ctx.reply(
                                irc_proto::errors::ERR_USERSDONTMATCH,
                                vec!["Can't change mode for other users".into()],
                            )?;
                            return Ok(());
                        }

                        if let Some(mode_str) = modes {
                            // Parse and apply user mode changes
                            let mut adding = true;
                            let mut applied = String::new();
                            let mut modes_guard = client.modes.write_lock("modes")?;

                            for c in mode_str.chars() {
                                match c {
                                    '+' => adding = true,
                                    '-' => adding = false,
                                    'i' => {
                                        modes_guard.invisible = adding;
                                        applied.push(c);
                                    }
                                    'w' => {
                                        modes_guard.wallops = adding;
                                        applied.push(c);
                                    }
                                    'B' => {
                                        // Bot mode - only allow if client has 'bot' capability
                                        if client.has_cap("bot")? {
                                            modes_guard.bot = adding;
                                            applied.push(c);
                                        }
                                    }
                                    'o' if !adding => {
                                        // Can de-oper yourself but not op yourself
                                        modes_guard.operator = false;
                                        applied.push(c);
                                    }
                                    _ => {} // Silently ignore unknown modes
                                }
                            }

                            // Send MODE reply to confirm changes
                            if !applied.is_empty() {
                                let prefix_str = if adding { "+" } else { "-" };
                                let mode_response = format!("{}{}", prefix_str, applied);
                                let msg = Message::with_prefix(
                                    client.prefix()?,
                                    Command::Mode {
                                        target: client_nick,
                                        modes: Some(mode_response),
                                        params: vec![],
                                    },
                                );
                                let _ = client.send(msg);
                            }
                        } else {
                            // Query user modes
                            let mode_str = client.modes.read_lock("modes")?.to_string();
                            ctx.reply(irc_proto::replies::RPL_UMODEIS, vec![mode_str])?;
                        }
                        Ok(())
                    }
                }

                Command::Userhost { nicknames } => {
                    // Basic USERHOST implementation
                    let mut replies = Vec::new();
                    for nick in nicknames.iter().take(5) {
                        if let Some(target) = state.find_client_by_nick(nick) {
                            let away_marker = if target.is_away()? { "-" } else { "+" };
                            let oper_marker = if target.modes.read_lock("modes")?.operator {
                                "*"
                            } else {
                                ""
                            };
                            replies.push(format!(
                                "{}{}={}{}@{}",
                                target.nickname()?.unwrap_or_default(),
                                oper_marker,
                                away_marker,
                                target.username()?.unwrap_or_default(),
                                target.hostname()?
                            ));
                        }
                    }
                    ctx.reply(302, vec![replies.join(" ")])?;
                    Ok(())
                }

                Command::Ison { nicknames } => {
                    // Basic ISON implementation
                    let online: Vec<_> = nicknames
                        .iter()
                        .filter(|nick| state.find_client_by_nick(nick).is_some())
                        .cloned()
                        .collect();
                    ctx.reply(303, vec![online.join(" ")])?;
                    Ok(())
                }

                // SETNAME (IRCv3)
                Command::Setname { realname } => handle_setname(&ctx, realname),

                Command::Unknown { command, .. } => {
                    ctx.reply(
                        irc_proto::errors::ERR_UNKNOWNCOMMAND,
                        vec![command.clone(), "Unknown command".into()],
                    )?;
                    Ok(())
                }

                _ => {
                    ctx.reply(
                        irc_proto::errors::ERR_UNKNOWNCOMMAND,
                        vec![
                            message.command.name().to_string(),
                            "Command not implemented".into(),
                        ],
                    )?;
                    Ok(())
                }
            }
        }
    }
}
