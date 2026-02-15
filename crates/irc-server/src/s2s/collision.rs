//! Nick and channel collision handling.
//!
//! TS6 uses timestamps to resolve collisions:
//! - Older timestamp wins
//! - On equal timestamps, lower UID wins

// Allow dead code during initial development
#![allow(dead_code)]

use std::sync::Arc;

use irc_proto::{S2SCommand, S2SMessage};

use crate::error::Result;
use crate::state::{Client, ServerState};

/// Result of a nick collision check.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CollisionResult {
    /// Local user wins, kill remote.
    LocalWins,
    /// Remote user wins, kill local.
    RemoteWins,
    /// Both users have same TS, compare UIDs.
    /// The lower UID wins.
    CompareUids,
}

/// Handle a nick collision between local and remote users.
///
/// Returns which user should be killed based on timestamps.
pub fn handle_nick_collision(
    local_nick_ts: i64,
    remote_nick_ts: i64,
    local_uid: &str,
    remote_uid: &str,
) -> CollisionResult {
    if remote_nick_ts < local_nick_ts {
        // Remote is older, they win
        CollisionResult::RemoteWins
    } else if remote_nick_ts > local_nick_ts {
        // Local is older, we win
        CollisionResult::LocalWins
    } else {
        // Same timestamp - lower UID wins
        if remote_uid < local_uid {
            CollisionResult::RemoteWins
        } else {
            CollisionResult::LocalWins
        }
    }
}

/// Process a nick collision, taking appropriate action.
///
/// If local loses, the local client is forced to change nick to their UID.
/// If remote loses, we send a KILL for the remote UID.
pub fn process_nick_collision(
    state: &Arc<ServerState>,
    local_client: &Arc<Client>,
    local_uid: &str,
    remote_uid: &str,
    remote_nick_ts: i64,
    our_sid: &str,
) -> Result<Option<S2SMessage>> {
    let local_nick_ts = local_client.connected_at.timestamp();

    match handle_nick_collision(local_nick_ts, remote_nick_ts, local_uid, remote_uid) {
        CollisionResult::LocalWins => {
            // Kill the remote user
            let kill_msg = S2SMessage::with_source(
                our_sid.to_string(),
                S2SCommand::Kill {
                    uid: remote_uid.to_string(),
                    path: state.config.server_name.clone(),
                    reason: "Nick collision (older wins)".to_string(),
                },
            );
            Ok(Some(kill_msg))
        }
        CollisionResult::RemoteWins | CollisionResult::CompareUids => {
            // Force local user to change nick to their UID
            force_nick_change_to_uid(state, local_client, local_uid)?;
            Ok(None)
        }
    }
}

/// Force a local user to change their nick to their UID.
///
/// This is used when they lose a nick collision.
fn force_nick_change_to_uid(
    state: &Arc<ServerState>,
    client: &Arc<Client>,
    uid: &str,
) -> Result<()> {
    use irc_proto::{Command, Message, Prefix};

    // Get current nick
    let old_nick = client.nickname()?.unwrap_or_else(|| "*".to_string());

    // Unregister old nick
    state.unregister_nickname(&old_nick);

    // Set new nick to UID
    client.set_nickname(uid.to_string())?;
    state.register_nickname(uid, client.id);

    // Notify the client
    let nick_msg = Message::with_prefix(
        Prefix::from_server(&state.config.server_name),
        Command::Nick {
            nickname: uid.to_string(),
        },
    );
    let _ = client.send(nick_msg);

    // Send notice explaining the collision
    let notice = Message::with_prefix(
        Prefix::from_server(&state.config.server_name),
        Command::Notice {
            target: uid.to_string(),
            message: format!(
                "Your nickname was changed to {} due to a nick collision",
                uid
            ),
        },
    );
    let _ = client.send(notice);

    tracing::info!(
        old_nick = %old_nick,
        new_nick = %uid,
        "Nick collision: forced local user to UID"
    );

    Ok(())
}

/// Check if a channel mode change should be accepted based on TS.
///
/// Mode changes from a server with a newer TS for the channel should be ignored.
pub fn handle_channel_ts(our_ts: i64, their_ts: i64) -> bool {
    // Accept if their TS is older or equal
    their_ts <= our_ts
}

/// Determine if we should accept channel state from a remote server.
///
/// Returns true if we should accept (their TS is older or equal).
pub fn should_accept_channel_state(our_ts: i64, their_ts: i64) -> ChannelTsAction {
    if their_ts < our_ts {
        // They're older - accept everything and reset our modes
        ChannelTsAction::AcceptAndReset
    } else if their_ts > our_ts {
        // We're older - ignore their modes, but accept membership
        ChannelTsAction::IgnoreModes
    } else {
        // Equal - merge
        ChannelTsAction::Merge
    }
}

/// Action to take based on channel timestamp comparison.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChannelTsAction {
    /// Accept remote state and reset our modes (their TS is older).
    AcceptAndReset,
    /// Ignore their modes but accept membership (our TS is older).
    IgnoreModes,
    /// Merge states normally (equal TS).
    Merge,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_nick_collision_older_wins() {
        // Remote is older (smaller timestamp)
        assert_eq!(
            handle_nick_collision(1000, 900, "00AAAAAAA", "00BAAAAAA"),
            CollisionResult::RemoteWins
        );

        // Local is older
        assert_eq!(
            handle_nick_collision(900, 1000, "00AAAAAAA", "00BAAAAAA"),
            CollisionResult::LocalWins
        );
    }

    #[test]
    fn test_nick_collision_same_ts() {
        // Same timestamp, lower UID wins
        // "00AAAAAAA" < "00BAAAAAA", so if remote has lower UID, remote wins
        assert_eq!(
            handle_nick_collision(1000, 1000, "00BAAAAAA", "00AAAAAAA"),
            CollisionResult::RemoteWins
        );

        assert_eq!(
            handle_nick_collision(1000, 1000, "00AAAAAAA", "00BAAAAAA"),
            CollisionResult::LocalWins
        );
    }

    #[test]
    fn test_channel_ts_action() {
        assert_eq!(
            should_accept_channel_state(1000, 900),
            ChannelTsAction::AcceptAndReset
        );

        assert_eq!(
            should_accept_channel_state(900, 1000),
            ChannelTsAction::IgnoreModes
        );

        assert_eq!(
            should_accept_channel_state(1000, 1000),
            ChannelTsAction::Merge
        );
    }
}
