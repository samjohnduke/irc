//! Command handlers.
//!
//! This module contains handlers for all IRC commands, organized by category.

mod cap;
mod channel;
mod messaging;
mod misc;
mod oper;
mod query;
mod registration;
mod sasl;
mod server;

use std::sync::Arc;

use irc_proto::{is_channel, Command, Message, Prefix};

use crate::error::{Error, Result};
use crate::lock::RwLockExt;
use crate::state::{Client, ServerState};

pub use channel::{
    handle_channel_mode, handle_invite, handle_join, handle_kick, handle_list, handle_names,
    handle_part, handle_topic,
};
pub use messaging::{handle_notice, handle_privmsg};
pub use misc::{handle_away, handle_ping, handle_pong, handle_setname};
pub use oper::{handle_kill, handle_oper, handle_wallops};
pub use query::{handle_who, handle_whois, handle_whowas};
pub use registration::{handle_nick, handle_pass, handle_quit, handle_user};
pub use server::{handle_admin, handle_info, handle_lusers, handle_motd, handle_stats, handle_time, handle_version};

/// Context for command handlers.
pub struct HandlerContext<'a> {
    /// The client that sent the message.
    pub client: &'a Arc<Client>,
    /// Server state.
    pub state: &'a Arc<ServerState>,
}

impl<'a> HandlerContext<'a> {
    /// Create a new handler context.
    pub fn new(client: &'a Arc<Client>, state: &'a Arc<ServerState>) -> Self {
        Self { client, state }
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
        self.client.send_numeric(self.server_name(), code, params)?;
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
        let msg = Message::with_prefix(self.server_prefix(), command);
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
    let ctx = HandlerContext::new(client, state);

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

        // Commands requiring registration
        _ => {
            ctx.require_registered()?;

            match &message.command {
                Command::Privmsg { target, message } => handle_privmsg(&ctx, target, message),
                Command::Notice { target, message } => handle_notice(&ctx, target, message),
                Command::Away { message: away_msg } => handle_away(&ctx, away_msg.as_deref()),

                // Channel commands
                Command::Join { channels } => handle_join(&ctx, channels),
                Command::Part { channels, message: part_msg } => {
                    handle_part(&ctx, channels, part_msg.as_deref())
                }
                Command::Topic { channel, topic } => {
                    handle_topic(&ctx, channel, topic.as_deref())
                }
                Command::Names { channels } => {
                    handle_names(&ctx, channels.as_deref())
                }
                Command::List { channels } => {
                    handle_list(&ctx, channels.as_deref())
                }
                Command::Kick { channel, users, comment } => {
                    handle_kick(&ctx, channel, users, comment.as_deref())
                }
                Command::Invite { nickname, channel } => {
                    handle_invite(&ctx, nickname, channel)
                }

                // Server query commands
                Command::Motd { .. } => handle_motd(&ctx),
                Command::Lusers { .. } => handle_lusers(&ctx),
                Command::Version { .. } => handle_version(&ctx),
                Command::Time { .. } => handle_time(&ctx),
                Command::Admin { .. } => handle_admin(&ctx),
                Command::Info { .. } => handle_info(&ctx),
                Command::Stats { query, .. } => handle_stats(&ctx, *query),

                // User query commands
                Command::Who { mask, operators_only } => {
                    handle_who(&ctx, mask, *operators_only)
                }
                Command::Whois { nicknames, .. } => handle_whois(&ctx, nicknames),
                Command::Whowas { nickname, count, .. } => {
                    handle_whowas(&ctx, nickname, *count)
                }

                // Operator commands
                Command::Oper { name, password } => handle_oper(&ctx, name, password),
                Command::Kill { nickname, comment } => handle_kill(&ctx, nickname, comment),
                Command::Wallops { message: wallops_msg } => handle_wallops(&ctx, wallops_msg),

                Command::Mode { target, modes, params } => {
                    if is_channel(target) {
                        handle_channel_mode(&ctx, target, modes.as_deref(), params)
                    } else {
                        // User mode
                        if modes.is_none() {
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

