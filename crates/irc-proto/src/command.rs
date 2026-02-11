//! IRC commands.
//!
//! This module defines all IRC commands as a strongly-typed enum.

use std::fmt;

/// IRC command.
///
/// This enum represents all standard IRC commands as well as
/// numeric replies from servers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Command {
    // === Connection Registration ===
    /// PASS - Set connection password
    Pass {
        password: String,
    },

    /// NICK - Set or change nickname
    Nick {
        nickname: String,
    },

    /// USER - Specify username and realname
    User {
        username: String,
        mode: u8,
        realname: String,
    },

    /// OPER - Obtain operator privileges
    Oper {
        name: String,
        password: String,
    },

    /// QUIT - Disconnect from server
    Quit {
        message: Option<String>,
    },

    // === Channel Operations ===
    /// JOIN - Join channel(s)
    Join {
        /// List of (channel, optional key) pairs
        channels: Vec<(String, Option<String>)>,
    },

    /// PART - Leave channel(s)
    Part {
        channels: Vec<String>,
        message: Option<String>,
    },

    /// MODE - Set channel or user modes
    Mode {
        target: String,
        modes: Option<String>,
        params: Vec<String>,
    },

    /// TOPIC - Get or set channel topic
    Topic {
        channel: String,
        topic: Option<String>,
    },

    /// NAMES - List users in channel
    Names {
        channels: Option<Vec<String>>,
    },

    /// LIST - List channels
    List {
        channels: Option<Vec<String>>,
    },

    /// INVITE - Invite user to channel
    Invite {
        nickname: String,
        channel: String,
    },

    /// KICK - Remove user from channel
    Kick {
        channel: String,
        users: Vec<String>,
        comment: Option<String>,
    },

    // === Messaging ===
    /// PRIVMSG - Send message to user or channel
    Privmsg {
        target: String,
        message: String,
    },

    /// NOTICE - Send notice (no auto-reply expected)
    Notice {
        target: String,
        message: String,
    },

    /// TAGMSG - Send message tags only (no text content)
    Tagmsg {
        target: String,
    },

    // === Server Queries ===
    /// MOTD - Request message of the day
    Motd {
        server: Option<String>,
    },

    /// LUSERS - Request user statistics
    Lusers {
        mask: Option<String>,
        server: Option<String>,
    },

    /// VERSION - Request server version
    Version {
        server: Option<String>,
    },

    /// STATS - Request server statistics
    Stats {
        query: Option<char>,
        server: Option<String>,
    },

    /// TIME - Request server time
    Time {
        server: Option<String>,
    },

    /// ADMIN - Request admin info
    Admin {
        server: Option<String>,
    },

    /// INFO - Request server info
    Info {
        server: Option<String>,
    },

    // === User Queries ===
    /// WHO - List users matching mask
    Who {
        mask: String,
        operators_only: bool,
    },

    /// WHOIS - Query user information
    Whois {
        server: Option<String>,
        nicknames: Vec<String>,
    },

    /// WHOWAS - Query disconnected user info
    Whowas {
        nickname: String,
        count: Option<u32>,
        server: Option<String>,
    },

    // === Miscellaneous ===
    /// PING - Test connection
    Ping {
        server1: String,
        server2: Option<String>,
    },

    /// PONG - Reply to ping
    Pong {
        server1: String,
        server2: Option<String>,
    },

    /// AWAY - Set or unset away message
    Away {
        message: Option<String>,
    },

    /// USERHOST - Get user host info
    Userhost {
        nicknames: Vec<String>,
    },

    /// ISON - Check if users are online
    Ison {
        nicknames: Vec<String>,
    },

    // === Operator Commands ===
    /// KILL - Disconnect a user
    Kill {
        nickname: String,
        comment: String,
    },

    /// WALLOPS - Send message to operators
    Wallops {
        message: String,
    },

    /// REHASH - Reload server configuration
    Rehash,

    /// RESTART - Restart the server
    Restart,

    /// DIE - Shut down the server
    Die,

    /// KLINE - Ban by user@host mask
    Kline {
        duration: Option<String>,
        mask: String,
        reason: Option<String>,
    },

    /// UNKLINE - Remove K-line
    Unkline {
        mask: String,
    },

    /// GLINE - Global ban (alias for KLINE on single-server)
    Gline {
        duration: Option<String>,
        mask: String,
        reason: Option<String>,
    },

    /// UNGLINE - Remove G-line
    Ungline {
        mask: String,
    },

    /// ZLINE - Ban by IP/CIDR
    Zline {
        duration: Option<String>,
        mask: String,
        reason: Option<String>,
    },

    /// UNZLINE - Remove Z-line
    Unzline {
        mask: String,
    },

    /// HELP - Request help information
    Help {
        topic: Option<String>,
    },

    // === IRCv3 ===
    /// CAP - Capability negotiation
    Cap {
        subcommand: String,
        params: Vec<String>,
    },

    /// AUTHENTICATE - SASL authentication
    Authenticate {
        data: String,
    },

    /// BATCH - Start or end a message batch
    Batch {
        reference: String,
        batch_type: Option<String>,
        params: Vec<String>,
    },

    /// ACCOUNT - Account notification (server to client)
    Account {
        account: String,
    },

    /// CHGHOST - Host change notification
    Chghost {
        user: String,
        host: String,
    },

    /// SETNAME - Change realname
    Setname {
        realname: String,
    },

    /// CHATHISTORY - Request message history
    Chathistory {
        subcommand: String,
        target: String,
        params: Vec<String>,
    },

    /// MONITOR - Watch for user online/offline
    Monitor {
        subcommand: char,
        targets: Option<String>,
    },

    /// MARKREAD - Set read marker
    Markread {
        target: String,
        timestamp: Option<String>,
    },

    // === Numeric Replies ===
    /// Numeric reply (001-999)
    Numeric {
        code: u16,
        target: String,
        params: Vec<String>,
    },

    /// Unknown command (passthrough)
    Unknown {
        command: String,
        params: Vec<String>,
    },
}

impl Command {
    /// Get the command name as a string.
    pub fn name(&self) -> &str {
        match self {
            Command::Pass { .. } => "PASS",
            Command::Nick { .. } => "NICK",
            Command::User { .. } => "USER",
            Command::Oper { .. } => "OPER",
            Command::Quit { .. } => "QUIT",
            Command::Join { .. } => "JOIN",
            Command::Part { .. } => "PART",
            Command::Mode { .. } => "MODE",
            Command::Topic { .. } => "TOPIC",
            Command::Names { .. } => "NAMES",
            Command::List { .. } => "LIST",
            Command::Invite { .. } => "INVITE",
            Command::Kick { .. } => "KICK",
            Command::Privmsg { .. } => "PRIVMSG",
            Command::Notice { .. } => "NOTICE",
            Command::Tagmsg { .. } => "TAGMSG",
            Command::Motd { .. } => "MOTD",
            Command::Lusers { .. } => "LUSERS",
            Command::Version { .. } => "VERSION",
            Command::Stats { .. } => "STATS",
            Command::Time { .. } => "TIME",
            Command::Admin { .. } => "ADMIN",
            Command::Info { .. } => "INFO",
            Command::Who { .. } => "WHO",
            Command::Whois { .. } => "WHOIS",
            Command::Whowas { .. } => "WHOWAS",
            Command::Ping { .. } => "PING",
            Command::Pong { .. } => "PONG",
            Command::Away { .. } => "AWAY",
            Command::Userhost { .. } => "USERHOST",
            Command::Ison { .. } => "ISON",
            Command::Kill { .. } => "KILL",
            Command::Wallops { .. } => "WALLOPS",
            Command::Rehash => "REHASH",
            Command::Restart => "RESTART",
            Command::Die => "DIE",
            Command::Kline { .. } => "KLINE",
            Command::Unkline { .. } => "UNKLINE",
            Command::Gline { .. } => "GLINE",
            Command::Ungline { .. } => "UNGLINE",
            Command::Zline { .. } => "ZLINE",
            Command::Unzline { .. } => "UNZLINE",
            Command::Help { .. } => "HELP",
            Command::Cap { .. } => "CAP",
            Command::Authenticate { .. } => "AUTHENTICATE",
            Command::Batch { .. } => "BATCH",
            Command::Account { .. } => "ACCOUNT",
            Command::Chghost { .. } => "CHGHOST",
            Command::Setname { .. } => "SETNAME",
            Command::Chathistory { .. } => "CHATHISTORY",
            Command::Monitor { .. } => "MONITOR",
            Command::Markread { .. } => "MARKREAD",
            Command::Numeric { .. } => "NUMERIC",
            Command::Unknown { command, .. } => command,
        }
    }

    /// Check if this is a numeric reply.
    pub fn is_numeric(&self) -> bool {
        matches!(self, Command::Numeric { .. })
    }

    /// Get the numeric code if this is a numeric reply.
    pub fn numeric_code(&self) -> Option<u16> {
        match self {
            Command::Numeric { code, .. } => Some(*code),
            _ => None,
        }
    }
}

impl fmt::Display for Command {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Command::Pass { password } => write!(f, "PASS {}", password),
            Command::Nick { nickname } => write!(f, "NICK {}", nickname),
            Command::User {
                username,
                mode,
                realname,
            } => write!(f, "USER {} {} * :{}", username, mode, realname),
            Command::Oper { name, password } => write!(f, "OPER {} {}", name, password),
            Command::Quit { message } => match message {
                Some(msg) => write!(f, "QUIT :{}", msg),
                None => write!(f, "QUIT"),
            },
            Command::Join { channels } => {
                let chans: Vec<_> = channels.iter().map(|(c, _)| c.as_str()).collect();
                let keys: Vec<_> = channels
                    .iter()
                    .filter_map(|(_, k)| k.as_deref())
                    .collect();

                if keys.is_empty() {
                    write!(f, "JOIN {}", chans.join(","))
                } else {
                    write!(f, "JOIN {} {}", chans.join(","), keys.join(","))
                }
            }
            Command::Part { channels, message } => {
                let chans = channels.join(",");
                match message {
                    Some(msg) => write!(f, "PART {} :{}", chans, msg),
                    None => write!(f, "PART {}", chans),
                }
            }
            Command::Mode {
                target,
                modes,
                params,
            } => {
                write!(f, "MODE {}", target)?;
                if let Some(m) = modes {
                    write!(f, " {}", m)?;
                    for p in params {
                        write!(f, " {}", p)?;
                    }
                }
                Ok(())
            }
            Command::Topic { channel, topic } => match topic {
                Some(t) => write!(f, "TOPIC {} :{}", channel, t),
                None => write!(f, "TOPIC {}", channel),
            },
            Command::Names { channels } => match channels {
                Some(chans) => write!(f, "NAMES {}", chans.join(",")),
                None => write!(f, "NAMES"),
            },
            Command::List { channels } => match channels {
                Some(chans) => write!(f, "LIST {}", chans.join(",")),
                None => write!(f, "LIST"),
            },
            Command::Invite { nickname, channel } => {
                write!(f, "INVITE {} {}", nickname, channel)
            }
            Command::Kick {
                channel,
                users,
                comment,
            } => {
                let users_str = users.join(",");
                match comment {
                    Some(c) => write!(f, "KICK {} {} :{}", channel, users_str, c),
                    None => write!(f, "KICK {} {}", channel, users_str),
                }
            }
            Command::Privmsg { target, message } => {
                write!(f, "PRIVMSG {} :{}", target, message)
            }
            Command::Notice { target, message } => {
                write!(f, "NOTICE {} :{}", target, message)
            }
            Command::Tagmsg { target } => write!(f, "TAGMSG {}", target),
            Command::Motd { server } => match server {
                Some(s) => write!(f, "MOTD {}", s),
                None => write!(f, "MOTD"),
            },
            Command::Lusers { mask, server } => {
                write!(f, "LUSERS")?;
                if let Some(m) = mask {
                    write!(f, " {}", m)?;
                    if let Some(s) = server {
                        write!(f, " {}", s)?;
                    }
                }
                Ok(())
            }
            Command::Version { server } => match server {
                Some(s) => write!(f, "VERSION {}", s),
                None => write!(f, "VERSION"),
            },
            Command::Stats { query, server } => {
                write!(f, "STATS")?;
                if let Some(q) = query {
                    write!(f, " {}", q)?;
                    if let Some(s) = server {
                        write!(f, " {}", s)?;
                    }
                }
                Ok(())
            }
            Command::Time { server } => match server {
                Some(s) => write!(f, "TIME {}", s),
                None => write!(f, "TIME"),
            },
            Command::Admin { server } => match server {
                Some(s) => write!(f, "ADMIN {}", s),
                None => write!(f, "ADMIN"),
            },
            Command::Info { server } => match server {
                Some(s) => write!(f, "INFO {}", s),
                None => write!(f, "INFO"),
            },
            Command::Who {
                mask,
                operators_only,
            } => {
                if *operators_only {
                    write!(f, "WHO {} o", mask)
                } else {
                    write!(f, "WHO {}", mask)
                }
            }
            Command::Whois { server, nicknames } => {
                let nicks = nicknames.join(",");
                match server {
                    Some(s) => write!(f, "WHOIS {} {}", s, nicks),
                    None => write!(f, "WHOIS {}", nicks),
                }
            }
            Command::Whowas {
                nickname,
                count,
                server,
            } => {
                write!(f, "WHOWAS {}", nickname)?;
                if let Some(c) = count {
                    write!(f, " {}", c)?;
                    if let Some(s) = server {
                        write!(f, " {}", s)?;
                    }
                }
                Ok(())
            }
            Command::Ping { server1, server2 } => match server2 {
                Some(s2) => write!(f, "PING {} {}", server1, s2),
                None => write!(f, "PING {}", server1),
            },
            Command::Pong { server1, server2 } => match server2 {
                Some(s2) => write!(f, "PONG {} {}", server1, s2),
                None => write!(f, "PONG {}", server1),
            },
            Command::Away { message } => match message {
                Some(msg) => write!(f, "AWAY :{}", msg),
                None => write!(f, "AWAY"),
            },
            Command::Userhost { nicknames } => {
                write!(f, "USERHOST {}", nicknames.join(" "))
            }
            Command::Ison { nicknames } => {
                write!(f, "ISON {}", nicknames.join(" "))
            }
            Command::Kill { nickname, comment } => {
                write!(f, "KILL {} :{}", nickname, comment)
            }
            Command::Wallops { message } => write!(f, "WALLOPS :{}", message),
            Command::Rehash => write!(f, "REHASH"),
            Command::Restart => write!(f, "RESTART"),
            Command::Die => write!(f, "DIE"),
            Command::Kline { duration, mask, reason } => {
                write!(f, "KLINE")?;
                if let Some(d) = duration {
                    write!(f, " {}", d)?;
                }
                write!(f, " {}", mask)?;
                if let Some(r) = reason {
                    write!(f, " :{}", r)?;
                }
                Ok(())
            }
            Command::Unkline { mask } => write!(f, "UNKLINE {}", mask),
            Command::Gline { duration, mask, reason } => {
                write!(f, "GLINE")?;
                if let Some(d) = duration {
                    write!(f, " {}", d)?;
                }
                write!(f, " {}", mask)?;
                if let Some(r) = reason {
                    write!(f, " :{}", r)?;
                }
                Ok(())
            }
            Command::Ungline { mask } => write!(f, "UNGLINE {}", mask),
            Command::Zline { duration, mask, reason } => {
                write!(f, "ZLINE")?;
                if let Some(d) = duration {
                    write!(f, " {}", d)?;
                }
                write!(f, " {}", mask)?;
                if let Some(r) = reason {
                    write!(f, " :{}", r)?;
                }
                Ok(())
            }
            Command::Unzline { mask } => write!(f, "UNZLINE {}", mask),
            Command::Help { topic } => match topic {
                Some(t) => write!(f, "HELP {}", t),
                None => write!(f, "HELP"),
            },
            Command::Cap { subcommand, params } => {
                write!(f, "CAP {}", subcommand)?;
                for p in params {
                    if p.contains(' ') {
                        write!(f, " :{}", p)?;
                    } else {
                        write!(f, " {}", p)?;
                    }
                }
                Ok(())
            }
            Command::Authenticate { data } => write!(f, "AUTHENTICATE {}", data),
            Command::Batch {
                reference,
                batch_type,
                params,
            } => {
                write!(f, "BATCH {}", reference)?;
                if let Some(bt) = batch_type {
                    write!(f, " {}", bt)?;
                }
                for p in params {
                    write!(f, " {}", p)?;
                }
                Ok(())
            }
            Command::Account { account } => write!(f, "ACCOUNT {}", account),
            Command::Chghost { user, host } => write!(f, "CHGHOST {} {}", user, host),
            Command::Setname { realname } => write!(f, "SETNAME :{}", realname),
            Command::Chathistory {
                subcommand,
                target,
                params,
            } => {
                write!(f, "CHATHISTORY {} {}", subcommand, target)?;
                for p in params {
                    write!(f, " {}", p)?;
                }
                Ok(())
            }
            Command::Monitor { subcommand, targets } => {
                write!(f, "MONITOR {}", subcommand)?;
                if let Some(t) = targets {
                    write!(f, " {}", t)?;
                }
                Ok(())
            }
            Command::Markread { target, timestamp } => {
                write!(f, "MARKREAD {}", target)?;
                if let Some(ts) = timestamp {
                    write!(f, " {}", ts)?;
                }
                Ok(())
            }
            Command::Numeric {
                code,
                target,
                params,
            } => {
                write!(f, "{:03} {}", code, target)?;
                for (i, p) in params.iter().enumerate() {
                    if i == params.len() - 1 && p.contains(' ') {
                        write!(f, " :{}", p)?;
                    } else {
                        write!(f, " {}", p)?;
                    }
                }
                Ok(())
            }
            Command::Unknown { command, params } => {
                write!(f, "{}", command)?;
                for (i, p) in params.iter().enumerate() {
                    if i == params.len() - 1 && p.contains(' ') {
                        write!(f, " :{}", p)?;
                    } else {
                        write!(f, " {}", p)?;
                    }
                }
                Ok(())
            }
        }
    }
}
