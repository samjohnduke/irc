//! IRC client library.
//!
//! This crate provides shared functionality for IRC clients:
//! - Connection management (TCP, TLS)
//! - Session state tracking
//! - Event-driven API
//! - Multi-server support

pub mod client;
pub mod config;
pub mod event;

pub use client::Client;
pub use config::ClientConfig;
pub use event::Event;
