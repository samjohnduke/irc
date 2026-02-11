//! IRC server implementation.
//!
//! This crate provides a complete IRC daemon supporting:
//! - Client connections (TCP and TLS)
//! - Channel management
//! - User modes and permissions
//! - IRCv3 extensions
//! - SASL authentication
//! - Built-in services (NickServ, ChanServ)

pub mod cap;
pub mod config;
pub mod connection;
pub mod error;
pub mod handler;
pub mod lock;
pub mod reply;
pub mod server;
pub mod state;

pub use config::ServerConfig;
pub use error::{Error, Result};
pub use lock::RwLockExt;
pub use server::Server;
pub use state::ServerState;
