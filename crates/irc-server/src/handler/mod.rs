//! Command handlers.
//!
//! This module contains handlers for all IRC commands, organized by category.

mod messaging;
mod misc;
mod registration;

use std::sync::Arc;

use irc_proto::{Command, Message, Prefix};

use crate::error::{Error, Result};
use crate::state::{Client, ServerState};

pub use messaging::{handle_notice, handle_privmsg};
pub use misc::{handle_away, handle_ping, handle_pong};
pub use registration::{handle_nick, handle_pass, handle_quit, handle_user};

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
    pub fn reply(&self, code: u16, params: Vec<String>) {
        self.client.send_numeric(self.server_name(), code, params);
    }

    /// Send an error reply to the client.
    pub fn error(&self, error: &Error) {
        if let Some(code) = error.numeric_code() {
            let message = error.to_string();
            self.reply(code, vec![message]);
        }
    }

    /// Check if the client is registered, returning an error if not.
    pub fn require_registered(&self) -> Result<()> {
        if self.client.is_registered() {
            Ok(())
        } else {
            let err = Error::NotRegistered;
            self.error(&err);
            Err(err)
        }
    }

    /// Send a message from the server to the client.
    pub fn send_server_message(&self, command: Command) {
        let msg = Message::with_prefix(self.server_prefix(), command);
        self.client.send(msg);
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

        // CAP negotiation (Phase 4 - stub for now)
        Command::Cap { subcommand, params } => handle_cap(&ctx, subcommand, params),

        // Commands requiring registration
        _ => {
            ctx.require_registered()?;

            match &message.command {
                Command::Privmsg { target, message } => handle_privmsg(&ctx, target, message),
                Command::Notice { target, message } => handle_notice(&ctx, target, message),
                Command::Away { message: away_msg } => handle_away(&ctx, away_msg.as_deref()),

                // Phase 2: Channel commands
                Command::Join { .. }
                | Command::Part { .. }
                | Command::Topic { .. }
                | Command::Names { .. }
                | Command::List { .. }
                | Command::Kick { .. }
                | Command::Invite { .. } => {
                    // Stub for Phase 2
                    ctx.reply(
                        irc_proto::errors::ERR_UNKNOWNCOMMAND,
                        vec![
                            message.command.name().to_string(),
                            "Channel commands not yet implemented".into(),
                        ],
                    );
                    Ok(())
                }

                // Phase 3: Query commands
                Command::Who { .. }
                | Command::Whois { .. }
                | Command::Whowas { .. }
                | Command::Motd { .. }
                | Command::Lusers { .. }
                | Command::Version { .. }
                | Command::Time { .. }
                | Command::Admin { .. }
                | Command::Info { .. } => {
                    // Stub for Phase 3
                    ctx.reply(
                        irc_proto::errors::ERR_UNKNOWNCOMMAND,
                        vec![
                            message.command.name().to_string(),
                            "Query commands not yet implemented".into(),
                        ],
                    );
                    Ok(())
                }

                Command::Mode { target, modes, .. } => {
                    // Basic mode stub - just echo back current modes for user
                    if !irc_proto::is_channel(target) {
                        if modes.is_none() {
                            // Query user modes
                            let mode_str = client.modes.read().unwrap().to_string();
                            ctx.reply(irc_proto::replies::RPL_UMODEIS, vec![mode_str]);
                        }
                    }
                    Ok(())
                }

                Command::Userhost { nicknames } => {
                    // Basic USERHOST implementation
                    let mut replies = Vec::new();
                    for nick in nicknames.iter().take(5) {
                        if let Some(target) = state.find_client_by_nick(nick) {
                            let away_marker = if target.is_away() { "-" } else { "+" };
                            let oper_marker = if target.modes.read().unwrap().operator {
                                "*"
                            } else {
                                ""
                            };
                            replies.push(format!(
                                "{}{}={}{}@{}",
                                target.nickname().unwrap_or_default(),
                                oper_marker,
                                away_marker,
                                target.username().unwrap_or_default(),
                                target.hostname()
                            ));
                        }
                    }
                    ctx.reply(302, vec![replies.join(" ")]);
                    Ok(())
                }

                Command::Ison { nicknames } => {
                    // Basic ISON implementation
                    let online: Vec<_> = nicknames
                        .iter()
                        .filter(|nick| state.find_client_by_nick(nick).is_some())
                        .cloned()
                        .collect();
                    ctx.reply(303, vec![online.join(" ")]);
                    Ok(())
                }

                Command::Unknown { command, .. } => {
                    ctx.reply(
                        irc_proto::errors::ERR_UNKNOWNCOMMAND,
                        vec![command.clone(), "Unknown command".into()],
                    );
                    Ok(())
                }

                _ => {
                    ctx.reply(
                        irc_proto::errors::ERR_UNKNOWNCOMMAND,
                        vec![
                            message.command.name().to_string(),
                            "Command not implemented".into(),
                        ],
                    );
                    Ok(())
                }
            }
        }
    }
}

/// Handle CAP command (Phase 4 stub).
fn handle_cap(ctx: &HandlerContext, subcommand: &str, _params: &[String]) -> Result<()> {
    match subcommand.to_uppercase().as_str() {
        "LS" => {
            // Return empty capability list for now
            ctx.send_server_message(Command::Cap {
                subcommand: "LS".into(),
                params: vec!["".into()],
            });
        }
        "REQ" => {
            // Reject all capability requests for now
            ctx.send_server_message(Command::Cap {
                subcommand: "NAK".into(),
                params: vec!["".into()],
            });
        }
        "END" => {
            // Client finished capability negotiation
        }
        _ => {}
    }
    Ok(())
}
