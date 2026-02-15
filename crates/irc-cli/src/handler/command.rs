//! Slash command parsing.

/// Parsed user command.
#[derive(Debug, Clone)]
pub enum Command {
    /// /join #channel [key]
    Join {
        channel: String,
        key: Option<String>,
    },

    /// /part [message]
    Part { message: Option<String> },

    /// /msg nick text
    Msg { target: String, text: String },

    /// /me action
    Me { text: String },

    /// /nick newnick
    Nick { nick: String },

    /// /quit [message]
    Quit { message: Option<String> },

    /// /topic [text]
    Topic { text: Option<String> },

    /// /kick nick [reason]
    Kick {
        nick: String,
        reason: Option<String>,
    },

    /// /invite nick
    Invite { nick: String },

    /// /away [message]
    Away { message: Option<String> },

    /// /clear - Clear current buffer
    Clear,

    /// /close - Close current buffer
    Close,

    /// /query nick - Open query with nick
    Query { nick: String },

    /// /history [count] - Request chat history
    History { count: Option<usize> },

    /// /raw command - Send raw IRC command
    Raw { text: String },

    /// /help [topic]
    Help { topic: Option<String> },

    /// /reconnect - Manual reconnect
    Reconnect,

    /// /disconnect - Disconnect without quitting
    Disconnect,

    /// /list [filter] - List channels (with optional filter)
    List { filter: Option<String> },

    /// /joinpart - Toggle join/part message visibility
    JoinPart,

    /// Regular message (not a command)
    Message { text: String },
}

/// Parse user input into a command.
pub fn parse_command(input: &str) -> Command {
    let input = input.trim();

    if !input.starts_with('/') {
        return Command::Message {
            text: input.to_string(),
        };
    }

    // Skip the slash
    let input = &input[1..];

    // Split into command and args
    let (cmd, args) = match input.find(' ') {
        Some(i) => (&input[..i], input[i + 1..].trim()),
        None => (input, ""),
    };

    match cmd.to_lowercase().as_str() {
        "join" | "j" => {
            let parts: Vec<&str> = args.splitn(2, ' ').collect();
            let channel = parts.first().map(|s| s.to_string()).unwrap_or_default();
            let key = parts.get(1).map(|s| s.to_string());

            if channel.is_empty() {
                Command::Help {
                    topic: Some("join".into()),
                }
            } else {
                Command::Join { channel, key }
            }
        }

        "part" | "leave" => Command::Part {
            message: if args.is_empty() {
                None
            } else {
                Some(args.to_string())
            },
        },

        "msg" | "privmsg" | "query" => {
            let parts: Vec<&str> = args.splitn(2, ' ').collect();
            let target = parts.first().map(|s| s.to_string()).unwrap_or_default();
            let text = parts.get(1).map(|s| s.to_string()).unwrap_or_default();

            if target.is_empty() {
                Command::Help {
                    topic: Some("msg".into()),
                }
            } else if text.is_empty() {
                // /query nick - just open a query
                Command::Query { nick: target }
            } else {
                Command::Msg { target, text }
            }
        }

        "me" | "action" => {
            if args.is_empty() {
                Command::Help {
                    topic: Some("me".into()),
                }
            } else {
                Command::Me {
                    text: args.to_string(),
                }
            }
        }

        "nick" => {
            if args.is_empty() {
                Command::Help {
                    topic: Some("nick".into()),
                }
            } else {
                let nick = args.split_whitespace().next().unwrap_or("").to_string();
                Command::Nick { nick }
            }
        }

        "quit" | "exit" | "q" => Command::Quit {
            message: if args.is_empty() {
                None
            } else {
                Some(args.to_string())
            },
        },

        "topic" | "t" => Command::Topic {
            text: if args.is_empty() {
                None
            } else {
                Some(args.to_string())
            },
        },

        "kick" | "k" => {
            let parts: Vec<&str> = args.splitn(2, ' ').collect();
            let nick = parts.first().map(|s| s.to_string()).unwrap_or_default();
            let reason = parts.get(1).map(|s| s.to_string());

            if nick.is_empty() {
                Command::Help {
                    topic: Some("kick".into()),
                }
            } else {
                Command::Kick { nick, reason }
            }
        }

        "invite" => {
            let nick = args.split_whitespace().next().unwrap_or("").to_string();
            if nick.is_empty() {
                Command::Help {
                    topic: Some("invite".into()),
                }
            } else {
                Command::Invite { nick }
            }
        }

        "away" => Command::Away {
            message: if args.is_empty() {
                None
            } else {
                Some(args.to_string())
            },
        },

        "clear" => Command::Clear,

        "close" | "wc" => Command::Close,

        "history" | "hist" => {
            let count = args.parse().ok();
            Command::History { count }
        }

        "raw" | "quote" => {
            if args.is_empty() {
                Command::Help {
                    topic: Some("raw".into()),
                }
            } else {
                Command::Raw {
                    text: args.to_string(),
                }
            }
        }

        "help" | "?" => Command::Help {
            topic: if args.is_empty() {
                None
            } else {
                Some(args.to_string())
            },
        },

        "reconnect" | "connect" => Command::Reconnect,

        "disconnect" => Command::Disconnect,

        "list" => Command::List {
            filter: if args.is_empty() {
                None
            } else {
                Some(args.to_string())
            },
        },

        "joinpart" | "jp" => Command::JoinPart,

        _ => {
            // Unknown command - try to send as raw
            Command::Raw {
                text: input.to_string(),
            }
        }
    }
}

/// Get help text for commands.
pub fn command_help(topic: Option<&str>) -> &'static str {
    match topic {
        Some("join") => "/join <#channel> [key] - Join a channel",
        Some("part") => "/part [message] - Leave the current channel",
        Some("msg") => "/msg <nick> <text> - Send a private message",
        Some("me") => "/me <action> - Send an action message",
        Some("nick") => "/nick <newnick> - Change your nickname",
        Some("quit") => "/quit [message] - Disconnect from the server",
        Some("topic") => "/topic [text] - View or set the channel topic",
        Some("kick") => "/kick <nick> [reason] - Kick a user from the channel",
        Some("invite") => "/invite <nick> - Invite a user to the channel",
        Some("away") => "/away [message] - Set or clear away status",
        Some("clear") => "/clear - Clear the current buffer",
        Some("close") => "/close - Close the current buffer",
        Some("history") => "/history [count] - Request chat history",
        Some("raw") => "/raw <command> - Send a raw IRC command",
        Some("joinpart") => "/joinpart - Toggle visibility of join/part/quit messages",
        _ => concat!(
            "Available commands:\n",
            "  /join, /part, /msg, /me, /nick, /quit, /topic\n",
            "  /kick, /invite, /away, /clear, /close, /history\n",
            "  /raw, /joinpart, /help\n",
            "\n",
            "Keyboard shortcuts:\n",
            "  Ctrl+N / Alt+Down - Next buffer\n",
            "  Ctrl+P / Alt+Up   - Previous buffer\n",
            "  PgUp / PgDown     - Scroll messages\n",
            "  Ctrl+C            - Quit"
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_join() {
        if let Command::Join { channel, key } = parse_command("/join #test key123") {
            assert_eq!(channel, "#test");
            assert_eq!(key, Some("key123".to_string()));
        } else {
            panic!("Expected Join command");
        }
    }

    #[test]
    fn test_parse_msg() {
        if let Command::Msg { target, text } = parse_command("/msg alice Hello there!") {
            assert_eq!(target, "alice");
            assert_eq!(text, "Hello there!");
        } else {
            panic!("Expected Msg command");
        }
    }

    #[test]
    fn test_parse_regular_message() {
        if let Command::Message { text } = parse_command("Hello everyone!") {
            assert_eq!(text, "Hello everyone!");
        } else {
            panic!("Expected Message");
        }
    }
}
