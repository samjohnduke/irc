//! S2S link authentication and handshake.
//!
//! Implements the TS6 handshake protocol:
//!
//! ```text
//! Connecting (A)                 Accepting (B)
//!      |-------- PASS --------------->|
//!      |-------- CAPAB -------------->|
//!      |-------- SERVER ------------->|
//!      |<-------- PASS ---------------|
//!      |<-------- CAPAB --------------|
//!      |<-------- SERVER -------------|
//!      |-------- SVINFO ------------->|
//!      |<-------- SVINFO -------------|
//!      |-------- BURST -------------->|
//!      |-------- ENDBURST ----------->|
//!      |<-------- BURST --------------|
//!      |<-------- ENDBURST -----------|
//! ```

use std::net::SocketAddr;
use std::sync::Arc;

use irc_proto::{S2SCommand, S2SMessage};
use tokio::io::{AsyncBufReadExt, AsyncRead, AsyncWrite, AsyncWriteExt, BufReader};
use tokio::sync::mpsc;

use crate::config::{LinkConfig, S2SConfig};
use crate::error::{Error, Result};
use crate::state::ServerState;

use super::state::{LinkState, ServerLink};
use super::{REQUIRED_CAPAB, TS_MIN_VERSION, TS_VERSION};

/// Handle an incoming S2S connection.
///
/// Expects the remote server to send PASS, CAPAB, SERVER first.
pub async fn handle_incoming_link<S>(
    stream: S,
    _addr: SocketAddr,
    state: Arc<ServerState>,
    s2s_config: &S2SConfig,
) -> Result<Arc<ServerLink>>
where
    S: AsyncRead + AsyncWrite + Unpin + Send + 'static,
{
    let (reader, mut writer) = tokio::io::split(stream);
    let mut reader = BufReader::new(reader);
    let mut line = String::new();

    // Create channel for sending messages to this link
    let (tx, _rx) = mpsc::channel::<S2SMessage>(512);

    // Receive PASS
    line.clear();
    reader.read_line(&mut line).await?;
    let pass_msg = S2SMessage::parse(line.trim())?;
    let (password, remote_ts_version, remote_sid) = match pass_msg.command {
        S2SCommand::Pass { password, ts_version, sid } => (password, ts_version, sid),
        _ => return Err(Error::Protocol("Expected PASS command".into())),
    };

    // Validate TS version
    if remote_ts_version < TS_MIN_VERSION {
        return Err(Error::Protocol(format!(
            "TS version {} too old, minimum is {}",
            remote_ts_version, TS_MIN_VERSION
        )));
    }

    // Validate SID
    if !irc_proto::validate_sid(&remote_sid) {
        return Err(Error::Protocol(format!("Invalid SID: {}", remote_sid)));
    }

    // Find the link config for this connection
    let link_config = s2s_config
        .links
        .iter()
        .find(|l| l.accept_incoming && l.receive_password == password)
        .ok_or_else(|| Error::Protocol("Invalid link password".into()))?;

    // Receive CAPAB
    line.clear();
    reader.read_line(&mut line).await?;
    let capab_msg = S2SMessage::parse(line.trim())?;
    let remote_caps = match capab_msg.command {
        S2SCommand::Capab { capabilities } => capabilities,
        _ => return Err(Error::Protocol("Expected CAPAB command".into())),
    };

    // Check required capabilities
    for cap in REQUIRED_CAPAB {
        if !remote_caps.iter().any(|c| c.eq_ignore_ascii_case(cap)) {
            return Err(Error::Protocol(format!("Missing required capability: {}", cap)));
        }
    }

    // Receive SERVER
    line.clear();
    reader.read_line(&mut line).await?;
    let server_msg = S2SMessage::parse(line.trim())?;
    let (remote_name, _remote_hopcount, _remote_desc) = match server_msg.command {
        S2SCommand::Server { name, hopcount, description } => (name, hopcount, description),
        _ => return Err(Error::Protocol("Expected SERVER command".into())),
    };

    // Validate server name matches config
    if remote_name != link_config.name {
        return Err(Error::Protocol(format!(
            "Server name mismatch: expected {}, got {}",
            link_config.name, remote_name
        )));
    }

    // Create the server link
    let link = Arc::new(ServerLink::new(remote_sid.clone(), remote_name.clone(), tx.clone()));
    link.set_state(LinkState::Authenticating)?;

    // Send our PASS, CAPAB, SERVER
    let our_pass = S2SMessage::new(S2SCommand::Pass {
        password: link_config.send_password.clone(),
        ts_version: TS_VERSION,
        sid: s2s_config.sid.clone(),
    });
    writer.write_all(&our_pass.to_bytes()).await?;

    let our_capab = S2SMessage::new(S2SCommand::Capab {
        capabilities: REQUIRED_CAPAB.iter().map(|s| s.to_string()).collect(),
    });
    writer.write_all(&our_capab.to_bytes()).await?;

    let our_server = S2SMessage::new(S2SCommand::Server {
        name: state.config.server_name.clone(),
        hopcount: 1,
        description: format!("{} IRC Server", state.config.network_name),
    });
    writer.write_all(&our_server.to_bytes()).await?;

    // Receive SVINFO
    line.clear();
    reader.read_line(&mut line).await?;
    let svinfo_msg = S2SMessage::parse(line.trim())?;
    match svinfo_msg.command {
        S2SCommand::SvInfo { ts_version, ts_min: _, current_time } => {
            if ts_version < TS_MIN_VERSION {
                return Err(Error::Protocol("TS version too old".into()));
            }
            // Could check time skew here
            tracing::debug!(
                remote_ts = ts_version,
                remote_time = current_time,
                "Received SVINFO"
            );
        }
        _ => return Err(Error::Protocol("Expected SVINFO command".into())),
    }

    // Send our SVINFO
    let now = chrono::Utc::now().timestamp();
    let our_svinfo = S2SMessage::new(S2SCommand::SvInfo {
        ts_version: TS_VERSION,
        ts_min: TS_MIN_VERSION,
        current_time: now,
    });
    writer.write_all(&our_svinfo.to_bytes()).await?;

    link.set_state(LinkState::Authenticated)?;

    tracing::info!(
        sid = %remote_sid,
        name = %remote_name,
        "S2S link authenticated"
    );

    Ok(link)
}

/// Initiate an outgoing S2S connection.
pub async fn initiate_outgoing_link(
    link_config: &LinkConfig,
    state: Arc<ServerState>,
    s2s_config: &S2SConfig,
) -> Result<Arc<ServerLink>> {
    use tokio::net::TcpStream;

    let addr = format!("{}:{}", link_config.address, link_config.port);
    tracing::info!(addr = %addr, name = %link_config.name, "Connecting to server");

    let stream = TcpStream::connect(&addr).await?;
    let (reader, mut writer) = tokio::io::split(stream);
    let mut reader = BufReader::new(reader);
    let mut line = String::new();

    // Create channel for sending messages to this link
    let (tx, _rx) = mpsc::channel::<S2SMessage>(512);

    // Send PASS, CAPAB, SERVER
    let our_pass = S2SMessage::new(S2SCommand::Pass {
        password: link_config.send_password.clone(),
        ts_version: TS_VERSION,
        sid: s2s_config.sid.clone(),
    });
    writer.write_all(&our_pass.to_bytes()).await?;

    let our_capab = S2SMessage::new(S2SCommand::Capab {
        capabilities: REQUIRED_CAPAB.iter().map(|s| s.to_string()).collect(),
    });
    writer.write_all(&our_capab.to_bytes()).await?;

    let our_server = S2SMessage::new(S2SCommand::Server {
        name: state.config.server_name.clone(),
        hopcount: 1,
        description: format!("{} IRC Server", state.config.network_name),
    });
    writer.write_all(&our_server.to_bytes()).await?;

    // Receive PASS
    line.clear();
    reader.read_line(&mut line).await?;
    let pass_msg = S2SMessage::parse(line.trim())?;
    let (password, remote_ts_version, remote_sid) = match pass_msg.command {
        S2SCommand::Pass { password, ts_version, sid } => (password, ts_version, sid),
        _ => return Err(Error::Protocol("Expected PASS command".into())),
    };

    // Validate password
    if password != link_config.receive_password {
        return Err(Error::Protocol("Invalid link password".into()));
    }

    // Validate TS version
    if remote_ts_version < TS_MIN_VERSION {
        return Err(Error::Protocol("TS version too old".into()));
    }

    // Receive CAPAB
    line.clear();
    reader.read_line(&mut line).await?;
    let capab_msg = S2SMessage::parse(line.trim())?;
    let remote_caps = match capab_msg.command {
        S2SCommand::Capab { capabilities } => capabilities,
        _ => return Err(Error::Protocol("Expected CAPAB command".into())),
    };

    // Check required capabilities
    for cap in REQUIRED_CAPAB {
        if !remote_caps.iter().any(|c| c.eq_ignore_ascii_case(cap)) {
            return Err(Error::Protocol(format!("Missing required capability: {}", cap)));
        }
    }

    // Receive SERVER
    line.clear();
    reader.read_line(&mut line).await?;
    let server_msg = S2SMessage::parse(line.trim())?;
    let (remote_name, _, _remote_desc) = match server_msg.command {
        S2SCommand::Server { name, hopcount, description } => (name, hopcount, description),
        _ => return Err(Error::Protocol("Expected SERVER command".into())),
    };

    // Create the server link
    let link = Arc::new(ServerLink::new(remote_sid.clone(), remote_name.clone(), tx.clone()));
    link.set_state(LinkState::Authenticating)?;

    // Send SVINFO
    let now = chrono::Utc::now().timestamp();
    let our_svinfo = S2SMessage::new(S2SCommand::SvInfo {
        ts_version: TS_VERSION,
        ts_min: TS_MIN_VERSION,
        current_time: now,
    });
    writer.write_all(&our_svinfo.to_bytes()).await?;

    // Receive SVINFO
    line.clear();
    reader.read_line(&mut line).await?;
    let svinfo_msg = S2SMessage::parse(line.trim())?;
    match svinfo_msg.command {
        S2SCommand::SvInfo { ts_version, .. } => {
            if ts_version < TS_MIN_VERSION {
                return Err(Error::Protocol("TS version too old".into()));
            }
        }
        _ => return Err(Error::Protocol("Expected SVINFO command".into())),
    }

    link.set_state(LinkState::Authenticated)?;

    tracing::info!(
        sid = %remote_sid,
        name = %remote_name,
        "S2S link established"
    );

    Ok(link)
}
