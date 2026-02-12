//! REGISTER command handler for draft/account-registration.
//!
//! This implements account registration as per the IRCv3
//! draft/account-registration specification.

use argon2::{
    password_hash::{rand_core::OsRng, PasswordHasher, SaltString},
    Argon2,
};
use irc_proto::register_errors::*;
use irc_proto::replies::*;

use super::HandlerContext;
use crate::db::accounts;
use crate::error::{Error, Result};

/// Handle REGISTER command.
///
/// Format: REGISTER <account> <email> <password>
/// - account: Account name to register (* to use current nick)
/// - email: Email address (* for none)
/// - password: Account password
pub fn handle_register(
    ctx: &HandlerContext,
    account: &str,
    email: &str,
    password: &str,
) -> Result<()> {
    // Check if client has the capability enabled
    if !ctx.client.has_cap("draft/account-registration")? {
        ctx.reply(
            irc_proto::errors::ERR_UNKNOWNCOMMAND,
            vec!["REGISTER".into(), "Unknown command".into()],
        )?;
        return Ok(());
    }

    // Check if already authenticated
    if ctx.client.account()?.is_some() {
        ctx.reply(
            ERR_REG_ALREADY_REGISTERED,
            vec!["REGISTER".into(), "You are already registered".into()],
        )?;
        return Ok(());
    }

    // Determine account name
    let account_name = if account == "*" {
        // Use current nickname
        ctx.client.nickname()?.unwrap_or_default()
    } else {
        account.to_string()
    };

    // Validate account name (use nickname validation rules)
    if account_name.is_empty() {
        ctx.reply(
            ERR_REG_NEED_MORE_PARAMS,
            vec!["REGISTER".into(), "No account name provided".into()],
        )?;
        return Ok(());
    }

    if let Err(_e) = irc_proto::validate_nickname(&account_name) {
        ctx.reply(
            ERR_REG_INVALID_ACCOUNT,
            vec![
                account_name.clone(),
                "Invalid account name".into(),
            ],
        )?;
        return Ok(());
    }

    // Validate password
    if password.is_empty() {
        ctx.reply(
            ERR_REG_NEED_MORE_PARAMS,
            vec!["REGISTER".into(), "No password provided".into()],
        )?;
        return Ok(());
    }

    if password.len() < 5 {
        ctx.reply(
            ERR_REG_UNACCEPTABLE_PASSWORD,
            vec![
                "REGISTER".into(),
                "Password too short (minimum 5 characters)".into(),
            ],
        )?;
        return Ok(());
    }

    // Validate email (basic check)
    let email_opt = if email == "*" {
        None
    } else {
        if !email.contains('@') || email.len() < 3 {
            ctx.reply(
                ERR_REG_UNACCEPTABLE_EMAIL,
                vec![email.to_string(), "Invalid email address".into()],
            )?;
            return Ok(());
        }
        Some(email)
    };

    // Hash the password with argon2
    let salt = SaltString::generate(&mut OsRng);
    let argon2 = Argon2::default();
    let password_hash = argon2
        .hash_password(password.as_bytes(), &salt)
        .map_err(|e| Error::PasswordHash(format!("Failed to hash password: {}", e)))?
        .to_string();

    // Create the account in the database
    let db = ctx.state.db.as_ref().ok_or(Error::ServicesUnavailable)?;
    let conn = db.connection()?;
    match accounts::create(&conn, &account_name, &password_hash, email_opt) {
        Ok(_id) => {
            // Success - automatically log them in
            ctx.client.set_account(account_name.clone())?;

            // Send success response
            ctx.reply(
                RPL_REG_SUCCESS,
                vec![
                    account_name.clone(),
                    "Account registered and logged in".into(),
                ],
            )?;

            // Also send SASL success-style logged in message
            let nick = ctx.client.nickname()?.unwrap_or_else(|| "*".to_string());
            let user = ctx.client.username()?.unwrap_or_else(|| "unknown".to_string());
            let host = ctx.client.hostname()?;
            ctx.reply(
                RPL_LOGGEDIN,
                vec![
                    format!("{}!{}@{}", nick, user, host),
                    account_name.clone(),
                    format!("You are now logged in as {}", account_name),
                ],
            )?;

            // Broadcast account notification if capability is in use
            crate::cap::extensions::broadcast_account_notify(ctx, Some(&account_name))?;

            tracing::info!(
                account = %account_name,
                client_id = %ctx.client.id,
                "Account registered successfully"
            );
        }
        Err(Error::AccountExists(_)) => {
            ctx.reply(
                ERR_REG_ACCOUNT_EXISTS,
                vec![
                    account_name,
                    "Account name already exists".into(),
                ],
            )?;
        }
        Err(e) => {
            tracing::error!(error = %e, "Failed to register account");
            return Err(e);
        }
    }

    Ok(())
}
