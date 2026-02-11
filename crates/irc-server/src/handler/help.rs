//! HELP command handler.

use irc_proto::replies::*;

use super::HandlerContext;
use crate::error::Result;

/// Help topics and their content.
const HELP_TOPICS: &[(&str, &[&str])] = &[
    ("", &[
        "Available topics: COMMANDS, USERMODES, CHANMODES, REGISTER, NICKSERV, CHANSERV",
        "Type /HELP <topic> for more information.",
    ]),
    ("COMMANDS", &[
        "User Commands:",
        "  JOIN PART PRIVMSG NOTICE NICK QUIT AWAY WHOIS WHO",
        "Channel Commands:",
        "  TOPIC MODE KICK INVITE NAMES LIST",
        "Server Commands:",
        "  MOTD VERSION TIME ADMIN INFO LUSERS STATS",
        "IRCv3:",
        "  CAP AUTHENTICATE MONITOR CHATHISTORY",
        "Operator:",
        "  OPER KILL WALLOPS KLINE ZLINE REHASH DIE",
    ]),
    ("USERMODES", &[
        "User modes:",
        "  +i - Invisible (hidden from WHO unless in shared channel)",
        "  +w - Receive WALLOPS messages",
        "  +o - IRC Operator (set via OPER command)",
        "  +r - Registered with services",
    ]),
    ("CHANMODES", &[
        "Channel modes:",
        "  +b <mask> - Ban mask",
        "  +e <mask> - Ban exception",
        "  +I <mask> - Invite exception",
        "  +i - Invite only",
        "  +k <key> - Channel key (password)",
        "  +l <limit> - User limit",
        "  +m - Moderated (only +v/+o can speak)",
        "  +n - No external messages",
        "  +o <nick> - Channel operator",
        "  +s - Secret (hidden from LIST)",
        "  +t - Topic lock (only ops can change)",
        "  +v <nick> - Voice",
    ]),
    ("REGISTER", &[
        "To register your nickname:",
        "  /msg NickServ REGISTER <password> <email>",
        "To identify:",
        "  /msg NickServ IDENTIFY <password>",
        "Or use SASL during connection.",
    ]),
    ("NICKSERV", &[
        "NickServ commands:",
        "  REGISTER <password> <email> - Register your nickname",
        "  IDENTIFY <password> - Identify to your account",
        "  GHOST <nick> - Disconnect someone using your nick",
        "  INFO <nick> - View registration info",
        "  SET <option> <value> - Change account settings",
    ]),
    ("CHANSERV", &[
        "ChanServ commands:",
        "  REGISTER <#channel> - Register a channel (you must be op)",
        "  INFO <#channel> - View channel registration info",
        "  OP <#channel> [nick] - Grant operator status",
        "  DEOP <#channel> <nick> - Remove operator status",
        "  ACCESS <#channel> LIST - List channel access",
        "  ACCESS <#channel> ADD <account> <flags> - Add access",
        "  ACCESS <#channel> DEL <account> - Remove access",
    ]),
    ("OPER", &[
        "Operator commands:",
        "  OPER <name> <password> - Gain operator privileges",
        "  KILL <nick> <reason> - Disconnect a user",
        "  WALLOPS <message> - Send message to operators",
        "  KLINE [duration] <mask> [reason] - Ban by user@host",
        "  UNKLINE <mask> - Remove K-line",
        "  ZLINE [duration] <ip> [reason] - Ban by IP",
        "  UNZLINE <ip> - Remove Z-line",
        "  REHASH - Reload server configuration",
        "  RESTART - Restart the server",
        "  DIE - Shut down the server",
    ]),
    ("MONITOR", &[
        "MONITOR command (track user presence):",
        "  MONITOR + nick1,nick2 - Add nicknames to watch",
        "  MONITOR - nick1,nick2 - Remove from watch list",
        "  MONITOR C - Clear watch list",
        "  MONITOR L - List watched nicknames",
        "  MONITOR S - Get status of watched nicknames",
    ]),
    ("CHATHISTORY", &[
        "CHATHISTORY command (message replay):",
        "  Requires draft/chathistory capability",
        "  CHATHISTORY LATEST <target> * <limit>",
        "  CHATHISTORY BEFORE <target> <msgid> <limit>",
        "  CHATHISTORY AFTER <target> <msgid> <limit>",
        "  CHATHISTORY BETWEEN <target> <start> <end> <limit>",
    ]),
];

/// Handle HELP command.
pub fn handle_help(ctx: &HandlerContext, topic: Option<&str>) -> Result<()> {
    let topic_upper = topic.map(|s| s.to_uppercase()).unwrap_or_default();
    let topic_display = if topic_upper.is_empty() { "HELP" } else { &topic_upper };

    let help_lines = HELP_TOPICS
        .iter()
        .find(|(t, _)| t.eq_ignore_ascii_case(&topic_upper))
        .map(|(_, lines)| *lines);

    match help_lines {
        Some(lines) => {
            for line in lines {
                ctx.reply(RPL_HELPTXT, vec![topic_display.to_string(), (*line).to_string()])?;
            }
        }
        None => {
            ctx.reply(
                RPL_HELPTXT,
                vec![
                    topic_display.to_string(),
                    "No help available for that topic.".into(),
                ],
            )?;
            ctx.reply(
                RPL_HELPTXT,
                vec![
                    topic_display.to_string(),
                    "Type /HELP for a list of topics.".into(),
                ],
            )?;
        }
    }

    ctx.reply(RPL_ENDOFHELP, vec![topic_display.to_string(), "End of HELP".into()])?;

    Ok(())
}
