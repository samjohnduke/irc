//! S2S message routing.
//!
//! Handles routing messages across the server network:
//! - `propagate`: Broadcast to all servers (except source)
//! - `route_to`: Route to a specific target (by UID or SID)
//! - `find_route`: Find which link to use for a target

// Allow dead code during initial development
#![allow(dead_code)]

use std::sync::Arc;

use dashmap::DashMap;
use irc_proto::S2SMessage;

use crate::error::Result;
use super::state::ServerLink;

/// Propagate a message to all linked servers except the source.
///
/// Used for broadcasts like QUIT, NICK changes, KILL, SQUIT.
pub fn propagate(
    servers: &DashMap<String, Arc<ServerLink>>,
    msg: S2SMessage,
    exclude_sid: Option<&str>,
) -> Result<()> {
    for entry in servers.iter() {
        let link = entry.value();

        // Skip the source server
        if exclude_sid.is_some_and(|sid| link.sid == sid) {
            continue;
        }

        // Only send to ready links
        if !link.is_ready()? {
            continue;
        }

        // Only send to directly connected servers
        if !link.is_direct() {
            continue;
        }

        if let Err(e) = link.send(msg.clone()) {
            tracing::warn!(
                sid = %link.sid,
                error = %e,
                "Failed to propagate message"
            );
        }
    }

    Ok(())
}

/// Route a message to a specific target (UID or SID).
pub fn route_to(
    servers: &DashMap<String, Arc<ServerLink>>,
    target: &str,
    msg: S2SMessage,
) -> Result<bool> {
    // Extract SID from target (first 3 chars of UID, or the SID itself)
    let target_sid = if target.len() >= 3 {
        &target[..3]
    } else {
        target
    };

    // Find the route to this SID
    if let Some(link) = find_route(servers, target_sid)? {
        link.send(msg)?;
        Ok(true)
    } else {
        tracing::debug!(target = %target, "No route found");
        Ok(false)
    }
}

/// Find the link to use for routing to a target SID.
///
/// In a mesh network, this finds the uplink that can reach the target.
pub fn find_route(
    servers: &DashMap<String, Arc<ServerLink>>,
    target_sid: &str,
) -> Result<Option<Arc<ServerLink>>> {
    // First, check if we have a direct link to this SID
    if let Some(entry) = servers.get(target_sid) {
        let link = Arc::clone(entry.value());
        if link.is_direct() && link.is_ready()? {
            return Ok(Some(link));
        }
    }

    // Otherwise, find the server that has this as a child
    for entry in servers.iter() {
        let link = entry.value();

        if !link.is_direct() || !link.is_ready()? {
            continue;
        }

        // Check if this link's children include the target
        let children = link.child_sids.read().map_err(|_| {
            crate::error::Error::LockPoisoned("child_sids".to_string())
        })?;

        if children.contains(target_sid) {
            return Ok(Some(Arc::clone(link)));
        }
    }

    // Check if the target is routed through any server
    for entry in servers.iter() {
        let link = entry.value();

        if !link.is_ready()? {
            continue;
        }

        // The link itself might route to the target
        if link.uplink_sid == target_sid
            && let Some(uplink) = servers.get(&link.uplink_sid)
            && uplink.is_direct()
        {
            return Ok(Some(Arc::clone(uplink.value())));
        }
    }

    Ok(None)
}

/// Route a message to all servers that have members in a channel.
///
/// Used for channel-targeted messages like JOIN, PART, KICK, MODE.
pub fn route_to_channel_servers(
    servers: &DashMap<String, Arc<ServerLink>>,
    channel_member_sids: &[String],
    msg: S2SMessage,
    exclude_sid: Option<&str>,
) -> Result<()> {
    for sid in channel_member_sids {
        if exclude_sid.is_some_and(|exclude| sid == exclude) {
            continue;
        }

        if let Some(link) = find_route(servers, sid)?
            && let Err(e) = link.send(msg.clone())
        {
            tracing::warn!(
                sid = %sid,
                error = %e,
                "Failed to route channel message"
            );
        }
    }

    Ok(())
}

/// Determine which servers need to receive a PRIVMSG/NOTICE.
///
/// For channels: servers with members in the channel
/// For users: the server hosting the user (by UID)
pub fn get_message_targets(
    _servers: &DashMap<String, Arc<ServerLink>>,
    target: &str,
    channel_member_sids: Option<&[String]>,
) -> Vec<String> {
    if target.starts_with('#') || target.starts_with('&') {
        // Channel target - return SIDs of servers with channel members
        channel_member_sids
            .map(|sids| sids.to_vec())
            .unwrap_or_default()
    } else {
        // User target - return SID from UID
        if target.len() >= 3 {
            vec![target[..3].to_string()]
        } else {
            Vec::new()
        }
    }
}
