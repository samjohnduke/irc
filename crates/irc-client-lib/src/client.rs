//! IRC client connection.

/// An IRC client connection.
pub struct Client {
    // TODO: Connection state
}

impl Client {
    /// Create a new disconnected client.
    pub fn new() -> Self {
        Self {}
    }
}

impl Default for Client {
    fn default() -> Self {
        Self::new()
    }
}
