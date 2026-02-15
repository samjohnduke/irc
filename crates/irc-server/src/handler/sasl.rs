//! SASL authentication handler.

use irc_proto::{
    Command,
    errors::{ERR_SASLABORTED, ERR_SASLALREADY, ERR_SASLFAIL, ERR_SASLTOOLONG},
    replies::{RPL_LOGGEDIN, RPL_SASLMECHS, RPL_SASLSUCCESS},
};

use super::HandlerContext;
use crate::cap::sasl::{SaslMechanism, SaslState, decode_plain, supported_mechanisms};
use crate::error::Result;

/// Maximum length for SASL data (400 bytes base64 = ~300 bytes decoded).
const MAX_SASL_DATA_LEN: usize = 400;

/// Handle AUTHENTICATE command.
pub fn handle_authenticate(ctx: &HandlerContext, data: &str) -> Result<()> {
    // Check if SASL capability is enabled
    if !ctx.client.has_cap("sasl")? {
        // SASL not enabled, silently ignore or send error
        ctx.reply(ERR_SASLFAIL, vec!["SASL authentication failed".into()])?;
        return Ok(());
    }

    // Check if already authenticated
    if ctx.client.account()?.is_some() {
        ctx.reply(
            ERR_SASLALREADY,
            vec!["You have already authenticated using SASL".into()],
        )?;
        return Ok(());
    }

    // Handle abort
    if data == "*" {
        ctx.client.set_sasl_state(None)?;
        ctx.reply(ERR_SASLABORTED, vec!["SASL authentication aborted".into()])?;
        return Ok(());
    }

    // Get current SASL state
    let sasl_state = ctx.client.sasl_state()?;

    match sasl_state {
        None | Some(SaslState::WaitingForMechanism) => {
            // Client is selecting a mechanism
            handle_mechanism_selection(ctx, data)
        }
        Some(SaslState::Authenticating {
            mechanism,
            data: accumulated,
        }) => {
            // Client is sending authentication data
            handle_auth_data(ctx, mechanism, accumulated, data)
        }
        Some(SaslState::Complete) => {
            // Already completed, shouldn't happen
            ctx.reply(
                ERR_SASLALREADY,
                vec!["You have already authenticated using SASL".into()],
            )?;
            Ok(())
        }
    }
}

/// Handle mechanism selection (AUTHENTICATE <mechanism>).
fn handle_mechanism_selection(ctx: &HandlerContext, mechanism_name: &str) -> Result<()> {
    match SaslMechanism::parse(mechanism_name) {
        Some(mechanism) => {
            // Valid mechanism - request data
            ctx.client.set_sasl_state(Some(SaslState::Authenticating {
                mechanism,
                data: Vec::new(),
            }))?;

            // Send AUTHENTICATE + to indicate we're ready for data
            ctx.send_server_message(Command::Authenticate {
                data: "+".to_string(),
            })?;
            Ok(())
        }
        None => {
            // Unknown mechanism
            ctx.reply(
                RPL_SASLMECHS,
                vec![
                    supported_mechanisms().to_string(),
                    "are available SASL mechanisms".into(),
                ],
            )?;
            ctx.reply(ERR_SASLFAIL, vec!["SASL authentication failed".into()])?;
            Ok(())
        }
    }
}

/// Handle authentication data.
fn handle_auth_data(
    ctx: &HandlerContext,
    mechanism: SaslMechanism,
    mut accumulated: Vec<u8>,
    data: &str,
) -> Result<()> {
    // Check for data too long
    if data.len() > MAX_SASL_DATA_LEN {
        ctx.client.set_sasl_state(None)?;
        ctx.reply(ERR_SASLTOOLONG, vec!["SASL message too long".into()])?;
        return Ok(());
    }

    // Handle empty response (just "+")
    if data == "+" {
        // Empty payload
        accumulated.clear();
    } else {
        // Accumulate data
        accumulated.extend(data.as_bytes());
    }

    // Check if this is a continuation (400-byte chunks)
    // If the data is exactly 400 bytes, wait for more
    if data.len() == 400 {
        ctx.client.set_sasl_state(Some(SaslState::Authenticating {
            mechanism,
            data: accumulated,
        }))?;
        return Ok(());
    }

    // Complete data received - process authentication
    let auth_data = String::from_utf8_lossy(&accumulated).to_string();

    match mechanism {
        SaslMechanism::Plain => handle_plain_auth(ctx, &auth_data),
    }
}

/// Handle PLAIN mechanism authentication.
fn handle_plain_auth(ctx: &HandlerContext, data: &str) -> Result<()> {
    // Decode PLAIN data
    let credentials = match decode_plain(data) {
        Ok(creds) => creds,
        Err(_) => {
            ctx.client.set_sasl_state(Some(SaslState::Complete))?;
            ctx.reply(ERR_SASLFAIL, vec!["SASL authentication failed".into()])?;
            return Ok(());
        }
    };

    // Use authcid as the account name (or authzid if specified)
    let account_name = if credentials.authzid.is_empty() {
        &credentials.authcid
    } else {
        &credentials.authzid
    };

    // Find the account in configuration
    let account = ctx
        .state
        .config
        .accounts
        .iter()
        .find(|a| a.name.eq_ignore_ascii_case(&credentials.authcid));

    let authenticated = match account {
        Some(acc) => {
            // Verify password using argon2
            verify_password(&credentials.password, &acc.password_hash)
        }
        None => false,
    };

    if authenticated {
        // Set account and mark as complete
        ctx.client.set_account(account_name.to_string())?;
        ctx.client.set_sasl_state(Some(SaslState::Complete))?;

        // Get client info for response
        let nick = ctx.client.nickname()?.unwrap_or_else(|| "*".to_string());
        let hostmask = ctx.client.hostmask()?;

        // Send 900 RPL_LOGGEDIN
        // Format: <nick> <nick>!<user>@<host> <account> :You are now logged in as <account>
        ctx.reply(
            RPL_LOGGEDIN,
            vec![
                hostmask,
                account_name.to_string(),
                format!("You are now logged in as {}", account_name),
            ],
        )?;

        // Send 903 RPL_SASLSUCCESS
        ctx.reply(
            RPL_SASLSUCCESS,
            vec!["SASL authentication successful".into()],
        )?;

        tracing::info!(
            client = %nick,
            account = %account_name,
            "SASL authentication successful"
        );
    } else {
        ctx.client.set_sasl_state(Some(SaslState::Complete))?;
        ctx.reply(ERR_SASLFAIL, vec!["SASL authentication failed".into()])?;

        tracing::debug!(
            authcid = %credentials.authcid,
            "SASL authentication failed - invalid credentials"
        );
    }

    Ok(())
}

/// Verify a password against an argon2 hash.
fn verify_password(password: &str, hash: &str) -> bool {
    use argon2::Argon2;
    use password_hash::{PasswordHash, PasswordVerifier};

    match PasswordHash::new(hash) {
        Ok(parsed_hash) => Argon2::default()
            .verify_password(password.as_bytes(), &parsed_hash)
            .is_ok(),
        Err(_) => false,
    }
}
