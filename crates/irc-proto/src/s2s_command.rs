//! TS6 Server-to-Server (S2S) commands.
//!
//! This module defines the commands used for server-to-server communication
//! in the TS6 protocol, used by modern IRC networks (Charybdis, Atheme, etc).
//!
//! # Protocol Overview
//!
//! - Each server has a unique 3-character SID (e.g., "00A", "00B")
//! - Each user has a unique UID = SID + 6 chars (e.g., "00AAAAAA1")
//! - Messages use SID/UID as prefix instead of nick!user@host
//! - Timestamps (TS) used for collision resolution

use std::fmt;

use crate::error::ParseError;

/// TS6 Server-to-Server command.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum S2SCommand {
    // === Authentication & Setup ===
    /// PASS - Link password with TS version and SID
    /// Format: PASS password TS ts_version sid
    Pass {
        password: String,
        ts_version: u8,
        sid: String,
    },

    /// CAPAB - Server capabilities negotiation
    /// Format: CAPAB :cap1 cap2 cap3...
    Capab { capabilities: Vec<String> },

    /// SERVER - Identify this server
    /// Format: SERVER name hopcount :description
    Server {
        name: String,
        hopcount: u32,
        description: String,
    },

    /// SVINFO - Send version/time info
    /// Format: SVINFO ts_version ts_min 0 :current_time
    SvInfo {
        ts_version: u8,
        ts_min: u8,
        current_time: i64,
    },

    // === Burst (State Synchronization) ===
    /// BURST - Start state synchronization
    Burst,

    /// ENDBURST - End state synchronization (ack optional)
    EndBurst,

    /// ENDBURSTACKS - End burst acknowledgement
    EndBurstAck,

    /// SID - Introduce a remote server
    /// Format: :source_sid SID name hopcount sid :description
    Sid {
        name: String,
        hopcount: u32,
        sid: String,
        description: String,
    },

    /// UID - Introduce a user (basic)
    /// Format: :sid UID nick hopcount nick_ts modes user host ip uid :realname
    Uid {
        nick: String,
        hopcount: u32,
        nick_ts: i64,
        modes: String,
        user: String,
        host: String,
        ip: String,
        uid: String,
        realname: String,
    },

    /// EUID - Extended UID (includes account, visible host)
    /// Format: :sid EUID nick hopcount nick_ts modes user visible_host ip uid real_host account :realname
    Euid {
        nick: String,
        hopcount: u32,
        nick_ts: i64,
        modes: String,
        user: String,
        visible_host: String,
        ip: String,
        uid: String,
        real_host: String,
        account: Option<String>,
        realname: String,
    },

    /// SJOIN - Server JOIN (batch channel membership sync)
    /// Format: :sid SJOIN channel_ts #channel +modes [mode_params] :[@+]uid ...
    Sjoin {
        channel_ts: i64,
        channel: String,
        modes: String,
        mode_params: Vec<String>,
        members: Vec<SjoinMember>,
    },

    /// TB - Topic burst
    /// Format: :sid TB #channel topic_ts [setter] :topic
    Tb {
        channel: String,
        topic_ts: i64,
        setter: Option<String>,
        topic: String,
    },

    /// BMASK - Batch ban/exception/invex list
    /// Format: :sid BMASK channel_ts #channel list_type :mask1 mask2 ...
    Bmask {
        channel_ts: i64,
        channel: String,
        list_type: char,
        masks: Vec<String>,
    },

    // === Runtime Operations ===
    /// PRIVMSG - Send private message
    /// Format: :source PRIVMSG target :text
    Privmsg { target: String, text: String },

    /// NOTICE - Send notice
    /// Format: :source NOTICE target :text
    Notice { target: String, text: String },

    /// JOIN - User joining channel (runtime, not burst)
    /// Format: :uid JOIN channel_ts #channel +
    Join { channel_ts: i64, channel: String },

    /// PART - Leave channel
    /// Format: :uid PART #channel [:reason]
    Part {
        channel: String,
        reason: Option<String>,
    },

    /// QUIT - User disconnecting
    /// Format: :uid QUIT [:reason]
    Quit { reason: Option<String> },

    /// NICK - Nickname change
    /// Format: :uid NICK newnick :ts
    Nick { nick: String, ts: i64 },

    /// KILL - Force disconnect user
    /// Format: :source KILL uid :path (reason)
    Kill {
        uid: String,
        path: String,
        reason: String,
    },

    /// KICK - Remove user from channel
    /// Format: :source KICK #channel uid :reason
    Kick {
        channel: String,
        uid: String,
        reason: String,
    },

    /// TMODE - Channel mode change with timestamp
    /// Format: :source TMODE channel_ts #channel modes [params...]
    Tmode {
        channel_ts: i64,
        channel: String,
        modes: String,
        params: Vec<String>,
    },

    /// TOPIC - Set channel topic (runtime)
    /// Format: :source TOPIC #channel setter ts :topic
    Topic {
        channel: String,
        setter: String,
        ts: i64,
        topic: String,
    },

    /// SQUIT - Server disconnection
    /// Format: :source SQUIT sid :reason
    Squit { sid: String, reason: String },

    /// PING - Connection keepalive
    /// Format: :source PING source [:target]
    Ping {
        source: String,
        target: Option<String>,
    },

    /// PONG - Ping reply
    /// Format: :source PONG source target
    Pong { source: String, target: String },

    /// ENCAP - Encapsulated command (for extensibility)
    /// Format: :source ENCAP target subcommand [params...]
    Encap {
        target: String,
        subcommand: String,
        params: Vec<String>,
    },

    /// MODE - User mode change (for self)
    /// Format: :uid MODE uid :+modes
    Mode { target: String, modes: String },

    /// AWAY - Set/unset away status
    /// Format: :uid AWAY [:reason]
    Away { reason: Option<String> },

    /// INVITE - Invite user to channel
    /// Format: :source INVITE uid #channel [channel_ts]
    Invite {
        uid: String,
        channel: String,
        channel_ts: Option<i64>,
    },

    /// WALLOPS - Send to all operators
    /// Format: :source WALLOPS :message
    Wallops { message: String },

    /// Unknown/passthrough command
    Unknown { command: String, params: Vec<String> },
}

/// Member in an SJOIN command.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SjoinMember {
    /// Status prefixes: "@" for op, "+" for voice, etc.
    pub prefixes: String,
    /// User ID
    pub uid: String,
}

impl SjoinMember {
    /// Create a new SJOIN member.
    pub fn new(uid: String) -> Self {
        Self {
            prefixes: String::new(),
            uid,
        }
    }

    /// Create a new SJOIN member with prefixes.
    pub fn with_prefixes(prefixes: String, uid: String) -> Self {
        Self { prefixes, uid }
    }

    /// Check if member has operator status.
    pub fn is_op(&self) -> bool {
        self.prefixes.contains('@')
    }

    /// Check if member has voice status.
    pub fn has_voice(&self) -> bool {
        self.prefixes.contains('+')
    }

    /// Parse from SJOIN format (e.g., "@00AAAAAAA" or "+00AAAAAAA").
    pub fn parse(s: &str) -> Self {
        let mut prefixes = String::new();
        let mut chars = s.chars().peekable();

        // Collect all prefix characters
        while let Some(&c) = chars.peek() {
            if c == '@' || c == '+' || c == '%' || c == '!' || c == '~' {
                prefixes.push(c);
                chars.next();
            } else {
                break;
            }
        }

        let uid: String = chars.collect();
        Self { prefixes, uid }
    }
}

impl fmt::Display for SjoinMember {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}{}", self.prefixes, self.uid)
    }
}

impl S2SCommand {
    /// Get the command name.
    pub fn name(&self) -> &str {
        match self {
            S2SCommand::Pass { .. } => "PASS",
            S2SCommand::Capab { .. } => "CAPAB",
            S2SCommand::Server { .. } => "SERVER",
            S2SCommand::SvInfo { .. } => "SVINFO",
            S2SCommand::Burst => "BURST",
            S2SCommand::EndBurst => "ENDBURST",
            S2SCommand::EndBurstAck => "ENDBURSTACKS",
            S2SCommand::Sid { .. } => "SID",
            S2SCommand::Uid { .. } => "UID",
            S2SCommand::Euid { .. } => "EUID",
            S2SCommand::Sjoin { .. } => "SJOIN",
            S2SCommand::Tb { .. } => "TB",
            S2SCommand::Bmask { .. } => "BMASK",
            S2SCommand::Privmsg { .. } => "PRIVMSG",
            S2SCommand::Notice { .. } => "NOTICE",
            S2SCommand::Join { .. } => "JOIN",
            S2SCommand::Part { .. } => "PART",
            S2SCommand::Quit { .. } => "QUIT",
            S2SCommand::Nick { .. } => "NICK",
            S2SCommand::Kill { .. } => "KILL",
            S2SCommand::Kick { .. } => "KICK",
            S2SCommand::Tmode { .. } => "TMODE",
            S2SCommand::Topic { .. } => "TOPIC",
            S2SCommand::Squit { .. } => "SQUIT",
            S2SCommand::Ping { .. } => "PING",
            S2SCommand::Pong { .. } => "PONG",
            S2SCommand::Encap { .. } => "ENCAP",
            S2SCommand::Mode { .. } => "MODE",
            S2SCommand::Away { .. } => "AWAY",
            S2SCommand::Invite { .. } => "INVITE",
            S2SCommand::Wallops { .. } => "WALLOPS",
            S2SCommand::Unknown { command, .. } => command,
        }
    }

    /// Parse an S2S command from a command name and parameters.
    pub fn parse(command: &str, params: Vec<String>) -> Result<Self, ParseError> {
        let cmd = command.to_uppercase();
        parse_s2s_command(&cmd, params)
    }
}

impl fmt::Display for S2SCommand {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            S2SCommand::Pass {
                password,
                ts_version,
                sid,
            } => {
                write!(f, "PASS {} TS {} {}", password, ts_version, sid)
            }

            S2SCommand::Capab { capabilities } => {
                write!(f, "CAPAB :{}", capabilities.join(" "))
            }

            S2SCommand::Server {
                name,
                hopcount,
                description,
            } => {
                write!(f, "SERVER {} {} :{}", name, hopcount, description)
            }

            S2SCommand::SvInfo {
                ts_version,
                ts_min,
                current_time,
            } => {
                write!(f, "SVINFO {} {} 0 :{}", ts_version, ts_min, current_time)
            }

            S2SCommand::Burst => write!(f, "BURST"),

            S2SCommand::EndBurst => write!(f, "ENDBURST"),

            S2SCommand::EndBurstAck => write!(f, "ENDBURSTACKS"),

            S2SCommand::Sid {
                name,
                hopcount,
                sid,
                description,
            } => {
                write!(f, "SID {} {} {} :{}", name, hopcount, sid, description)
            }

            S2SCommand::Uid {
                nick,
                hopcount,
                nick_ts,
                modes,
                user,
                host,
                ip,
                uid,
                realname,
            } => {
                write!(
                    f,
                    "UID {} {} {} {} {} {} {} {} :{}",
                    nick, hopcount, nick_ts, modes, user, host, ip, uid, realname
                )
            }

            S2SCommand::Euid {
                nick,
                hopcount,
                nick_ts,
                modes,
                user,
                visible_host,
                ip,
                uid,
                real_host,
                account,
                realname,
            } => {
                write!(
                    f,
                    "EUID {} {} {} {} {} {} {} {} {} {} :{}",
                    nick,
                    hopcount,
                    nick_ts,
                    modes,
                    user,
                    visible_host,
                    ip,
                    uid,
                    real_host,
                    account.as_deref().unwrap_or("*"),
                    realname
                )
            }

            S2SCommand::Sjoin {
                channel_ts,
                channel,
                modes,
                mode_params,
                members,
            } => {
                write!(f, "SJOIN {} {} {}", channel_ts, channel, modes)?;
                for p in mode_params {
                    write!(f, " {}", p)?;
                }
                let member_str: Vec<_> = members.iter().map(|m| m.to_string()).collect();
                write!(f, " :{}", member_str.join(" "))
            }

            S2SCommand::Tb {
                channel,
                topic_ts,
                setter,
                topic,
            } => {
                if let Some(s) = setter {
                    write!(f, "TB {} {} {} :{}", channel, topic_ts, s, topic)
                } else {
                    write!(f, "TB {} {} :{}", channel, topic_ts, topic)
                }
            }

            S2SCommand::Bmask {
                channel_ts,
                channel,
                list_type,
                masks,
            } => {
                write!(
                    f,
                    "BMASK {} {} {} :{}",
                    channel_ts,
                    channel,
                    list_type,
                    masks.join(" ")
                )
            }

            S2SCommand::Privmsg { target, text } => {
                write!(f, "PRIVMSG {} :{}", target, text)
            }

            S2SCommand::Notice { target, text } => {
                write!(f, "NOTICE {} :{}", target, text)
            }

            S2SCommand::Join { channel_ts, channel } => {
                write!(f, "JOIN {} {} +", channel_ts, channel)
            }

            S2SCommand::Part { channel, reason } => {
                if let Some(r) = reason {
                    write!(f, "PART {} :{}", channel, r)
                } else {
                    write!(f, "PART {}", channel)
                }
            }

            S2SCommand::Quit { reason } => {
                if let Some(r) = reason {
                    write!(f, "QUIT :{}", r)
                } else {
                    write!(f, "QUIT")
                }
            }

            S2SCommand::Nick { nick, ts } => {
                write!(f, "NICK {} :{}", nick, ts)
            }

            S2SCommand::Kill { uid, path, reason } => {
                write!(f, "KILL {} :{} ({})", uid, path, reason)
            }

            S2SCommand::Kick { channel, uid, reason } => {
                write!(f, "KICK {} {} :{}", channel, uid, reason)
            }

            S2SCommand::Tmode {
                channel_ts,
                channel,
                modes,
                params,
            } => {
                write!(f, "TMODE {} {} {}", channel_ts, channel, modes)?;
                for p in params {
                    write!(f, " {}", p)?;
                }
                Ok(())
            }

            S2SCommand::Topic {
                channel,
                setter,
                ts,
                topic,
            } => {
                write!(f, "TOPIC {} {} {} :{}", channel, setter, ts, topic)
            }

            S2SCommand::Squit { sid, reason } => {
                write!(f, "SQUIT {} :{}", sid, reason)
            }

            S2SCommand::Ping { source, target } => {
                if let Some(t) = target {
                    write!(f, "PING {} :{}", source, t)
                } else {
                    write!(f, "PING {}", source)
                }
            }

            S2SCommand::Pong { source, target } => {
                write!(f, "PONG {} {}", source, target)
            }

            S2SCommand::Encap {
                target,
                subcommand,
                params,
            } => {
                write!(f, "ENCAP {} {}", target, subcommand)?;
                for (i, p) in params.iter().enumerate() {
                    if i == params.len() - 1 && p.contains(' ') {
                        write!(f, " :{}", p)?;
                    } else {
                        write!(f, " {}", p)?;
                    }
                }
                Ok(())
            }

            S2SCommand::Mode { target, modes } => {
                write!(f, "MODE {} :{}", target, modes)
            }

            S2SCommand::Away { reason } => {
                if let Some(r) = reason {
                    write!(f, "AWAY :{}", r)
                } else {
                    write!(f, "AWAY")
                }
            }

            S2SCommand::Invite {
                uid,
                channel,
                channel_ts,
            } => {
                if let Some(ts) = channel_ts {
                    write!(f, "INVITE {} {} {}", uid, channel, ts)
                } else {
                    write!(f, "INVITE {} {}", uid, channel)
                }
            }

            S2SCommand::Wallops { message } => {
                write!(f, "WALLOPS :{}", message)
            }

            S2SCommand::Unknown { command, params } => {
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

/// Parse an S2S command from command name and parameters.
fn parse_s2s_command(command: &str, params: Vec<String>) -> Result<S2SCommand, ParseError> {
    match command {
        "PASS" => {
            // PASS password TS ts_version sid
            let mut iter = params.into_iter();
            let password = iter.next().unwrap_or_default();
            let _ts = iter.next(); // Skip "TS" literal
            let ts_version = iter
                .next()
                .and_then(|s| s.parse().ok())
                .unwrap_or(6);
            let sid = iter.next().unwrap_or_default();
            Ok(S2SCommand::Pass {
                password,
                ts_version,
                sid,
            })
        }

        "CAPAB" => {
            let caps = params
                .into_iter()
                .flat_map(|s| s.split_whitespace().map(String::from).collect::<Vec<_>>())
                .collect();
            Ok(S2SCommand::Capab { capabilities: caps })
        }

        "SERVER" => {
            let mut iter = params.into_iter();
            Ok(S2SCommand::Server {
                name: iter.next().unwrap_or_default(),
                hopcount: iter.next().and_then(|s| s.parse().ok()).unwrap_or(1),
                description: iter.next().unwrap_or_default(),
            })
        }

        "SVINFO" => {
            // SVINFO ts_version ts_min 0 :current_time
            let mut iter = params.into_iter();
            Ok(S2SCommand::SvInfo {
                ts_version: iter.next().and_then(|s| s.parse().ok()).unwrap_or(6),
                ts_min: iter.next().and_then(|s| s.parse().ok()).unwrap_or(6),
                current_time: iter
                    .nth(1) // Skip the "0"
                    .and_then(|s| s.parse().ok())
                    .unwrap_or(0),
            })
        }

        "BURST" => Ok(S2SCommand::Burst),

        "ENDBURST" => Ok(S2SCommand::EndBurst),

        "ENDBURSTACKS" => Ok(S2SCommand::EndBurstAck),

        "SID" => {
            let mut iter = params.into_iter();
            Ok(S2SCommand::Sid {
                name: iter.next().unwrap_or_default(),
                hopcount: iter.next().and_then(|s| s.parse().ok()).unwrap_or(1),
                sid: iter.next().unwrap_or_default(),
                description: iter.next().unwrap_or_default(),
            })
        }

        "UID" => {
            // UID nick hopcount nick_ts modes user host ip uid :realname
            let mut iter = params.into_iter();
            Ok(S2SCommand::Uid {
                nick: iter.next().unwrap_or_default(),
                hopcount: iter.next().and_then(|s| s.parse().ok()).unwrap_or(1),
                nick_ts: iter.next().and_then(|s| s.parse().ok()).unwrap_or(0),
                modes: iter.next().unwrap_or_default(),
                user: iter.next().unwrap_or_default(),
                host: iter.next().unwrap_or_default(),
                ip: iter.next().unwrap_or_default(),
                uid: iter.next().unwrap_or_default(),
                realname: iter.next().unwrap_or_default(),
            })
        }

        "EUID" => {
            // EUID nick hopcount nick_ts modes user visible_host ip uid real_host account :realname
            let mut iter = params.into_iter();
            let account_raw = iter.clone().nth(9).unwrap_or_default();
            let account = if account_raw == "*" {
                None
            } else {
                Some(account_raw)
            };

            Ok(S2SCommand::Euid {
                nick: iter.next().unwrap_or_default(),
                hopcount: iter.next().and_then(|s| s.parse().ok()).unwrap_or(1),
                nick_ts: iter.next().and_then(|s| s.parse().ok()).unwrap_or(0),
                modes: iter.next().unwrap_or_default(),
                user: iter.next().unwrap_or_default(),
                visible_host: iter.next().unwrap_or_default(),
                ip: iter.next().unwrap_or_default(),
                uid: iter.next().unwrap_or_default(),
                real_host: iter.next().unwrap_or_default(),
                account,
                realname: iter.nth(1).unwrap_or_default(), // Skip account (already processed)
            })
        }

        "SJOIN" => {
            // SJOIN channel_ts #channel +modes [mode_params] :[@+]uid ...
            let mut iter = params.into_iter();
            let channel_ts = iter.next().and_then(|s| s.parse().ok()).unwrap_or(0);
            let channel = iter.next().unwrap_or_default();
            let modes = iter.next().unwrap_or_else(|| "+".to_string());

            // Collect remaining params - last one is the member list
            let remaining: Vec<_> = iter.collect();
            let (mode_params, members_str) = if let Some((last, rest)) = remaining.split_last() {
                (rest.to_vec(), last.as_str())
            } else {
                (Vec::new(), "")
            };

            let members = members_str
                .split_whitespace()
                .filter(|s| !s.is_empty())
                .map(SjoinMember::parse)
                .collect();

            Ok(S2SCommand::Sjoin {
                channel_ts,
                channel,
                modes,
                mode_params,
                members,
            })
        }

        "TB" => {
            // TB #channel topic_ts [setter] :topic
            let mut iter = params.into_iter();
            let channel = iter.next().unwrap_or_default();
            let topic_ts = iter.next().and_then(|s| s.parse().ok()).unwrap_or(0);
            let remaining: Vec<_> = iter.collect();

            // If there are 2 remaining, first is setter, second is topic
            // If there's 1 remaining, it's the topic (no setter)
            let (setter, topic) = if remaining.len() >= 2 {
                (Some(remaining[0].clone()), remaining[1].clone())
            } else {
                (None, remaining.into_iter().next().unwrap_or_default())
            };

            Ok(S2SCommand::Tb {
                channel,
                topic_ts,
                setter,
                topic,
            })
        }

        "BMASK" => {
            // BMASK channel_ts #channel list_type :mask1 mask2 ...
            let mut iter = params.into_iter();
            let channel_ts = iter.next().and_then(|s| s.parse().ok()).unwrap_or(0);
            let channel = iter.next().unwrap_or_default();
            let list_type = iter
                .next()
                .and_then(|s| s.chars().next())
                .unwrap_or('b');
            let masks_str = iter.next().unwrap_or_default();
            let masks = masks_str
                .split_whitespace()
                .map(String::from)
                .collect();

            Ok(S2SCommand::Bmask {
                channel_ts,
                channel,
                list_type,
                masks,
            })
        }

        "PRIVMSG" => {
            let mut iter = params.into_iter();
            Ok(S2SCommand::Privmsg {
                target: iter.next().unwrap_or_default(),
                text: iter.next().unwrap_or_default(),
            })
        }

        "NOTICE" => {
            let mut iter = params.into_iter();
            Ok(S2SCommand::Notice {
                target: iter.next().unwrap_or_default(),
                text: iter.next().unwrap_or_default(),
            })
        }

        "JOIN" => {
            // :uid JOIN channel_ts #channel +
            let mut iter = params.into_iter();
            Ok(S2SCommand::Join {
                channel_ts: iter.next().and_then(|s| s.parse().ok()).unwrap_or(0),
                channel: iter.next().unwrap_or_default(),
            })
        }

        "PART" => {
            let mut iter = params.into_iter();
            Ok(S2SCommand::Part {
                channel: iter.next().unwrap_or_default(),
                reason: iter.next(),
            })
        }

        "QUIT" => Ok(S2SCommand::Quit {
            reason: params.into_iter().next(),
        }),

        "NICK" => {
            let mut iter = params.into_iter();
            Ok(S2SCommand::Nick {
                nick: iter.next().unwrap_or_default(),
                ts: iter.next().and_then(|s| s.parse().ok()).unwrap_or(0),
            })
        }

        "KILL" => {
            let mut iter = params.into_iter();
            let uid = iter.next().unwrap_or_default();
            let message = iter.next().unwrap_or_default();

            // Parse "path (reason)" format
            let (path, reason) = if let Some(paren_start) = message.find('(') {
                let path = message[..paren_start].trim().to_string();
                let reason = message[paren_start + 1..]
                    .trim_end_matches(')')
                    .to_string();
                (path, reason)
            } else {
                (message.clone(), message)
            };

            Ok(S2SCommand::Kill { uid, path, reason })
        }

        "KICK" => {
            let mut iter = params.into_iter();
            Ok(S2SCommand::Kick {
                channel: iter.next().unwrap_or_default(),
                uid: iter.next().unwrap_or_default(),
                reason: iter.next().unwrap_or_default(),
            })
        }

        "TMODE" => {
            let mut iter = params.into_iter();
            Ok(S2SCommand::Tmode {
                channel_ts: iter.next().and_then(|s| s.parse().ok()).unwrap_or(0),
                channel: iter.next().unwrap_or_default(),
                modes: iter.next().unwrap_or_default(),
                params: iter.collect(),
            })
        }

        "TOPIC" => {
            let mut iter = params.into_iter();
            Ok(S2SCommand::Topic {
                channel: iter.next().unwrap_or_default(),
                setter: iter.next().unwrap_or_default(),
                ts: iter.next().and_then(|s| s.parse().ok()).unwrap_or(0),
                topic: iter.next().unwrap_or_default(),
            })
        }

        "SQUIT" => {
            let mut iter = params.into_iter();
            Ok(S2SCommand::Squit {
                sid: iter.next().unwrap_or_default(),
                reason: iter.next().unwrap_or_default(),
            })
        }

        "PING" => {
            let mut iter = params.into_iter();
            Ok(S2SCommand::Ping {
                source: iter.next().unwrap_or_default(),
                target: iter.next(),
            })
        }

        "PONG" => {
            let mut iter = params.into_iter();
            Ok(S2SCommand::Pong {
                source: iter.next().unwrap_or_default(),
                target: iter.next().unwrap_or_default(),
            })
        }

        "ENCAP" => {
            let mut iter = params.into_iter();
            Ok(S2SCommand::Encap {
                target: iter.next().unwrap_or_default(),
                subcommand: iter.next().unwrap_or_default(),
                params: iter.collect(),
            })
        }

        "MODE" => {
            let mut iter = params.into_iter();
            Ok(S2SCommand::Mode {
                target: iter.next().unwrap_or_default(),
                modes: iter.next().unwrap_or_default(),
            })
        }

        "AWAY" => Ok(S2SCommand::Away {
            reason: params.into_iter().next(),
        }),

        "INVITE" => {
            let mut iter = params.into_iter();
            Ok(S2SCommand::Invite {
                uid: iter.next().unwrap_or_default(),
                channel: iter.next().unwrap_or_default(),
                channel_ts: iter.next().and_then(|s| s.parse().ok()),
            })
        }

        "WALLOPS" => Ok(S2SCommand::Wallops {
            message: params.into_iter().next().unwrap_or_default(),
        }),

        _ => Ok(S2SCommand::Unknown {
            command: command.to_string(),
            params,
        }),
    }
}

/// An S2S message with prefix and command.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct S2SMessage {
    /// Source SID or UID (without leading colon)
    pub source: Option<String>,
    /// The S2S command
    pub command: S2SCommand,
}

impl S2SMessage {
    /// Create a new S2S message.
    pub fn new(command: S2SCommand) -> Self {
        Self {
            source: None,
            command,
        }
    }

    /// Create a new S2S message with a source.
    pub fn with_source(source: String, command: S2SCommand) -> Self {
        Self {
            source: Some(source),
            command,
        }
    }

    /// Parse an S2S message from a line (without CRLF).
    pub fn parse(input: &str) -> Result<Self, ParseError> {
        let mut remaining = input.trim();

        // Parse source (optional, starts with :)
        let source = if remaining.starts_with(':') {
            let space_idx = remaining.find(' ').ok_or(ParseError::EmptyCommand)?;
            let src = &remaining[1..space_idx];
            remaining = remaining[space_idx..].trim_start();
            Some(src.to_string())
        } else {
            None
        };

        // Parse command
        let (command_str, params_str) = match remaining.find(' ') {
            Some(idx) => (&remaining[..idx], Some(&remaining[idx + 1..])),
            None => (remaining, None),
        };

        let params = parse_s2s_params(params_str.unwrap_or(""));
        let command = S2SCommand::parse(command_str, params)?;

        Ok(Self { source, command })
    }

    /// Serialize the message to a string (without CRLF).
    pub fn to_line(&self) -> String {
        if let Some(ref src) = self.source {
            format!(":{} {}", src, self.command)
        } else {
            self.command.to_string()
        }
    }

    /// Serialize the message to bytes (with CRLF).
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut bytes = self.to_line().into_bytes();
        bytes.extend_from_slice(b"\r\n");
        bytes
    }
}

impl fmt::Display for S2SMessage {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if let Some(ref src) = self.source {
            write!(f, ":{} {}", src, self.command)
        } else {
            write!(f, "{}", self.command)
        }
    }
}

/// Parse S2S message parameters (same format as C2S).
fn parse_s2s_params(input: &str) -> Vec<String> {
    if input.is_empty() {
        return Vec::new();
    }

    let mut params = Vec::new();
    let mut remaining = input;

    while !remaining.is_empty() {
        if let Some(trailing) = remaining.strip_prefix(':') {
            params.push(trailing.to_string());
            break;
        }

        match remaining.find(' ') {
            Some(idx) => {
                let param = &remaining[..idx];
                if !param.is_empty() {
                    params.push(param.to_string());
                }
                remaining = &remaining[idx + 1..];
            }
            None => {
                if !remaining.is_empty() {
                    params.push(remaining.to_string());
                }
                break;
            }
        }
    }

    params
}

/// Validate a Server ID (SID).
/// SIDs are exactly 3 alphanumeric characters.
pub fn validate_sid(sid: &str) -> bool {
    sid.len() == 3 && sid.chars().all(|c| c.is_ascii_alphanumeric())
}

/// Validate a User ID (UID).
/// UIDs are SID (3 chars) + 6 alphanumeric characters = 9 chars total.
pub fn validate_uid(uid: &str) -> bool {
    uid.len() == 9 && uid.chars().all(|c| c.is_ascii_alphanumeric())
}

/// Extract the SID from a UID.
pub fn uid_to_sid(uid: &str) -> Option<&str> {
    if validate_uid(uid) {
        Some(&uid[..3])
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_pass() {
        let cmd = S2SCommand::parse("PASS", vec![
            "linkpass".into(),
            "TS".into(),
            "6".into(),
            "00B".into(),
        ])
        .unwrap();

        if let S2SCommand::Pass {
            password,
            ts_version,
            sid,
        } = cmd
        {
            assert_eq!(password, "linkpass");
            assert_eq!(ts_version, 6);
            assert_eq!(sid, "00B");
        } else {
            panic!("Expected Pass command");
        }
    }

    #[test]
    fn test_parse_sjoin() {
        let cmd = S2SCommand::parse("SJOIN", vec![
            "1234567890".into(),
            "#test".into(),
            "+nt".into(),
            "@00AAAAAAA +00AAAAAAB 00AAAAAAC".into(),
        ])
        .unwrap();

        if let S2SCommand::Sjoin {
            channel_ts,
            channel,
            modes,
            members,
            ..
        } = cmd
        {
            assert_eq!(channel_ts, 1234567890);
            assert_eq!(channel, "#test");
            assert_eq!(modes, "+nt");
            assert_eq!(members.len(), 3);
            assert!(members[0].is_op());
            assert_eq!(members[0].uid, "00AAAAAAA");
            assert!(members[1].has_voice());
            assert_eq!(members[1].uid, "00AAAAAAB");
            assert!(!members[2].is_op());
            assert!(!members[2].has_voice());
        } else {
            panic!("Expected Sjoin command");
        }
    }

    #[test]
    fn test_parse_euid() {
        let cmd = S2SCommand::parse("EUID", vec![
            "TestUser".into(),
            "1".into(),
            "1234567890".into(),
            "+i".into(),
            "test".into(),
            "visible.host.com".into(),
            "127.0.0.1".into(),
            "00AAAAAAA".into(),
            "real.host.com".into(),
            "account1".into(),
            "Test User".into(),
        ])
        .unwrap();

        if let S2SCommand::Euid {
            nick,
            uid,
            account,
            realname,
            ..
        } = cmd
        {
            assert_eq!(nick, "TestUser");
            assert_eq!(uid, "00AAAAAAA");
            assert_eq!(account, Some("account1".into()));
            assert_eq!(realname, "Test User");
        } else {
            panic!("Expected Euid command");
        }
    }

    #[test]
    fn test_s2s_message_parse() {
        let msg = S2SMessage::parse(":00A SJOIN 1234567890 #test +nt :@00AAAAAAA").unwrap();
        assert_eq!(msg.source, Some("00A".into()));
        assert!(matches!(msg.command, S2SCommand::Sjoin { .. }));
    }

    #[test]
    fn test_s2s_message_display() {
        let msg = S2SMessage::with_source(
            "00A".into(),
            S2SCommand::Privmsg {
                target: "#test".into(),
                text: "Hello world".into(),
            },
        );
        assert_eq!(msg.to_string(), ":00A PRIVMSG #test :Hello world");
    }

    #[test]
    fn test_validate_sid() {
        assert!(validate_sid("00A"));
        assert!(validate_sid("ABC"));
        assert!(validate_sid("123"));
        assert!(!validate_sid("00"));
        assert!(!validate_sid("00AB"));
        assert!(!validate_sid("00!"));
    }

    #[test]
    fn test_validate_uid() {
        assert!(validate_uid("00AAAAAAA"));
        assert!(validate_uid("ABC123456"));
        assert!(!validate_uid("00A"));
        assert!(!validate_uid("00AAAAAAAA")); // 10 chars
        assert!(!validate_uid("00AAAAA!")); // invalid char
    }

    #[test]
    fn test_uid_to_sid() {
        assert_eq!(uid_to_sid("00AAAAAAA"), Some("00A"));
        assert_eq!(uid_to_sid("ABC123456"), Some("ABC"));
        assert_eq!(uid_to_sid("00A"), None);
    }

    #[test]
    fn test_sjoin_member_parse() {
        let m1 = SjoinMember::parse("@00AAAAAAA");
        assert!(m1.is_op());
        assert!(!m1.has_voice());
        assert_eq!(m1.uid, "00AAAAAAA");

        let m2 = SjoinMember::parse("+00AAAAAAB");
        assert!(!m2.is_op());
        assert!(m2.has_voice());

        let m3 = SjoinMember::parse("@+00AAAAAAC");
        assert!(m3.is_op());
        assert!(m3.has_voice());

        let m4 = SjoinMember::parse("00AAAAAAD");
        assert!(!m4.is_op());
        assert!(!m4.has_voice());
    }
}
