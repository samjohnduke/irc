//! Message to Event conversion.
//!
//! Converts incoming `irc_proto::Message` to `Event` variants,
//! parsing IRCv3 tags for metadata.

use irc_proto::{Command, Message, Prefix};

use crate::event::{Event, MessageMeta};
use crate::state::{MemberInfo, SessionState, TopicInfo};

/// Convert an IRC message to an Event.
///
/// Also updates session state as needed.
pub fn message_to_event(msg: &Message, state: &mut SessionState) -> Option<Event> {
    let meta = MessageMeta::from_tags(msg.tags.as_ref());

    match &msg.command {
        // === Messages ===
        Command::Privmsg { target, message } => {
            let source = format_source(&msg.prefix);

            // Check for CTCP ACTION
            if message.starts_with("\x01ACTION ") && message.ends_with('\x01') {
                let action = &message[8..message.len() - 1];
                return Some(Event::Action {
                    source,
                    target: target.clone(),
                    action: action.to_string(),
                    meta,
                });
            }

            Some(Event::Privmsg {
                source,
                target: target.clone(),
                message: message.clone(),
                meta,
            })
        }

        Command::Notice { target, message } => Some(Event::Notice {
            source: msg.prefix.as_ref().map(|p| format_prefix(p)),
            target: target.clone(),
            message: message.clone(),
            meta,
        }),

        // === Channel Events ===
        Command::Join { channels } => {
            let nick = msg.source_nick().unwrap_or("").to_string();
            let userhost = msg.prefix.as_ref().and_then(|p| {
                match p {
                    Prefix::User { user: Some(user), host: Some(host), .. } => {
                        Some(format!("{}@{}", user, host))
                    }
                    _ => None,
                }
            });

            // Extended-join: account and realname in channel params
            // Format: JOIN #channel account :realname
            let (account, realname) = if channels.len() >= 2 {
                // This is a bit of a hack - extended-join puts account/realname
                // as additional params, but our Command::Join doesn't parse them
                (None, None)
            } else {
                (None, None)
            };

            // Check if this is us joining
            if nick.eq_ignore_ascii_case(state.nick()) {
                for (channel, _) in channels {
                    state.add_channel(channel);
                }
            } else {
                // Add the user to channel members
                for (channel, _) in channels {
                    state.add_member(channel, &nick, MemberInfo {
                        prefixes: String::new(),
                        userhost: userhost.clone(),
                        account: account.clone(),
                        away: None,
                    });
                }
            }

            // Only emit event for first channel (usually just one)
            channels.first().map(|(channel, _)| Event::Join {
                nick,
                userhost,
                channel: channel.clone(),
                account,
                realname,
            })
        }

        Command::Part { channels, message } => {
            let nick = msg.source_nick().unwrap_or("").to_string();

            // Check if this is us leaving
            if nick.eq_ignore_ascii_case(state.nick()) {
                for channel in channels {
                    state.remove_channel(channel);
                }
            } else {
                // Remove user from channels
                for channel in channels {
                    state.remove_member(channel, &nick);
                }
            }

            channels.first().map(|channel| Event::Part {
                nick,
                channel: channel.clone(),
                message: message.clone(),
            })
        }

        Command::Kick { channel, users, comment } => {
            let kicker = msg.source_nick().unwrap_or("").to_string();

            for user in users {
                // Check if this is us being kicked
                if user.eq_ignore_ascii_case(state.nick()) {
                    state.remove_channel(channel);
                } else {
                    state.remove_member(channel, user);
                }
            }

            users.first().map(|nick| Event::Kick {
                nick: nick.clone(),
                channel: channel.clone(),
                kicker,
                reason: comment.clone(),
            })
        }

        Command::Quit { message } => {
            let nick = msg.source_nick().unwrap_or("").to_string();
            state.remove_user_from_all_channels(&nick);

            Some(Event::Quit {
                nick,
                message: message.clone(),
            })
        }

        Command::Topic { channel, topic } => {
            let setter = msg.source_nick().map(String::from);

            if let Some(topic_text) = topic {
                state.set_topic(channel, Some(TopicInfo {
                    text: topic_text.clone(),
                    setter: setter.clone(),
                    set_at: None,
                }));
            } else {
                state.set_topic(channel, None);
            }

            Some(Event::Topic {
                channel: channel.clone(),
                topic: topic.clone(),
                setter,
            })
        }

        Command::Nick { nickname } => {
            let old_nick = msg.source_nick().unwrap_or("").to_string();
            let new_nick = nickname.clone();

            // Check if this is our nick change
            if old_nick.eq_ignore_ascii_case(state.nick()) {
                state.set_nick(&new_nick);
                return Some(Event::NickChange {
                    old_nick,
                    new_nick,
                });
            }

            // Rename user in all channels
            state.rename_user(&old_nick, &new_nick);

            Some(Event::Nick {
                old_nick,
                new_nick,
            })
        }

        Command::Mode { target, modes, params } => {
            if target.starts_with('#') || target.starts_with('&') {
                // Channel mode
                let setter = msg.source_nick().unwrap_or("server").to_string();
                let mode_str = format_mode_string(modes.as_deref(), params);

                // Update channel member modes
                update_channel_modes(state, target, modes.as_deref(), params);

                Some(Event::ChannelMode {
                    channel: target.clone(),
                    setter,
                    modes: mode_str,
                })
            } else if target.eq_ignore_ascii_case(state.nick()) {
                // Our user mode
                if let Some(mode_str) = modes {
                    update_user_modes(state, mode_str);
                    Some(Event::UserMode {
                        modes: mode_str.clone(),
                    })
                } else {
                    None
                }
            } else {
                None
            }
        }

        Command::Invite { nickname: _, channel } => {
            let inviter = msg.source_nick().unwrap_or("").to_string();
            Some(Event::Invite {
                inviter,
                channel: channel.clone(),
            })
        }

        // === IRCv3 Events ===
        Command::Account { account } => {
            let nick = msg.source_nick().unwrap_or("").to_string();
            let account = if account == "*" { None } else { Some(account.clone()) };

            // Update user info
            state.update_user(&nick, |u| u.account = account.clone());

            Some(Event::AccountChange { nick, account })
        }

        Command::Away { message } => {
            let nick = msg.source_nick().unwrap_or("").to_string();

            // Update user info
            state.update_user(&nick, |u| u.away = message.clone());

            Some(Event::AwayChange {
                nick,
                away: message.clone(),
            })
        }

        Command::Chghost { user, host } => {
            let nick = msg.source_nick().unwrap_or("").to_string();
            let new_userhost = format!("{}@{}", user, host);

            // Update user info
            state.update_user(&nick, |u| u.userhost = Some(new_userhost.clone()));

            Some(Event::HostChange {
                nick,
                user: user.clone(),
                host: host.clone(),
            })
        }

        Command::Setname { realname } => {
            let nick = msg.source_nick().unwrap_or("").to_string();

            state.update_user(&nick, |u| u.realname = Some(realname.clone()));

            Some(Event::RealnameChange {
                nick,
                realname: realname.clone(),
            })
        }

        // === Server Messages ===
        Command::Ping { server1, .. } => Some(Event::Ping {
            token: server1.clone(),
        }),

        Command::Numeric { code, params, .. } => {
            handle_numeric(*code, params, state)
        }

        _ => {
            // Return raw event for unhandled messages
            Some(Event::Raw {
                line: msg.to_string(),
            })
        }
    }
}

/// Handle numeric replies.
fn handle_numeric(code: u16, params: &[String], state: &mut SessionState) -> Option<Event> {
    match code {
        // === NAMES reply ===
        353 => {
            // RPL_NAMREPLY: <nick> <symbol> <channel> :<names>
            // params: ["=", "#channel", "nick1 @nick2 +nick3"]
            if params.len() >= 2 {
                let channel = &params[params.len() - 2];
                let names_str = params.last()?;

                let names: Vec<(String, String)> = names_str
                    .split_whitespace()
                    .map(|entry| {
                        let prefixes: String = entry
                            .chars()
                            .take_while(|c| !c.is_alphanumeric() && *c != '[' && *c != '{' && *c != '\\')
                            .collect();
                        let nick = entry[prefixes.len()..].to_string();
                        (prefixes, nick)
                    })
                    .collect();

                // Update channel state
                for (prefix, nick) in &names {
                    state.add_member(channel, nick, MemberInfo {
                        prefixes: prefix.clone(),
                        userhost: None,
                        account: None,
                        away: None,
                    });
                }

                return Some(Event::Names {
                    channel: channel.clone(),
                    names,
                });
            }
            None
        }

        // === Topic replies ===
        332 => {
            // RPL_TOPIC: <channel> :<topic>
            if params.len() >= 2 {
                let channel = &params[0];
                let topic = params.get(1).cloned();

                if let Some(text) = topic.clone() {
                    state.set_topic(channel, Some(TopicInfo {
                        text,
                        setter: None,
                        set_at: None,
                    }));
                }

                return Some(Event::Topic {
                    channel: channel.clone(),
                    topic,
                    setter: None,
                });
            }
            None
        }

        333 => {
            // RPL_TOPICWHOTIME: <channel> <setter> <timestamp>
            if params.len() >= 3 {
                let channel = &params[0];
                let setter = &params[1];
                let timestamp: Option<i64> = params.get(2).and_then(|s| s.parse().ok());

                if let Some(chan) = state.channel_mut(channel) {
                    if let Some(ref mut topic) = chan.topic {
                        topic.setter = Some(setter.clone());
                        topic.set_at = timestamp;
                    }
                }
            }
            None
        }

        // === MOTD ===
        372 | 375 | 376 => {
            // RPL_MOTD, RPL_MOTDSTART, RPL_ENDOFMOTD
            let line = params.last().cloned().unwrap_or_default();
            Some(Event::Motd { line })
        }

        // === ISUPPORT ===
        5 => {
            // RPL_ISUPPORT
            for param in params.iter().take(params.len().saturating_sub(1)) {
                if let Some((key, value)) = param.split_once('=') {
                    state.set_isupport(key, Some(value.to_string()));
                } else if !param.starts_with(':') {
                    state.set_isupport(param, None);
                }
            }
            None
        }

        // === Error numerics ===
        400..=599 => {
            let message = params.last().cloned().unwrap_or_default();
            Some(Event::ServerError { message })
        }

        // Default: pass through as numeric
        _ => Some(Event::Numeric {
            code,
            params: params.to_vec(),
        }),
    }
}

/// Format message source for display.
fn format_source(prefix: &Option<Prefix>) -> String {
    match prefix {
        Some(p) => format_prefix(p),
        None => "server".to_string(),
    }
}

/// Format prefix for display.
fn format_prefix(prefix: &Prefix) -> String {
    prefix.to_string()
}

/// Format mode string for display.
fn format_mode_string(modes: Option<&str>, params: &[String]) -> String {
    match modes {
        Some(m) if !params.is_empty() => format!("{} {}", m, params.join(" ")),
        Some(m) => m.to_string(),
        None => String::new(),
    }
}

/// Update user modes from a mode string.
fn update_user_modes(state: &mut SessionState, modes: &str) {
    let mut adding = true;
    for c in modes.chars() {
        match c {
            '+' => adding = true,
            '-' => adding = false,
            _ => {
                if adding {
                    state.add_user_mode(c);
                } else {
                    state.remove_user_mode(c);
                }
            }
        }
    }
}

/// Update channel member modes (op, voice, etc).
fn update_channel_modes(
    state: &mut SessionState,
    channel: &str,
    modes: Option<&str>,
    params: &[String],
) {
    let modes = match modes {
        Some(m) => m,
        None => return,
    };

    let mut adding = true;
    let mut param_idx = 0;

    for c in modes.chars() {
        match c {
            '+' => adding = true,
            '-' => adding = false,
            // Channel member modes
            'o' | 'v' | 'h' | 'a' | 'q' => {
                if let Some(nick) = params.get(param_idx) {
                    if let Some(member) = state.channel_mut(channel).and_then(|ch| ch.member_mut(nick)) {
                        let prefix_char = match c {
                            'o' => '@',
                            'v' => '+',
                            'h' => '%',
                            'a' => '&',
                            'q' => '~',
                            _ => continue,
                        };

                        if adding {
                            if !member.prefixes.contains(prefix_char) {
                                member.prefixes.push(prefix_char);
                            }
                        } else {
                            member.prefixes = member.prefixes.replace(prefix_char, "");
                        }
                    }
                    param_idx += 1;
                }
            }
            // Modes with parameters
            'k' | 'l' | 'b' | 'e' | 'I' => {
                param_idx += 1;
            }
            // Simple channel modes (no param)
            _ => {}
        }
    }
}
