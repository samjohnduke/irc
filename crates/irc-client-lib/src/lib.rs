//! IRC client library.
//!
//! This crate provides a full-featured IRC client library with:
//! - Connection management (TCP, TLS)
//! - IRCv3 capability negotiation
//! - SASL authentication
//! - Session state tracking
//! - Event-driven API
//!
//! # Example
//!
//! ```ignore
//! use irc_client_lib::{Client, ClientConfig, Event};
//!
//! #[tokio::main]
//! async fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     let config = ClientConfig::new("mynick")
//!         .server("irc.libera.chat", 6697)
//!         .tls(true);
//!
//!     let mut client = Client::new(config);
//!     client.connect().await?;
//!
//!     let mut events = client.subscribe();
//!     client.join("#test").await?;
//!
//!     while let Ok(event) = events.recv().await {
//!         match event {
//!             Event::Privmsg { source, target, message, .. } => {
//!                 println!("{} -> {}: {}", source, target, message);
//!             }
//!             _ => {}
//!         }
//!     }
//!
//!     Ok(())
//! }
//! ```

pub mod batch;
pub mod cap;
pub mod client;
pub mod config;
pub mod connection;
pub mod error;
pub mod event;
pub mod handler;
pub mod registration;
pub mod state;

pub use client::Client;
pub use config::{ClientConfig, SaslConfig, SaslMechanism};
pub use error::{ConnectionError, Error, RegistrationError, SaslError};
pub use event::{Event, MessageMeta};
pub use state::{ChannelState, MemberInfo, SessionState, TopicInfo, UserInfo};
