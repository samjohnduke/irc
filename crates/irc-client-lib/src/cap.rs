//! IRCv3 capability negotiation.
//!
//! This module tracks capabilities requested from and enabled by the server.

use std::collections::{HashMap, HashSet};

/// Tracks capability negotiation state.
#[derive(Debug, Clone)]
pub struct CapabilityState {
    /// Capabilities we want to request.
    requested: HashSet<String>,

    /// Capabilities available on the server (from CAP LS).
    available: HashMap<String, Option<String>>,

    /// Capabilities we have successfully enabled.
    enabled: HashSet<String>,

    /// Whether CAP negotiation is complete.
    negotiation_complete: bool,

    /// CAP version (302 = IRCv3.2).
    #[allow(dead_code)]
    version: u16,
}

impl Default for CapabilityState {
    fn default() -> Self {
        Self::new()
    }
}

impl CapabilityState {
    /// Create a new capability state.
    pub fn new() -> Self {
        Self {
            requested: HashSet::new(),
            available: HashMap::new(),
            enabled: HashSet::new(),
            negotiation_complete: false,
            version: 302,
        }
    }

    /// Set the capabilities we want to request.
    pub fn set_requested(&mut self, caps: impl IntoIterator<Item = String>) {
        self.requested = caps.into_iter().collect();
    }

    /// Parse CAP LS response and store available capabilities.
    ///
    /// Format: `cap1 cap2=value cap3 ...`
    pub fn parse_ls(&mut self, caps_str: &str) {
        for cap in caps_str.split_whitespace() {
            if let Some((name, value)) = cap.split_once('=') {
                self.available
                    .insert(name.to_string(), Some(value.to_string()));
            } else {
                self.available.insert(cap.to_string(), None);
            }
        }
    }

    /// Get capabilities to request (intersection of requested and available).
    pub fn caps_to_request(&self) -> Vec<String> {
        self.requested
            .iter()
            .filter(|cap| {
                // Handle sasl specially - we want it if available
                if *cap == "sasl" {
                    return self.available.contains_key("sasl");
                }
                self.available.contains_key(*cap)
            })
            .cloned()
            .collect()
    }

    /// Parse CAP ACK response and mark capabilities as enabled.
    pub fn parse_ack(&mut self, caps_str: &str) {
        for cap in caps_str.split_whitespace() {
            // Handle capability modifiers (-, ~, =)
            let cap = cap.trim_start_matches(['-', '~', '='].as_ref());
            self.enabled.insert(cap.to_string());
        }
    }

    /// Parse CAP NAK response (capabilities denied).
    pub fn parse_nak(&mut self, caps_str: &str) {
        // NAK means these caps weren't enabled - just log for now
        tracing::debug!("CAP NAK: {}", caps_str);
    }

    /// Mark negotiation as complete.
    pub fn complete_negotiation(&mut self) {
        self.negotiation_complete = true;
    }

    /// Check if negotiation is complete.
    pub fn is_negotiation_complete(&self) -> bool {
        self.negotiation_complete
    }

    /// Check if a capability is enabled.
    pub fn is_enabled(&self, cap: &str) -> bool {
        self.enabled.contains(cap)
    }

    /// Check if a capability is available on the server.
    pub fn is_available(&self, cap: &str) -> bool {
        self.available.contains_key(cap)
    }

    /// Get the value of an available capability (e.g., sasl=PLAIN).
    pub fn get_value(&self, cap: &str) -> Option<&str> {
        self.available.get(cap).and_then(|v| v.as_deref())
    }

    /// Get all enabled capabilities.
    pub fn enabled_caps(&self) -> impl Iterator<Item = &str> {
        self.enabled.iter().map(String::as_str)
    }

    /// Get all available capabilities.
    pub fn available_caps(&self) -> impl Iterator<Item = &str> {
        self.available.keys().map(String::as_str)
    }

    /// Check if SASL is available.
    pub fn sasl_available(&self) -> bool {
        self.available.contains_key("sasl")
    }

    /// Get supported SASL mechanisms.
    pub fn sasl_mechanisms(&self) -> Option<Vec<&str>> {
        self.available
            .get("sasl")
            .and_then(|v| v.as_ref().map(|s| s.split(',').collect()))
    }

    /// Check if SASL PLAIN is supported.
    pub fn sasl_plain_available(&self) -> bool {
        match self.sasl_mechanisms() {
            Some(mechs) => mechs.iter().any(|m| m.eq_ignore_ascii_case("PLAIN")),
            None => self.sasl_available(), // No value means any mechanism
        }
    }

    /// Check if chathistory is enabled.
    pub fn chathistory_enabled(&self) -> bool {
        self.enabled.contains("draft/chathistory")
    }

    /// Check if server-time is enabled.
    pub fn server_time_enabled(&self) -> bool {
        self.enabled.contains("server-time")
    }

    /// Check if echo-message is enabled.
    pub fn echo_message_enabled(&self) -> bool {
        self.enabled.contains("echo-message")
    }

    /// Check if batch is enabled.
    pub fn batch_enabled(&self) -> bool {
        self.enabled.contains("batch")
    }

    /// Handle CAP NEW (dynamic capability added).
    pub fn handle_new(&mut self, caps_str: &str) {
        self.parse_ls(caps_str);
    }

    /// Handle CAP DEL (dynamic capability removed).
    pub fn handle_del(&mut self, caps_str: &str) {
        for cap in caps_str.split_whitespace() {
            self.available.remove(cap);
            self.enabled.remove(cap);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_ls() {
        let mut state = CapabilityState::new();
        state.parse_ls("server-time sasl=PLAIN,EXTERNAL batch message-tags");

        assert!(state.is_available("server-time"));
        assert!(state.is_available("sasl"));
        assert!(state.is_available("batch"));
        assert!(!state.is_available("unknown"));

        assert_eq!(state.get_value("sasl"), Some("PLAIN,EXTERNAL"));
        assert_eq!(state.get_value("server-time"), None);
    }

    #[test]
    fn test_caps_to_request() {
        let mut state = CapabilityState::new();
        state.set_requested(vec![
            "server-time".into(),
            "batch".into(),
            "not-available".into(),
        ]);
        state.parse_ls("server-time batch echo-message");

        let to_req = state.caps_to_request();
        assert!(to_req.contains(&"server-time".to_string()));
        assert!(to_req.contains(&"batch".to_string()));
        assert!(!to_req.contains(&"not-available".to_string()));
    }

    #[test]
    fn test_parse_ack() {
        let mut state = CapabilityState::new();
        state.parse_ack("server-time batch");

        assert!(state.is_enabled("server-time"));
        assert!(state.is_enabled("batch"));
        assert!(!state.is_enabled("sasl"));
    }

    #[test]
    fn test_sasl_mechanisms() {
        let mut state = CapabilityState::new();
        state.parse_ls("sasl=PLAIN,EXTERNAL");

        let mechs = state.sasl_mechanisms().unwrap();
        assert!(mechs.contains(&"PLAIN"));
        assert!(mechs.contains(&"EXTERNAL"));
        assert!(state.sasl_plain_available());
    }
}
