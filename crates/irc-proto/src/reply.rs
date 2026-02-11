//! IRC numeric reply codes.
//!
//! These constants represent the standard IRC numeric replies
//! as defined in RFC 2812 and extended by IRCv3.

/// Reply codes (001-099, 200-399)
pub mod reply {
    // === Connection Registration ===

    /// Welcome to the network
    pub const RPL_WELCOME: u16 = 1;
    /// Your host is...
    pub const RPL_YOURHOST: u16 = 2;
    /// Server created...
    pub const RPL_CREATED: u16 = 3;
    /// Server info (name, version, modes)
    pub const RPL_MYINFO: u16 = 4;
    /// ISUPPORT / server capabilities
    pub const RPL_ISUPPORT: u16 = 5;

    // === Command Responses ===

    /// User mode string
    pub const RPL_UMODEIS: u16 = 221;

    // === LUSERS ===

    /// User/invisible/server count
    pub const RPL_LUSERCLIENT: u16 = 251;
    /// Operator count
    pub const RPL_LUSEROP: u16 = 252;
    /// Unknown connections
    pub const RPL_LUSERUNKNOWN: u16 = 253;
    /// Channel count
    pub const RPL_LUSERCHANNELS: u16 = 254;
    /// Local user count
    pub const RPL_LUSERME: u16 = 255;
    /// Local/global max users
    pub const RPL_LOCALUSERS: u16 = 265;
    /// Global users
    pub const RPL_GLOBALUSERS: u16 = 266;

    // === AWAY ===

    /// User is away
    pub const RPL_AWAY: u16 = 301;
    /// You are no longer away
    pub const RPL_UNAWAY: u16 = 305;
    /// You are now away
    pub const RPL_NOWAWAY: u16 = 306;

    // === WHOIS ===

    /// WHOIS user info
    pub const RPL_WHOISUSER: u16 = 311;
    /// WHOIS server info
    pub const RPL_WHOISSERVER: u16 = 312;
    /// WHOIS operator status
    pub const RPL_WHOISOPERATOR: u16 = 313;
    /// WHOIS idle time
    pub const RPL_WHOISIDLE: u16 = 317;
    /// End of WHOIS
    pub const RPL_ENDOFWHOIS: u16 = 318;
    /// WHOIS channels
    pub const RPL_WHOISCHANNELS: u16 = 319;
    /// WHOIS account
    pub const RPL_WHOISACCOUNT: u16 = 330;
    /// WHOIS actually (real host)
    pub const RPL_WHOISACTUALLY: u16 = 338;

    // === WHOWAS ===

    /// WHOWAS user info
    pub const RPL_WHOWASUSER: u16 = 314;
    /// End of WHOWAS
    pub const RPL_ENDOFWHOWAS: u16 = 369;

    // === LIST ===

    /// Start of LIST
    pub const RPL_LISTSTART: u16 = 321;
    /// LIST entry
    pub const RPL_LIST: u16 = 322;
    /// End of LIST
    pub const RPL_LISTEND: u16 = 323;

    // === Channel Info ===

    /// Channel modes
    pub const RPL_CHANNELMODEIS: u16 = 324;
    /// Channel creation time
    pub const RPL_CREATIONTIME: u16 = 329;
    /// No topic set
    pub const RPL_NOTOPIC: u16 = 331;
    /// Topic
    pub const RPL_TOPIC: u16 = 332;
    /// Topic setter and time
    pub const RPL_TOPICWHOTIME: u16 = 333;

    // === INVITE ===

    /// Invite sent
    pub const RPL_INVITING: u16 = 341;

    // === WHO ===

    /// WHO reply
    pub const RPL_WHOREPLY: u16 = 352;
    /// End of WHO
    pub const RPL_ENDOFWHO: u16 = 315;

    // === NAMES ===

    /// Names list
    pub const RPL_NAMREPLY: u16 = 353;
    /// End of NAMES
    pub const RPL_ENDOFNAMES: u16 = 366;

    // === Ban List ===

    /// Ban list entry
    pub const RPL_BANLIST: u16 = 367;
    /// End of ban list
    pub const RPL_ENDOFBANLIST: u16 = 368;

    // === Exception List ===

    /// Exception list entry
    pub const RPL_EXCEPTLIST: u16 = 348;
    /// End of exception list
    pub const RPL_ENDOFEXCEPTLIST: u16 = 349;

    // === Invite List ===

    /// Invite list entry
    pub const RPL_INVITELIST: u16 = 346;
    /// End of invite list
    pub const RPL_ENDOFINVITELIST: u16 = 347;

    // === MOTD ===

    /// Start of MOTD
    pub const RPL_MOTDSTART: u16 = 375;
    /// MOTD line
    pub const RPL_MOTD: u16 = 372;
    /// End of MOTD
    pub const RPL_ENDOFMOTD: u16 = 376;

    // === Operator ===

    /// You are now an operator
    pub const RPL_YOUREOPER: u16 = 381;

    // === Server Info ===

    /// Server version
    pub const RPL_VERSION: u16 = 351;
    /// Server time
    pub const RPL_TIME: u16 = 391;

    // === STATS ===

    /// Stats link info
    pub const RPL_STATSLINKINFO: u16 = 211;
    /// Stats commands
    pub const RPL_STATSCOMMANDS: u16 = 212;
    /// Stats uptime
    pub const RPL_STATSUPTIME: u16 = 242;
    /// End of STATS
    pub const RPL_ENDOFSTATS: u16 = 219;

    // === ADMIN ===

    /// Admin server info
    pub const RPL_ADMINME: u16 = 256;
    /// Admin location 1
    pub const RPL_ADMINLOC1: u16 = 257;
    /// Admin location 2
    pub const RPL_ADMINLOC2: u16 = 258;
    /// Admin email
    pub const RPL_ADMINEMAIL: u16 = 259;

    // === INFO ===

    /// Info line
    pub const RPL_INFO: u16 = 371;
    /// End of INFO
    pub const RPL_ENDOFINFO: u16 = 374;

    // === SASL ===

    /// Logged in
    pub const RPL_LOGGEDIN: u16 = 900;
    /// Logged out
    pub const RPL_LOGGEDOUT: u16 = 901;
    /// SASL successful
    pub const RPL_SASLSUCCESS: u16 = 903;
    /// SASL mechanisms
    pub const RPL_SASLMECHS: u16 = 908;

    // === Monitor ===

    /// User is online
    pub const RPL_MONONLINE: u16 = 730;
    /// User is offline
    pub const RPL_MONOFFLINE: u16 = 731;
    /// Monitor list
    pub const RPL_MONLIST: u16 = 732;
    /// End of monitor list
    pub const RPL_ENDOFMONLIST: u16 = 733;
}

/// Error codes (400-599)
pub mod error {
    /// No such nick/channel
    pub const ERR_NOSUCHNICK: u16 = 401;
    /// No such server
    pub const ERR_NOSUCHSERVER: u16 = 402;
    /// No such channel
    pub const ERR_NOSUCHCHANNEL: u16 = 403;
    /// Cannot send to channel
    pub const ERR_CANNOTSENDTOCHAN: u16 = 404;
    /// Too many channels
    pub const ERR_TOOMANYCHANNELS: u16 = 405;
    /// Was no such nick
    pub const ERR_WASNOSUCHNICK: u16 = 406;
    /// Too many targets
    pub const ERR_TOOMANYTARGETS: u16 = 407;
    /// No origin
    pub const ERR_NOORIGIN: u16 = 409;

    /// No recipient
    pub const ERR_NORECIPIENT: u16 = 411;
    /// No text to send
    pub const ERR_NOTEXTTOSEND: u16 = 412;
    /// No top level
    pub const ERR_NOTOPLEVEL: u16 = 413;
    /// Wild top level
    pub const ERR_WILDTOPLEVEL: u16 = 414;

    /// Unknown command
    pub const ERR_UNKNOWNCOMMAND: u16 = 421;
    /// No MOTD
    pub const ERR_NOMOTD: u16 = 422;
    /// No admin info
    pub const ERR_NOADMININFO: u16 = 423;
    /// File error
    pub const ERR_FILEERROR: u16 = 424;

    /// No nickname given
    pub const ERR_NONICKNAMEGIVEN: u16 = 431;
    /// Erroneous nickname
    pub const ERR_ERRONEUSNICKNAME: u16 = 432;
    /// Nickname in use
    pub const ERR_NICKNAMEINUSE: u16 = 433;
    /// Nick collision
    pub const ERR_NICKCOLLISION: u16 = 436;
    /// Unavailable resource
    pub const ERR_UNAVAILRESOURCE: u16 = 437;

    /// User not in channel
    pub const ERR_USERNOTINCHANNEL: u16 = 441;
    /// Not on channel
    pub const ERR_NOTONCHANNEL: u16 = 442;
    /// User on channel
    pub const ERR_USERONCHANNEL: u16 = 443;
    /// No login
    pub const ERR_NOLOGIN: u16 = 444;
    /// Summon disabled
    pub const ERR_SUMMONDISABLED: u16 = 445;
    /// Users disabled
    pub const ERR_USERSDISABLED: u16 = 446;

    /// Not registered
    pub const ERR_NOTREGISTERED: u16 = 451;

    /// Need more params
    pub const ERR_NEEDMOREPARAMS: u16 = 461;
    /// Already registered
    pub const ERR_ALREADYREGISTERED: u16 = 462;
    /// No permission for host
    pub const ERR_NOPERMFORHOST: u16 = 463;
    /// Password mismatch
    pub const ERR_PASSWDMISMATCH: u16 = 464;
    /// You're banned
    pub const ERR_YOUREBANNEDCREEP: u16 = 465;
    /// Key set
    pub const ERR_KEYSET: u16 = 467;

    /// Channel is full
    pub const ERR_CHANNELISFULL: u16 = 471;
    /// Unknown mode
    pub const ERR_UNKNOWNMODE: u16 = 472;
    /// Invite only channel
    pub const ERR_INVITEONLYCHAN: u16 = 473;
    /// Banned from channel
    pub const ERR_BANNEDFROMCHAN: u16 = 474;
    /// Bad channel key
    pub const ERR_BADCHANNELKEY: u16 = 475;
    /// Bad channel mask
    pub const ERR_BADCHANMASK: u16 = 476;
    /// No channel modes
    pub const ERR_NOCHANMODES: u16 = 477;
    /// Ban list full
    pub const ERR_BANLISTFULL: u16 = 478;

    /// No privileges
    pub const ERR_NOPRIVILEGES: u16 = 481;
    /// Channel op privileges needed
    pub const ERR_CHANOPRIVSNEEDED: u16 = 482;
    /// Can't kill server
    pub const ERR_CANTKILLSERVER: u16 = 483;
    /// Restricted
    pub const ERR_RESTRICTED: u16 = 484;
    /// Unique op privileges needed
    pub const ERR_UNIQOPRIVSNEEDED: u16 = 485;

    /// No oper host
    pub const ERR_NOOPERHOST: u16 = 491;

    /// User mode unknown flag
    pub const ERR_UMODEUNKNOWNFLAG: u16 = 501;
    /// Users don't match
    pub const ERR_USERSDONTMATCH: u16 = 502;

    // === SASL Errors ===

    /// Nick locked
    pub const ERR_NICKLOCKED: u16 = 902;
    /// SASL failed
    pub const ERR_SASLFAIL: u16 = 904;
    /// SASL too long
    pub const ERR_SASLTOOLONG: u16 = 905;
    /// SASL aborted
    pub const ERR_SASLABORTED: u16 = 906;
    /// SASL already authenticated
    pub const ERR_SASLALREADY: u16 = 907;
}

/// Get a human-readable description for a numeric code.
pub fn describe(code: u16) -> &'static str {
    match code {
        1 => "Welcome",
        2 => "Your host",
        3 => "Server created",
        4 => "Server info",
        5 => "Server supports",

        301 => "Away",
        305 => "No longer away",
        306 => "Now away",

        311 => "WHOIS user",
        318 => "End of WHOIS",

        331 => "No topic",
        332 => "Topic",
        333 => "Topic set by",

        353 => "Names list",
        366 => "End of names",

        372 => "MOTD",
        375 => "MOTD start",
        376 => "End of MOTD",

        401 => "No such nick/channel",
        403 => "No such channel",
        404 => "Cannot send to channel",
        421 => "Unknown command",
        422 => "No MOTD",
        431 => "No nickname given",
        432 => "Erroneous nickname",
        433 => "Nickname in use",
        442 => "Not on channel",
        451 => "Not registered",
        461 => "Not enough parameters",
        462 => "Already registered",
        464 => "Password mismatch",
        471 => "Channel is full",
        473 => "Invite only",
        474 => "Banned",
        475 => "Bad channel key",
        482 => "Not channel operator",

        903 => "SASL success",
        904 => "SASL failed",

        _ => "Unknown",
    }
}
