//! CAP command handler for IRCv3 capability negotiation.

use irc_proto::Command;

use super::HandlerContext;
use crate::error::Result;
use crate::reply::send_welcome_burst;

/// Handle CAP command.
///
/// Subcommands:
/// - LS [302]: List available capabilities
/// - LIST: List client's enabled capabilities
/// - REQ: Request to enable/disable capabilities
/// - END: End capability negotiation
pub fn handle_cap(ctx: &HandlerContext, subcommand: &str, params: &[String]) -> Result<()> {
    match subcommand.to_uppercase().as_str() {
        "LS" => handle_cap_ls(ctx, params),
        "LIST" => handle_cap_list(ctx),
        "REQ" => handle_cap_req(ctx, params),
        "END" => handle_cap_end(ctx),
        _ => {
            // Unknown CAP subcommand - silently ignore per spec
            Ok(())
        }
    }
}

/// Handle CAP LS - list available capabilities.
fn handle_cap_ls(ctx: &HandlerContext, params: &[String]) -> Result<()> {
    // Mark that we're in capability negotiation
    ctx.client.start_cap_negotiation()?;

    // Check for CAP 302 (capability negotiation version)
    let _version = params.first().and_then(|p| p.parse::<u32>().ok());

    // Get available capabilities
    let caps = ctx.state.capabilities.format_ls();

    // Get target (nick or * if not yet registered)
    let target = ctx.client.nickname()?.unwrap_or_else(|| "*".to_string());

    // Send CAP LS response
    // Format: CAP <target> LS :<caps>
    // Using subcommand for target, params[0] for "LS", params[1] for caps
    ctx.send_server_message(Command::Cap {
        subcommand: target,
        params: vec!["LS".to_string(), caps],
    })?;

    Ok(())
}

/// Handle CAP LIST - list client's enabled capabilities.
fn handle_cap_list(ctx: &HandlerContext) -> Result<()> {
    let enabled = ctx.client.format_cap_list()?;
    let target = ctx.client.nickname()?.unwrap_or_else(|| "*".to_string());

    ctx.send_server_message(Command::Cap {
        subcommand: target,
        params: vec!["LIST".to_string(), enabled],
    })?;

    Ok(())
}

/// Handle CAP REQ - request capabilities.
fn handle_cap_req(ctx: &HandlerContext, params: &[String]) -> Result<()> {
    let target = ctx.client.nickname()?.unwrap_or_else(|| "*".to_string());

    // Parse requested capabilities (space-separated, may be prefixed with -)
    let caps_str = params.first().map(|s| s.as_str()).unwrap_or("");
    let requested: Vec<&str> = caps_str.split_whitespace().collect();

    if requested.is_empty() {
        // Nothing to do
        return Ok(());
    }

    // Check if all requested capabilities are valid
    let mut to_enable = Vec::new();
    let mut to_disable = Vec::new();
    let mut all_valid = true;

    for cap in &requested {
        if let Some(cap_name) = cap.strip_prefix('-') {
            // Disable request
            if ctx.state.capabilities.is_available(cap_name) {
                to_disable.push(cap_name.to_string());
            } else {
                all_valid = false;
                break;
            }
        } else {
            // Enable request
            if ctx.state.capabilities.is_available(cap) {
                to_enable.push(cap.to_string());
            } else {
                all_valid = false;
                break;
            }
        }
    }

    if all_valid {
        // Apply changes
        for cap in &to_enable {
            ctx.client.enable_cap(cap)?;
        }
        for cap in &to_disable {
            ctx.client.disable_cap(cap)?;
        }

        // Send ACK with the original request string
        ctx.send_server_message(Command::Cap {
            subcommand: target.clone(),
            params: vec!["ACK".to_string(), caps_str.to_string()],
        })?;
    } else {
        // Send NAK
        ctx.send_server_message(Command::Cap {
            subcommand: target,
            params: vec!["NAK".to_string(), caps_str.to_string()],
        })?;
    }

    Ok(())
}

/// Handle CAP END - end capability negotiation.
fn handle_cap_end(ctx: &HandlerContext) -> Result<()> {
    // Only process if we were actually negotiating
    if !ctx.client.is_cap_negotiating()? {
        return Ok(());
    }

    ctx.client.end_cap_negotiation()?;

    // Check if registration is now complete (NICK and USER already received)
    // We need to check if both nick and user have been set
    let has_nick = ctx.client.nickname()?.is_some();
    let has_user = ctx.client.username()?.is_some();

    if has_nick && has_user {
        tracing::info!(
            client_id = %ctx.client.id,
            nick = ?ctx.client.nickname()?,
            user = ?ctx.client.username()?,
            "Client registered after CAP END"
        );

        // Send welcome burst
        let client = ctx.client.clone();
        let state = ctx.state.clone();
        tokio::spawn(async move {
            send_welcome_burst(&client, &state).await;
        });
    }

    Ok(())
}
