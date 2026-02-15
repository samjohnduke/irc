//! IRC registration state machine.
//!
//! Handles the connection registration flow:
//! 1. CAP LS 302 (capability negotiation)
//! 2. SASL authentication (if configured)
//! 3. CAP END
//! 4. NICK/USER
//! 5. Wait for 001 (RPL_WELCOME)

use irc_proto::{Command, Message};

use crate::cap::CapabilityState;
use crate::config::ClientConfig;
use crate::error::{RegistrationError, SaslError};

/// Registration state machine.
#[derive(Debug)]
pub struct RegistrationState {
    /// Current phase of registration.
    phase: RegistrationPhase,

    /// Nicknames to try (index into config.nicknames).
    nick_index: usize,

    /// SASL state if authenticating.
    sasl_state: SaslState,

    /// Capability state.
    caps: CapabilityState,

    /// Whether we've received welcome.
    welcomed: bool,

    /// Server name from welcome message.
    server_name: Option<String>,

    /// Welcome message.
    welcome_message: Option<String>,
}

/// Registration phase.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RegistrationPhase {
    /// Initial state, need to send CAP LS.
    Start,
    /// Waiting for CAP LS response.
    WaitingCapLs,
    /// Waiting for CAP REQ acknowledgment.
    WaitingCapAck,
    /// SASL authentication in progress.
    SaslAuth,
    /// Waiting for registration to complete (001).
    WaitingWelcome,
    /// Registration complete.
    Complete,
    /// Registration failed.
    Failed,
}

/// SASL authentication state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SaslState {
    /// Not using SASL.
    None,
    /// Waiting to start SASL.
    Pending,
    /// Sent AUTHENTICATE <mechanism>.
    SentMechanism,
    /// Waiting for server challenge.
    WaitingChallenge,
    /// Sent credentials.
    SentCredentials,
    /// SASL complete (success or failure).
    Complete,
}

/// Messages to send as part of registration.
#[derive(Debug)]
pub enum RegistrationAction {
    /// Send these messages.
    Send(Vec<Message>),
    /// Registration complete.
    Complete {
        nick: String,
        server: String,
        welcome: String,
    },
    /// Registration failed.
    Failed(RegistrationError),
    /// Continue waiting.
    Continue,
}

impl Default for RegistrationState {
    fn default() -> Self {
        Self::new()
    }
}

impl RegistrationState {
    /// Create new registration state.
    pub fn new() -> Self {
        Self {
            phase: RegistrationPhase::Start,
            nick_index: 0,
            sasl_state: SaslState::None,
            caps: CapabilityState::new(),
            welcomed: false,
            server_name: None,
            welcome_message: None,
        }
    }

    /// Get the current phase.
    pub fn phase(&self) -> RegistrationPhase {
        self.phase
    }

    /// Check if registration is complete.
    pub fn is_complete(&self) -> bool {
        self.phase == RegistrationPhase::Complete
    }

    /// Check if registration failed.
    pub fn is_failed(&self) -> bool {
        self.phase == RegistrationPhase::Failed
    }

    /// Get capability state.
    pub fn caps(&self) -> &CapabilityState {
        &self.caps
    }

    /// Start registration - returns initial messages to send.
    pub fn start(&mut self, config: &ClientConfig) -> Vec<Message> {
        self.caps.set_requested(config.capabilities.iter().cloned());

        // If SASL is configured, mark as pending
        if config.sasl.is_some() {
            self.sasl_state = SaslState::Pending;
        }

        self.phase = RegistrationPhase::WaitingCapLs;

        let mut messages = Vec::new();

        // Send CAP LS 302 to start capability negotiation
        messages.push(Message::new(Command::Cap {
            subcommand: "LS".into(),
            params: vec!["302".into()],
        }));

        messages
    }

    /// Process an incoming message during registration.
    pub fn process(&mut self, msg: &Message, config: &ClientConfig) -> RegistrationAction {
        match &msg.command {
            Command::Cap { subcommand, params } => {
                // Server CAP responses have format: CAP <target> <subcommand> [params]
                // The target is typically "*" during pre-registration
                // Our parser puts target in subcommand and real subcommand in params[0]
                let (actual_subcommand, actual_params) =
                    if subcommand == "*" || subcommand.contains('.') {
                        // This is a target, real subcommand is in params
                        if let Some((first, rest)) = params.split_first() {
                            (first.as_str(), rest.to_vec())
                        } else {
                            (subcommand.as_str(), params.clone())
                        }
                    } else {
                        (subcommand.as_str(), params.clone())
                    };
                self.handle_cap(actual_subcommand, &actual_params, config)
            }

            Command::Authenticate { data } => self.handle_authenticate(data, config),

            Command::Numeric { code, params, .. } => self.handle_numeric(*code, params, config),

            Command::Ping { server1, .. } => {
                // Respond to ping during registration
                RegistrationAction::Send(vec![Message::new(Command::Pong {
                    server1: server1.clone(),
                    server2: None,
                })])
            }

            _ => RegistrationAction::Continue,
        }
    }

    /// Handle CAP response.
    fn handle_cap(
        &mut self,
        subcommand: &str,
        params: &Vec<String>,
        config: &ClientConfig,
    ) -> RegistrationAction {
        tracing::debug!("handle_cap: subcommand={}, params={:?}", subcommand, params);
        match subcommand.to_uppercase().as_str() {
            "LS" => {
                // Parse available caps
                if let Some(caps_str) = params.last() {
                    tracing::debug!("CAP LS caps: {}", caps_str);
                    self.caps.parse_ls(caps_str);
                }

                // Check for multi-line (starts with *)
                if params.first().map(|s| s == "*").unwrap_or(false) {
                    tracing::debug!("Multi-line CAP LS, waiting for more");
                    // More caps coming
                    return RegistrationAction::Continue;
                }

                // All caps received, request what we want
                let mut to_request = self.caps.caps_to_request();
                tracing::debug!("Caps to request: {:?}", to_request);

                // Add SASL if configured and available
                if config.sasl.is_some() && self.caps.sasl_available() {
                    if !to_request.contains(&"sasl".to_string()) {
                        to_request.push("sasl".into());
                    }
                }

                if to_request.is_empty() {
                    // No caps to request, proceed with registration
                    return self.finish_cap_and_register(config);
                }

                self.phase = RegistrationPhase::WaitingCapAck;
                RegistrationAction::Send(vec![Message::new(Command::Cap {
                    subcommand: "REQ".into(),
                    params: vec![to_request.join(" ")],
                })])
            }

            "ACK" => {
                // Parse acknowledged caps
                if let Some(caps_str) = params.last() {
                    tracing::debug!("CAP ACK: {}", caps_str);
                    self.caps.parse_ack(caps_str);
                }

                // Start SASL if configured and cap is enabled
                if self.sasl_state == SaslState::Pending && self.caps.is_enabled("sasl") {
                    tracing::debug!("Starting SASL authentication");
                    return self.start_sasl(config);
                }

                // Otherwise finish cap negotiation
                tracing::debug!("CAP negotiation complete, finishing registration");
                self.finish_cap_and_register(config)
            }

            "NAK" => {
                // Caps rejected, continue anyway
                if let Some(caps_str) = params.last() {
                    tracing::warn!("CAP NAK: {}", caps_str);
                    self.caps.parse_nak(caps_str);
                }

                // If SASL was rejected but required, fail
                if self.sasl_state == SaslState::Pending && config.sasl.is_some() {
                    // SASL cap was rejected - try to continue without it
                    tracing::warn!("SASL capability rejected by server");
                    self.sasl_state = SaslState::Complete;
                }

                self.finish_cap_and_register(config)
            }

            _ => RegistrationAction::Continue,
        }
    }

    /// Start SASL authentication.
    fn start_sasl(&mut self, config: &ClientConfig) -> RegistrationAction {
        let sasl = match &config.sasl {
            Some(s) => s,
            None => return self.finish_cap_and_register(config),
        };

        // Check if our mechanism is supported
        if !self.caps.sasl_plain_available() {
            return RegistrationAction::Failed(RegistrationError::SaslFailed(
                SaslError::MechanismNotSupported("PLAIN".into()),
            ));
        }

        self.phase = RegistrationPhase::SaslAuth;
        self.sasl_state = SaslState::SentMechanism;

        RegistrationAction::Send(vec![Message::new(Command::Authenticate {
            data: sasl.mechanism.as_str().into(),
        })])
    }

    /// Handle AUTHENTICATE response.
    fn handle_authenticate(&mut self, data: &str, config: &ClientConfig) -> RegistrationAction {
        if self.phase != RegistrationPhase::SaslAuth {
            return RegistrationAction::Continue;
        }

        match self.sasl_state {
            SaslState::SentMechanism => {
                if data == "+" {
                    // Server ready for credentials
                    return self.send_sasl_credentials(config);
                }
                RegistrationAction::Continue
            }

            _ => RegistrationAction::Continue,
        }
    }

    /// Send SASL PLAIN credentials.
    fn send_sasl_credentials(&mut self, config: &ClientConfig) -> RegistrationAction {
        let sasl = match &config.sasl {
            Some(s) => s,
            None => return RegistrationAction::Continue,
        };

        let credentials = encode_sasl_plain(&sasl.username, &sasl.password);
        self.sasl_state = SaslState::SentCredentials;

        RegistrationAction::Send(vec![Message::new(Command::Authenticate {
            data: credentials,
        })])
    }

    /// Handle numeric replies during registration.
    fn handle_numeric(
        &mut self,
        code: u16,
        params: &[String],
        config: &ClientConfig,
    ) -> RegistrationAction {
        match code {
            // SASL numerics
            900 => {
                // RPL_LOGGEDIN - SASL successful
                tracing::info!("SASL authentication successful");
                self.sasl_state = SaslState::Complete;
                RegistrationAction::Continue
            }

            903 => {
                // RPL_SASLSUCCESS
                self.sasl_state = SaslState::Complete;
                // Continue to CAP END
                self.finish_cap_and_register(config)
            }

            902 | 904 | 905 | 906 => {
                // SASL failed (ERR_NICKLOCKED, ERR_SASLFAIL, ERR_SASLTOOLONG, ERR_SASLABORTED)
                let reason = params
                    .last()
                    .cloned()
                    .unwrap_or_else(|| "Unknown error".into());
                self.sasl_state = SaslState::Complete;

                // For now, continue registration without SASL
                tracing::warn!("SASL authentication failed: {}", reason);
                self.finish_cap_and_register(config)
            }

            // Registration numerics
            1 => {
                // RPL_WELCOME
                self.server_name = msg_prefix_server(params.first());
                self.welcome_message = params.last().cloned();
                self.welcomed = true;
                self.phase = RegistrationPhase::Complete;

                let nick = config
                    .nicknames
                    .get(self.nick_index)
                    .cloned()
                    .unwrap_or_else(|| "unknown".into());

                RegistrationAction::Complete {
                    nick,
                    server: self.server_name.clone().unwrap_or_default(),
                    welcome: self.welcome_message.clone().unwrap_or_default(),
                }
            }

            // Nick errors during registration
            431 | 432 | 433 | 436 => {
                // ERR_NONICKNAMEGIVEN, ERR_ERRONEUSNICKNAME, ERR_NICKNAMEINUSE, ERR_NICKCOLLISION
                self.nick_index += 1;
                if self.nick_index >= config.nicknames.len() {
                    return RegistrationAction::Failed(RegistrationError::NoValidNick);
                }

                // Try next nickname
                let next_nick = config.nicknames[self.nick_index].clone();
                RegistrationAction::Send(vec![Message::new(Command::Nick {
                    nickname: next_nick,
                })])
            }

            // Banned
            465 => {
                // ERR_YOUREBANNEDCREEP
                let reason = params.last().cloned().unwrap_or_else(|| "Banned".into());
                RegistrationAction::Failed(RegistrationError::Banned(reason))
            }

            _ => RegistrationAction::Continue,
        }
    }

    /// Finish CAP negotiation and send NICK/USER.
    fn finish_cap_and_register(&mut self, config: &ClientConfig) -> RegistrationAction {
        self.caps.complete_negotiation();
        self.phase = RegistrationPhase::WaitingWelcome;

        let nick = config
            .nicknames
            .get(self.nick_index)
            .cloned()
            .unwrap_or_else(|| "user".into());

        let mut messages = vec![
            // CAP END
            Message::new(Command::Cap {
                subcommand: "END".into(),
                params: vec![],
            }),
        ];

        // PASS if configured
        if let Some(ref pass) = config.server_password {
            messages.push(Message::new(Command::Pass {
                password: pass.clone(),
            }));
        }

        // NICK and USER
        messages.push(Message::new(Command::Nick { nickname: nick }));

        messages.push(Message::new(Command::User {
            username: config.username.clone(),
            mode: 0,
            realname: config.realname.clone(),
        }));

        RegistrationAction::Send(messages)
    }
}

/// Encode SASL PLAIN credentials.
fn encode_sasl_plain(username: &str, password: &str) -> String {
    use base64::{Engine, engine::general_purpose::STANDARD};

    // PLAIN format: authzid\0authcid\0password
    // We leave authzid empty (use authcid)
    let plain = format!("\0{}\0{}", username, password);
    STANDARD.encode(plain.as_bytes())
}

/// Extract server name from message prefix.
fn msg_prefix_server(param: Option<&String>) -> Option<String> {
    param.cloned()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_config() -> ClientConfig {
        ClientConfig {
            nicknames: vec!["testnick".into()],
            username: "testuser".into(),
            realname: "Test User".into(),
            ..Default::default()
        }
    }

    #[test]
    fn test_start_registration() {
        let mut state = RegistrationState::new();
        let config = test_config();

        let messages = state.start(&config);

        assert_eq!(messages.len(), 1);
        assert!(matches!(
            &messages[0].command,
            Command::Cap { subcommand, params } if subcommand == "LS" && params == &["302"]
        ));
        assert_eq!(state.phase(), RegistrationPhase::WaitingCapLs);
    }

    #[test]
    fn test_sasl_plain_encoding() {
        let encoded = encode_sasl_plain("testuser", "testpass");
        // Should be base64 of "\0testuser\0testpass"
        use base64::{Engine, engine::general_purpose::STANDARD};
        let decoded = STANDARD.decode(&encoded).unwrap();
        assert_eq!(decoded, b"\0testuser\0testpass");
    }
}
