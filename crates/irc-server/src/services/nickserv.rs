//! NickServ service for nickname registration and identification.

use argon2::{
    Argon2,
    password_hash::{PasswordHash, PasswordHasher, PasswordVerifier, SaltString, rand_core::OsRng},
};

use super::ServiceContext;
use crate::cap::extensions::broadcast_account_notify;
use crate::db::{accounts, nicks};
use crate::error::Result;

/// Handle a NickServ command.
pub fn handle_command(sctx: &ServiceContext, args: &[&str]) -> Result<()> {
    match args.first().map(|s| s.to_uppercase()).as_deref() {
        Some("HELP") => cmd_help(sctx),
        Some("REGISTER") => cmd_register(sctx, &args[1..]),
        Some("IDENTIFY") | Some("LOGIN") | Some("ID") => cmd_identify(sctx, &args[1..]),
        Some("LOGOUT") => cmd_logout(sctx),
        Some("INFO") => cmd_info(sctx, &args[1..]),
        Some("SET") => cmd_set(sctx, &args[1..]),
        Some("DROP") => cmd_drop(sctx, &args[1..]),
        Some("GHOST") => cmd_ghost(sctx, &args[1..]),
        Some(cmd) => {
            sctx.error(&format!(
                "Unknown command: {}. Use HELP for a list of commands.",
                cmd
            ))?;
            Ok(())
        }
        None => cmd_help(sctx),
    }
}

/// Show help message.
fn cmd_help(sctx: &ServiceContext) -> Result<()> {
    sctx.reply("***** NickServ Help *****")?;
    sctx.reply(" ")?;
    sctx.reply("NickServ allows you to register and protect your nickname.")?;
    sctx.reply(" ")?;
    sctx.reply("Commands:")?;
    sctx.reply("  REGISTER <password> [email]  - Register your current nickname")?;
    sctx.reply("  IDENTIFY [nick] <password>   - Log in to your account")?;
    sctx.reply("  LOGOUT                       - Log out of your account")?;
    sctx.reply("  INFO [nick]                  - Show info about a nickname")?;
    sctx.reply("  SET PASSWORD <newpass>       - Change your password")?;
    sctx.reply("  DROP <password>              - Unregister your nickname")?;
    sctx.reply("  GHOST <nick>                 - Disconnect a session using your nick")?;
    sctx.reply(" ")?;
    sctx.reply("***** End of Help *****")?;
    Ok(())
}

/// Register the current nickname.
fn cmd_register(sctx: &ServiceContext, args: &[&str]) -> Result<()> {
    let db = sctx.require_db()?;
    let conn = db.connection()?;
    let nick = sctx.nickname()?;

    // Check if already logged in
    if sctx.ctx.client.account()?.is_some() {
        sctx.error("You are already logged in. Use LOGOUT first.")?;
        return Ok(());
    }

    // Get password
    let password = match args.first() {
        Some(p) => *p,
        None => {
            sctx.error("Usage: REGISTER <password> [email]")?;
            return Ok(());
        }
    };

    let email = args.get(1).copied();

    // Check if nick is already registered
    if nicks::is_registered(&conn, &nick)? {
        sctx.error(&format!("The nickname {} is already registered.", nick))?;
        return Ok(());
    }

    // Check if account name is already taken
    if accounts::exists(&conn, &nick)? {
        sctx.error(&format!("An account named {} already exists.", nick))?;
        return Ok(());
    }

    // Hash the password
    let password_hash = hash_password(password)?;

    // Create account and register nick
    let account_id = accounts::create(&conn, &nick, &password_hash, email)?;
    nicks::register(&conn, &nick, account_id, true)?;

    // Log in the user
    sctx.ctx.client.set_account(nick.clone())?;

    // Broadcast account-notify
    let _ = broadcast_account_notify(sctx.ctx, Some(&nick));

    sctx.reply(&format!(
        "Nickname {} registered to your account. You are now identified.",
        nick
    ))?;

    tracing::info!(nick = %nick, "Nickname registered via NickServ");

    Ok(())
}

/// Identify to an account.
fn cmd_identify(sctx: &ServiceContext, args: &[&str]) -> Result<()> {
    let db = sctx.require_db()?;
    let conn = db.connection()?;

    // Check if already logged in
    if sctx.ctx.client.account()?.is_some() {
        sctx.error("You are already logged in. Use LOGOUT first.")?;
        return Ok(());
    }

    // Parse arguments: IDENTIFY [nick] <password>
    let (account_name, password) = match args.len() {
        1 => {
            // Just password - use current nick as account name
            let nick = sctx.nickname()?;
            (nick, args[0])
        }
        2 => {
            // Nick and password
            (args[0].to_string(), args[1])
        }
        _ => {
            sctx.error("Usage: IDENTIFY [nick] <password>")?;
            return Ok(());
        }
    };

    // Find the account
    let account = match accounts::find_by_name(&conn, &account_name)? {
        Some(acc) => acc,
        None => {
            sctx.error("Invalid credentials.")?;
            return Ok(());
        }
    };

    // Verify password
    if !verify_password(password, &account.password_hash) {
        sctx.error("Invalid credentials.")?;
        return Ok(());
    }

    // Update last seen
    accounts::update_last_seen(&conn, account.id)?;

    // Log in
    sctx.ctx.client.set_account(account.name.clone())?;

    // Broadcast account-notify
    let _ = broadcast_account_notify(sctx.ctx, Some(&account.name));

    sctx.reply(&format!("You are now identified as {}.", account.name))?;

    tracing::info!(
        nick = ?sctx.ctx.client.nickname()?,
        account = %account.name,
        "User identified via NickServ"
    );

    Ok(())
}

/// Log out of the current account.
fn cmd_logout(sctx: &ServiceContext) -> Result<()> {
    let _ = sctx.require_db()?;

    let account = match sctx.ctx.client.account()? {
        Some(acc) => acc,
        None => {
            sctx.error("You are not logged in.")?;
            return Ok(());
        }
    };

    // Clear account
    sctx.ctx.client.set_sasl_state(None)?;
    // We need to clear the account field - add a method for this
    clear_client_account(sctx)?;

    // Broadcast account-notify (now logged out)
    let _ = broadcast_account_notify(sctx.ctx, None);

    sctx.reply(&format!("You have logged out of account {}.", account))?;

    tracing::info!(
        nick = ?sctx.ctx.client.nickname()?,
        account = %account,
        "User logged out via NickServ"
    );

    Ok(())
}

/// Show info about a nickname.
fn cmd_info(sctx: &ServiceContext, args: &[&str]) -> Result<()> {
    let db = sctx.require_db()?;
    let conn = db.connection()?;

    // Get the nickname to look up (default to current nick)
    let nick = if let Some(n) = args.first() {
        n.to_string()
    } else {
        sctx.nickname()?
    };

    // Check if registered
    let reg_nick = match nicks::find(&conn, &nick)? {
        Some(rn) => rn,
        None => {
            sctx.reply(&format!("Nickname {} is not registered.", nick))?;
            return Ok(());
        }
    };

    // Get account info
    let account = match accounts::find_by_id(&conn, reg_nick.account_id)? {
        Some(acc) => acc,
        None => {
            sctx.error("Internal error: account not found.")?;
            return Ok(());
        }
    };

    // Get all registered nicks for this account
    let all_nicks = nicks::get_for_account(&conn, account.id)?;

    sctx.reply(&format!("***** NickServ Info for {} *****", nick))?;
    sctx.reply(&format!("Account:      {}", account.name))?;
    sctx.reply(&format!(
        "Registered:   {}",
        account.registered_at.format("%Y-%m-%d %H:%M:%S UTC")
    ))?;

    if let Some(last_seen) = account.last_seen {
        sctx.reply(&format!(
            "Last seen:    {}",
            last_seen.format("%Y-%m-%d %H:%M:%S UTC")
        ))?;
    }

    if all_nicks.len() > 1 {
        let nick_list: Vec<_> = all_nicks.iter().map(|n| n.nickname.as_str()).collect();
        sctx.reply(&format!("Nicknames:    {}", nick_list.join(", ")))?;
    }

    // Check if this nick is online
    if let Some(online_client) = sctx.ctx.state.find_client_by_nick(&nick) {
        let logged_in = online_client.account()?.is_some();
        sctx.reply(&format!(
            "Status:       Online{}",
            if logged_in { ", logged in" } else { "" }
        ))?;
    } else {
        sctx.reply("Status:       Offline")?;
    }

    sctx.reply("***** End of Info *****")?;

    Ok(())
}

/// Change account settings.
fn cmd_set(sctx: &ServiceContext, args: &[&str]) -> Result<()> {
    let db = sctx.require_db()?;
    let conn = db.connection()?;
    let account_name = sctx.require_account()?;

    // Get the account
    let account = match accounts::find_by_name(&conn, &account_name)? {
        Some(acc) => acc,
        None => {
            sctx.error("Internal error: account not found.")?;
            return Ok(());
        }
    };

    match args.first().map(|s| s.to_uppercase()).as_deref() {
        Some("PASSWORD") => {
            let new_password = match args.get(1) {
                Some(p) => *p,
                None => {
                    sctx.error("Usage: SET PASSWORD <newpassword>")?;
                    return Ok(());
                }
            };

            let password_hash = hash_password(new_password)?;
            accounts::update_password(&conn, account.id, &password_hash)?;

            sctx.reply("Your password has been changed.")?;

            tracing::info!(account = %account_name, "Password changed via NickServ");
        }
        Some(setting) => {
            sctx.error(&format!(
                "Unknown setting: {}. Use SET PASSWORD <newpass>.",
                setting
            ))?;
        }
        None => {
            sctx.error("Usage: SET PASSWORD <newpassword>")?;
        }
    }

    Ok(())
}

/// Drop (unregister) the current nickname.
fn cmd_drop(sctx: &ServiceContext, args: &[&str]) -> Result<()> {
    let db = sctx.require_db()?;
    let conn = db.connection()?;
    let account_name = sctx.require_account()?;
    let nick = sctx.nickname()?;

    // Get password confirmation
    let password = match args.first() {
        Some(p) => *p,
        None => {
            sctx.error("Usage: DROP <password>")?;
            return Ok(());
        }
    };

    // Get account
    let account = match accounts::find_by_name(&conn, &account_name)? {
        Some(acc) => acc,
        None => {
            sctx.error("Internal error: account not found.")?;
            return Ok(());
        }
    };

    // Verify password
    if !verify_password(password, &account.password_hash) {
        sctx.error("Invalid password.")?;
        return Ok(());
    }

    // Check if this nick belongs to the account
    let _reg_nick = match nicks::find(&conn, &nick)? {
        Some(rn) if rn.account_id == account.id => rn,
        _ => {
            sctx.error(&format!("You do not own the nickname {}.", nick))?;
            return Ok(());
        }
    };

    // Unregister the nick
    nicks::unregister(&conn, &nick)?;

    // If this was the primary nick and the account has no more nicks, delete the account
    let remaining = nicks::get_for_account(&conn, account.id)?;
    if remaining.is_empty() {
        accounts::delete(&conn, account.id)?;
        clear_client_account(sctx)?;
        let _ = broadcast_account_notify(sctx.ctx, None);
        sctx.reply(&format!(
            "Nickname {} has been dropped. Your account has been deleted.",
            nick
        ))?;
    } else {
        sctx.reply(&format!("Nickname {} has been dropped.", nick))?;
    }

    tracing::info!(nick = %nick, account = %account_name, "Nickname dropped via NickServ");

    Ok(())
}

/// Ghost (disconnect) a session using your nick.
fn cmd_ghost(sctx: &ServiceContext, args: &[&str]) -> Result<()> {
    let db = sctx.require_db()?;
    let conn = db.connection()?;

    // Get the nick to ghost
    let target_nick = match args.first() {
        Some(n) => *n,
        None => {
            sctx.error("Usage: GHOST <nick>")?;
            return Ok(());
        }
    };

    // Find the target client
    let target = match sctx.ctx.state.find_client_by_nick(target_nick) {
        Some(c) => c,
        None => {
            sctx.error(&format!("{} is not online.", target_nick))?;
            return Ok(());
        }
    };

    // Check if the caller owns this nick
    let account_name = sctx.require_account()?;
    let owner = nicks::get_owner_account(&conn, target_nick)?;

    // Get account ID
    let account = match accounts::find_by_name(&conn, &account_name)? {
        Some(acc) => acc,
        None => {
            sctx.error("Internal error: account not found.")?;
            return Ok(());
        }
    };

    if owner != Some(account.id) {
        sctx.error(&format!("You do not own the nickname {}.", target_nick))?;
        return Ok(());
    }

    // Don't allow ghosting yourself
    if target.id == sctx.ctx.client.id {
        sctx.error("You cannot ghost yourself.")?;
        return Ok(());
    }

    // Send QUIT to the target
    let quit_msg = irc_proto::Message::with_prefix(
        target.prefix()?,
        irc_proto::Command::Quit {
            message: Some(format!("Ghosted by {}", sctx.nickname()?)),
        },
    );

    // Broadcast to common channels
    let common_members = sctx.ctx.state.get_common_channel_members(target.id)?;
    for member_id in common_members {
        if let Some(member) = sctx.ctx.state.clients.get(&member_id) {
            let _ = member.send(quit_msg.clone());
        }
    }

    // Close the connection by dropping the sender (client will see connection closed)
    // We can't actually close it, but we can remove the client from state
    sctx.ctx.state.remove_client_from_all_channels(target.id)?;
    sctx.ctx.state.remove_client(target.id)?;

    sctx.reply(&format!("{} has been ghosted.", target_nick))?;

    tracing::info!(
        ghost_target = %target_nick,
        by = %sctx.nickname()?,
        "User ghosted via NickServ"
    );

    Ok(())
}

// ============ Helper Functions ============

/// Hash a password using argon2.
fn hash_password(password: &str) -> Result<String> {
    let salt = SaltString::generate(&mut OsRng);
    let argon2 = Argon2::default();
    let password_hash = argon2
        .hash_password(password.as_bytes(), &salt)
        .map_err(|e| crate::error::Error::PasswordHash(e.to_string()))?
        .to_string();
    Ok(password_hash)
}

/// Verify a password against an argon2 hash.
fn verify_password(password: &str, hash: &str) -> bool {
    match PasswordHash::new(hash) {
        Ok(parsed_hash) => Argon2::default()
            .verify_password(password.as_bytes(), &parsed_hash)
            .is_ok(),
        Err(_) => false,
    }
}

/// Clear the client's account (for logout).
fn clear_client_account(sctx: &ServiceContext) -> Result<()> {
    sctx.ctx.client.clear_account()?;
    Ok(())
}
