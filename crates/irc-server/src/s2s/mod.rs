//! Server-to-Server (S2S) communication module.
//!
//! This module implements the TS6 protocol for server linking, providing:
//! - Server authentication and handshake
//! - State synchronization (BURST)
//! - Message routing across the server network
//! - Nick and channel collision handling
//!
//! # Protocol Overview
//!
//! TS6 is a modern server-to-server protocol used by IRC networks like
//! Charybdis and Atheme. Key features:
//!
//! - **SIDs**: Each server has a unique 3-character ID (e.g., "00A")
//! - **UIDs**: Each user has a unique ID = SID + 6 chars (e.g., "00AAAAAAA")
//! - **Timestamps**: Used for collision resolution (older wins)
//! - **BURST**: State synchronization on link establishment

pub mod state;
mod handshake;
mod burst;
mod routing;
mod collision;
mod handler;

pub use state::{ServerLink, LinkState};
pub use handshake::{handle_incoming_link, initiate_outgoing_link};
pub use routing::{propagate, route_to, find_route};
pub use collision::{handle_nick_collision, handle_channel_ts};

/// Required capabilities for TS6 links.
pub const REQUIRED_CAPAB: &[&str] = &["QS", "ENCAP", "EX", "IE", "EUID", "TB"];

/// TS6 protocol version.
pub const TS_VERSION: u8 = 6;

/// Minimum TS version we accept.
pub const TS_MIN_VERSION: u8 = 6;

/// Generate a UID from SID and counter.
pub fn generate_uid(sid: &str, counter: u64) -> String {
    // UID is SID (3 chars) + 6 alphanumeric chars
    // We use base36 encoding for the counter to fit in 6 chars
    let mut uid = String::with_capacity(9);
    uid.push_str(sid);

    // Convert counter to base36, padded to 6 chars
    let base36 = format_base36(counter, 6);
    uid.push_str(&base36);

    uid
}

/// Format a number as base36, padded to the given length.
fn format_base36(mut n: u64, len: usize) -> String {
    const CHARS: &[u8] = b"0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZ";
    let mut result = vec![b'A'; len];

    for i in (0..len).rev() {
        result[i] = CHARS[(n % 36) as usize];
        n /= 36;
    }

    String::from_utf8(result).unwrap()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_uid() {
        assert_eq!(generate_uid("00A", 0), "00A000000");
        assert_eq!(generate_uid("00A", 1), "00A000001");
        assert_eq!(generate_uid("00A", 35), "00A00000Z");
        assert_eq!(generate_uid("00A", 36), "00A000010");
        assert_eq!(generate_uid("ABC", 1234567), "ABC00QGLJ");
    }

    #[test]
    fn test_format_base36() {
        assert_eq!(format_base36(0, 6), "000000");
        assert_eq!(format_base36(35, 6), "00000Z");
        assert_eq!(format_base36(36, 6), "000010");
        assert_eq!(format_base36(36 * 36, 6), "000100");
    }
}
