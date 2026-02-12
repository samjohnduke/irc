//! Application state.

pub mod buffer;
pub mod message;

pub use buffer::{Buffer, BufferKind, BufferList};
pub use message::{DisplayMessage, MessageKind};
