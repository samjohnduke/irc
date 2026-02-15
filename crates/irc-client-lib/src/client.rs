//! IRC client connection.
//!
//! The `Client` struct provides the main API for connecting to IRC servers
//! and sending/receiving messages.

use std::sync::Arc;

use tokio::sync::{broadcast, mpsc, RwLock};
use tokio::time::{timeout, Duration};

use irc_proto::{Command, Message};

use crate::batch::{BatchCollector, BatchResult};
use crate::config::ClientConfig;
use crate::connection::{Connection, ConnectionReader, ConnectionWriter};
use crate::error::Error;
use crate::event::Event;
use crate::handler::message_to_event;
use crate::registration::{RegistrationAction, RegistrationState};
use crate::state::SessionState;

/// Event channel capacity.
/// Needs to be large enough to handle bursts like LIST responses.
const EVENT_CHANNEL_SIZE: usize = 4096;

/// Command channel capacity.
/// Needs headroom for PONG responses during heavy traffic.
const COMMAND_CHANNEL_SIZE: usize = 256;

/// An IRC client.
pub struct Client {
    /// Client configuration.
    config: ClientConfig,

    /// Current session state.
    state: Arc<RwLock<SessionState>>,

    /// Channel for sending commands to the write task.
    command_tx: Option<mpsc::Sender<Message>>,

    /// Channel for receiving events.
    event_tx: broadcast::Sender<Event>,

    /// Whether we're connected.
    connected: bool,
}

impl Client {
    /// Create a new disconnected client with the given configuration.
    pub fn new(config: ClientConfig) -> Self {
        let (event_tx, _) = broadcast::channel(EVENT_CHANNEL_SIZE);

        Self {
            config,
            state: Arc::new(RwLock::new(SessionState::new())),
            command_tx: None,
            event_tx,
            connected: false,
        }
    }

    /// Connect to the IRC server.
    pub async fn connect(&mut self) -> Result<(), Error> {
        tracing::info!("Connecting to {}:{}", self.config.server, self.config.port);

        // Emit connecting event
        let _ = self.event_tx.send(Event::Connecting);
        let _ = self.event_tx.send(Event::ConnectionProgress {
            phase: "connecting".into(),
            message: format!("Connecting to {}:{}...", self.config.server, self.config.port),
        });

        // Establish connection
        let connection = match Connection::connect(&self.config).await {
            Ok(conn) => {
                let tls_info = if self.config.tls { " (TLS)" } else { "" };
                let _ = self.event_tx.send(Event::ConnectionProgress {
                    phase: "connected".into(),
                    message: format!("TCP connection established{}", tls_info),
                });
                conn
            }
            Err(e) => {
                let _ = self.event_tx.send(Event::ConnectionProgress {
                    phase: "error".into(),
                    message: format!("Connection failed: {}", e),
                });
                return Err(Error::Connection(e));
            }
        };
        let (reader, writer) = connection.split();

        // Create command channel
        let (command_tx, command_rx) = mpsc::channel(COMMAND_CHANNEL_SIZE);
        self.command_tx = Some(command_tx.clone());

        // Clone references for tasks
        let state = Arc::clone(&self.state);
        let event_tx = self.event_tx.clone();
        let config = self.config.clone();

        // Spawn read task
        let read_state = Arc::clone(&state);
        let read_event_tx = event_tx.clone();
        let read_command_tx = command_tx.clone();
        tokio::spawn(async move {
            read_loop(reader, read_state, read_event_tx, read_command_tx, config).await;
        });

        // Spawn write task
        tokio::spawn(async move {
            write_loop(writer, command_rx).await;
        });

        // Start registration
        let _ = self.event_tx.send(Event::ConnectionProgress {
            phase: "capabilities".into(),
            message: "Negotiating capabilities (CAP LS 302)...".into(),
        });

        let mut reg_state = RegistrationState::new();
        let initial_messages = reg_state.start(&self.config);

        // Send initial registration messages
        for msg in initial_messages {
            if let Some(ref tx) = self.command_tx {
                tx.send(msg).await.map_err(|_| Error::SendError)?;
            }
        }

        // Wait for registration to complete
        let mut event_rx = self.event_tx.subscribe();
        let registration_timeout = Duration::from_secs(30);

        let result = timeout(registration_timeout, async {
            loop {
                match event_rx.recv().await {
                    Ok(Event::Connected { nick, server, welcome: _ }) => {
                        // Update state
                        let mut state = self.state.write().await;
                        state.set_nick(&nick);
                        state.set_server_name(&server);
                        state.set_registered(true);

                        self.connected = true;
                        return Ok(());
                    }
                    Ok(Event::Error { message }) => {
                        return Err(Error::Registration(
                            crate::error::RegistrationError::Rejected(message),
                        ));
                    }
                    Ok(Event::Disconnected { reason, .. }) => {
                        return Err(Error::Registration(
                            crate::error::RegistrationError::Rejected(
                                reason.unwrap_or_else(|| "Connection closed".into()),
                            ),
                        ));
                    }
                    Err(broadcast::error::RecvError::Closed) => {
                        return Err(Error::Disconnected);
                    }
                    Err(broadcast::error::RecvError::Lagged(_)) => {
                        // Continue, we might have missed some events
                        continue;
                    }
                    _ => continue,
                }
            }
        })
        .await;

        match result {
            Ok(Ok(())) => {
                // Auto-join channels
                for channel in &self.config.autojoin.clone() {
                    self.join(channel).await?;
                }
                Ok(())
            }
            Ok(Err(e)) => Err(e),
            Err(_) => Err(Error::Registration(crate::error::RegistrationError::Timeout)),
        }
    }

    /// Subscribe to events.
    pub fn subscribe(&self) -> broadcast::Receiver<Event> {
        self.event_tx.subscribe()
    }

    /// Get the current nickname.
    pub async fn nick(&self) -> String {
        self.state.read().await.nick().to_string()
    }

    /// Check if we're connected and registered.
    pub fn is_connected(&self) -> bool {
        self.connected
    }

    /// Get enabled IRCv3 capabilities.
    pub async fn capabilities(&self) -> Vec<String> {
        let state = self.state.read().await;
        state.caps().enabled_caps().map(|s| s.to_string()).collect()
    }

    /// Send a raw IRC message.
    pub async fn send_raw(&self, msg: Message) -> Result<(), Error> {
        if let Some(ref tx) = self.command_tx {
            tx.send(msg).await.map_err(|_| Error::SendError)?;
            Ok(())
        } else {
            Err(Error::Disconnected)
        }
    }

    /// Join a channel.
    pub async fn join(&self, channel: &str) -> Result<(), Error> {
        self.send_raw(Message::new(Command::Join {
            channels: vec![(channel.to_string(), None)],
        }))
        .await
    }

    /// Join a channel with a key.
    pub async fn join_with_key(&self, channel: &str, key: &str) -> Result<(), Error> {
        self.send_raw(Message::new(Command::Join {
            channels: vec![(channel.to_string(), Some(key.to_string()))],
        }))
        .await
    }

    /// Leave a channel.
    pub async fn part(&self, channel: &str, message: Option<&str>) -> Result<(), Error> {
        self.send_raw(Message::new(Command::Part {
            channels: vec![channel.to_string()],
            message: message.map(String::from),
        }))
        .await
    }

    /// Send a message to a target (channel or user).
    pub async fn privmsg(&self, target: &str, message: &str) -> Result<(), Error> {
        self.send_raw(Message::new(Command::Privmsg {
            target: target.to_string(),
            message: message.to_string(),
        }))
        .await
    }

    /// Send a notice to a target.
    pub async fn notice(&self, target: &str, message: &str) -> Result<(), Error> {
        self.send_raw(Message::new(Command::Notice {
            target: target.to_string(),
            message: message.to_string(),
        }))
        .await
    }

    /// Send a CTCP ACTION (/me).
    pub async fn action(&self, target: &str, action: &str) -> Result<(), Error> {
        let ctcp_msg = format!("\x01ACTION {}\x01", action);
        self.privmsg(target, &ctcp_msg).await
    }

    /// Change nickname.
    pub async fn change_nick(&self, nick: &str) -> Result<(), Error> {
        self.send_raw(Message::new(Command::Nick {
            nickname: nick.to_string(),
        }))
        .await
    }

    /// Set or clear away status.
    pub async fn away(&self, message: Option<&str>) -> Result<(), Error> {
        self.send_raw(Message::new(Command::Away {
            message: message.map(String::from),
        }))
        .await
    }

    /// Set channel topic.
    pub async fn topic(&self, channel: &str, topic: Option<&str>) -> Result<(), Error> {
        self.send_raw(Message::new(Command::Topic {
            channel: channel.to_string(),
            topic: topic.map(String::from),
        }))
        .await
    }

    /// Kick a user from a channel.
    pub async fn kick(&self, channel: &str, user: &str, reason: Option<&str>) -> Result<(), Error> {
        self.send_raw(Message::new(Command::Kick {
            channel: channel.to_string(),
            users: vec![user.to_string()],
            comment: reason.map(String::from),
        }))
        .await
    }

    /// Invite a user to a channel.
    pub async fn invite(&self, nick: &str, channel: &str) -> Result<(), Error> {
        self.send_raw(Message::new(Command::Invite {
            nickname: nick.to_string(),
            channel: channel.to_string(),
        }))
        .await
    }

    /// Request chat history for a target.
    pub async fn chathistory(&self, target: &str, limit: usize) -> Result<(), Error> {
        self.send_raw(Message::new(Command::Chathistory {
            subcommand: "LATEST".to_string(),
            target: target.to_string(),
            params: vec!["*".to_string(), limit.to_string()],
        }))
        .await
    }

    /// Quit with an optional message.
    pub async fn quit(&mut self, message: Option<&str>) -> Result<(), Error> {
        let result = self
            .send_raw(Message::new(Command::Quit {
                message: message.map(String::from),
            }))
            .await;

        self.connected = false;
        self.command_tx = None;

        result
    }

    /// Get a reference to the session state.
    pub fn state(&self) -> Arc<RwLock<SessionState>> {
        Arc::clone(&self.state)
    }
}

impl Default for Client {
    fn default() -> Self {
        Self::new(ClientConfig::default())
    }
}

/// Read loop task.
async fn read_loop(
    mut reader: ConnectionReader,
    state: Arc<RwLock<SessionState>>,
    event_tx: broadcast::Sender<Event>,
    command_tx: mpsc::Sender<Message>,
    config: ClientConfig,
) {
    tracing::debug!("Read loop started");
    let mut batch_collector = BatchCollector::new();
    let mut reg_state = RegistrationState::new();
    reg_state.start(&config);
    tracing::debug!("Registration state initialized, phase: {:?}", reg_state.phase());

    loop {
        match reader.recv().await {
            Some(Ok(msg)) => {
                tracing::debug!("Received message: {:?}", msg.command);

                // Handle registration
                if !reg_state.is_complete() && !reg_state.is_failed() {
                    let old_phase = reg_state.phase();
                    tracing::debug!("Processing registration, current phase: {:?}", old_phase);
                    match reg_state.process(&msg, &config) {
                        RegistrationAction::Send(messages) => {
                            tracing::debug!("Registration action: Send {} messages", messages.len());
                            // Check for phase transitions and emit progress
                            let new_phase = reg_state.phase();
                            tracing::debug!("Phase transition: {:?} -> {:?}", old_phase, new_phase);
                            if old_phase != new_phase {
                                let (phase, message) = match new_phase {
                                    crate::registration::RegistrationPhase::WaitingCapAck => {
                                        ("capabilities", "Received server capabilities, requesting...")
                                    }
                                    crate::registration::RegistrationPhase::SaslAuth => {
                                        ("authenticating", "Starting SASL PLAIN authentication...")
                                    }
                                    crate::registration::RegistrationPhase::WaitingWelcome => {
                                        ("registering", "Completing registration (NICK/USER)...")
                                    }
                                    _ => ("", ""),
                                };
                                if !phase.is_empty() {
                                    let _ = event_tx.send(Event::ConnectionProgress {
                                        phase: phase.into(),
                                        message: message.into(),
                                    });
                                }
                            }

                            for m in messages {
                                let _ = command_tx.send(m).await;
                            }
                        }
                        RegistrationAction::Complete { nick, server, welcome } => {
                            tracing::info!("Registration complete: nick={}, server={}", nick, server);
                            let _ = event_tx.send(Event::ConnectionProgress {
                                phase: "complete".into(),
                                message: format!("Registered as {}", nick),
                            });
                            let _ = event_tx.send(Event::Connected {
                                nick,
                                server,
                                welcome,
                            });
                        }
                        RegistrationAction::Failed(e) => {
                            tracing::error!("Registration failed: {}", e);
                            let _ = event_tx.send(Event::ConnectionProgress {
                                phase: "error".into(),
                                message: format!("Registration failed: {}", e),
                            });
                            let _ = event_tx.send(Event::Error {
                                message: e.to_string(),
                            });
                            break;
                        }
                        RegistrationAction::Continue => {
                            tracing::trace!("Registration action: Continue");
                        }
                    }
                }

                // Auto-reply to PING (critical for staying connected)
                if let Command::Ping { server1, .. } = &msg.command {
                    let pong = Message::new(Command::Pong {
                        server1: server1.clone(),
                        server2: None,
                    });
                    if let Err(e) = command_tx.send(pong).await {
                        tracing::error!("Failed to send PONG: {} - connection may drop!", e);
                    }
                }

                // Process through batch collector
                let batch_result = batch_collector.process(msg.clone(), |m| {
                    let mut state = futures::executor::block_on(state.write());
                    message_to_event(&m, &mut state)
                });

                match batch_result {
                    BatchResult::NotBatched(msg) => {
                        // Convert to event and broadcast
                        let mut state = state.write().await;
                        if let Some(event) = message_to_event(&msg, &mut state) {
                            let _ = event_tx.send(event);
                        }
                    }
                    BatchResult::Complete(event) => {
                        let _ = event_tx.send(event);
                    }
                    BatchResult::Batched | BatchResult::Started => {
                        // Message added to batch, no event yet
                    }
                }
            }

            Some(Err(e)) => {
                tracing::error!("Parse error: {}", e);
            }

            None => {
                // Connection closed
                tracing::info!("Connection closed");
                let _ = event_tx.send(Event::Disconnected {
                    reason: None,
                    clean: true,
                });
                break;
            }
        }
    }
}

/// Write loop task.
async fn write_loop(mut writer: ConnectionWriter, mut command_rx: mpsc::Receiver<Message>) {
    while let Some(msg) = command_rx.recv().await {
        tracing::trace!("Sending: {}", msg);
        if let Err(e) = writer.send(msg).await {
            tracing::error!("Send error: {}", e);
            break;
        }
        // Flush after each message to ensure it's sent immediately
        if let Err(e) = writer.flush().await {
            tracing::error!("Flush error: {}", e);
            break;
        }
    }
}
