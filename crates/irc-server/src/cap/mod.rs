//! IRCv3 capability negotiation.
//!
//! This module implements CAP command handling and capability state management
//! for IRCv3 capability negotiation.

pub mod extensions;
pub mod sasl;

use std::collections::{HashMap, HashSet};

/// Registry of available server capabilities.
#[derive(Debug, Clone)]
pub struct CapabilityRegistry {
    /// Available capabilities.
    available: HashSet<String>,
    /// Capability values (for caps like sasl=PLAIN).
    values: HashMap<String, String>,
}

impl CapabilityRegistry {
    /// Create a new capability registry with default capabilities.
    pub fn new() -> Self {
        let mut registry = Self {
            available: HashSet::new(),
            values: HashMap::new(),
        };

        // Register default capabilities
        registry.register("sasl", Some("PLAIN"));
        registry.register("message-tags", None);
        registry.register("server-time", None);
        registry.register("echo-message", None);

        // Phase 5 capabilities
        registry.register("account-notify", None);
        registry.register("account-tag", None);
        registry.register("extended-join", None);
        registry.register("away-notify", None);
        registry.register("multi-prefix", None);
        registry.register("setname", None);

        // Phase 6 capabilities
        registry.register("draft/chathistory", None);
        registry.register("batch", None);

        // Phase 7 capabilities (IRCv3.2+)
        registry.register("cap-notify", None);
        registry.register("message-ids", None);
        registry.register("userhost-in-names", None);
        registry.register("invite-notify", None);
        registry.register("labeled-response", None);
        registry.register("chghost", None);
        registry.register("bot", None);
        registry.register("draft/account-registration", Some("before-connect"));
        registry.register("draft/read-marker", None);

        registry
    }

    /// Register a capability.
    pub fn register(&mut self, name: &str, value: Option<&str>) {
        self.available.insert(name.to_string());
        if let Some(v) = value {
            self.values.insert(name.to_string(), v.to_string());
        }
    }

    /// Check if a capability is available.
    pub fn is_available(&self, name: &str) -> bool {
        self.available.contains(name)
    }

    /// Get the value for a capability.
    pub fn get_value(&self, name: &str) -> Option<&str> {
        self.values.get(name).map(|s| s.as_str())
    }

    /// Format the capability list for CAP LS response.
    ///
    /// Returns a string like "cap1 cap2=value cap3".
    pub fn format_ls(&self) -> String {
        let mut caps: Vec<String> = self
            .available
            .iter()
            .map(|name| {
                if let Some(value) = self.values.get(name) {
                    format!("{}={}", name, value)
                } else {
                    name.clone()
                }
            })
            .collect();
        caps.sort(); // Consistent ordering for tests
        caps.join(" ")
    }

    /// Get the list of available capabilities.
    pub fn available_caps(&self) -> &HashSet<String> {
        &self.available
    }
}

impl Default for CapabilityRegistry {
    fn default() -> Self {
        Self::new()
    }
}

/// Client capability state.
#[derive(Debug, Clone, Default)]
pub struct ClientCapState {
    /// Enabled capabilities for this client.
    pub enabled: HashSet<String>,
    /// Whether capability negotiation is in progress.
    pub negotiating: bool,
    /// SASL authentication state.
    pub sasl_state: Option<sasl::SaslState>,
    /// Logged-in account name (after successful SASL).
    pub account: Option<String>,
    /// CAP LS version (302 or higher enables cap-notify).
    pub cap_version: Option<u32>,
}

impl ClientCapState {
    /// Create a new client capability state.
    pub fn new() -> Self {
        Self::default()
    }

    /// Check if a capability is enabled.
    pub fn has_cap(&self, name: &str) -> bool {
        self.enabled.contains(name)
    }

    /// Enable a capability.
    pub fn enable(&mut self, name: &str) {
        self.enabled.insert(name.to_string());
    }

    /// Disable a capability.
    pub fn disable(&mut self, name: &str) {
        self.enabled.remove(name);
    }

    /// Start capability negotiation.
    pub fn start_negotiation(&mut self) {
        self.negotiating = true;
    }

    /// End capability negotiation.
    pub fn end_negotiation(&mut self) {
        self.negotiating = false;
    }

    /// Check if negotiation is in progress.
    pub fn is_negotiating(&self) -> bool {
        self.negotiating
    }

    /// Format enabled capabilities for CAP LIST response.
    pub fn format_list(&self) -> String {
        let mut caps: Vec<&str> = self.enabled.iter().map(|s| s.as_str()).collect();
        caps.sort();
        caps.join(" ")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_capability_registry() {
        let registry = CapabilityRegistry::new();

        assert!(registry.is_available("sasl"));
        assert!(registry.is_available("server-time"));
        assert!(registry.is_available("echo-message"));
        assert!(registry.is_available("message-tags"));
        assert!(!registry.is_available("nonexistent"));

        assert_eq!(registry.get_value("sasl"), Some("PLAIN"));
        assert_eq!(registry.get_value("server-time"), None);
    }

    #[test]
    fn test_format_ls() {
        let registry = CapabilityRegistry::new();
        let ls = registry.format_ls();

        assert!(ls.contains("sasl=PLAIN"));
        assert!(ls.contains("server-time"));
        assert!(ls.contains("echo-message"));
        assert!(ls.contains("message-tags"));
    }

    #[test]
    fn test_client_cap_state() {
        let mut state = ClientCapState::new();

        assert!(!state.has_cap("server-time"));
        state.enable("server-time");
        assert!(state.has_cap("server-time"));
        state.disable("server-time");
        assert!(!state.has_cap("server-time"));
    }

    #[test]
    fn test_negotiation_state() {
        let mut state = ClientCapState::new();

        assert!(!state.is_negotiating());
        state.start_negotiation();
        assert!(state.is_negotiating());
        state.end_negotiation();
        assert!(!state.is_negotiating());
    }
}
