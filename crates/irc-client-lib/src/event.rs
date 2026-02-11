//! Client events.

/// Events emitted by the client.
#[derive(Debug, Clone)]
pub enum Event {
    /// Connected to server
    Connected,

    /// Disconnected from server
    Disconnected {
        reason: Option<String>,
    },

    /// Received a message
    Message {
        target: String,
        sender: String,
        text: String,
    },
}
