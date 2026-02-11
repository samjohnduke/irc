//! Low-level parsing utilities.
//!
//! This module provides the tokio codec for streaming message parsing.

use bytes::{Buf, BytesMut};
use tokio_util::codec::{Decoder, Encoder};

use crate::error::ParseError;
use crate::message::Message;
use crate::MAX_MESSAGE_LEN;

/// Codec for parsing and serializing IRC messages.
///
/// This codec handles the CRLF-delimited framing of IRC messages.
///
/// # Example
///
/// ```ignore
/// use tokio_util::codec::Framed;
/// use irc_proto::MessageCodec;
///
/// async fn example(stream: tokio::net::TcpStream) {
///     let framed = Framed::new(stream, MessageCodec::new());
///     // Use framed.next() and framed.send()
/// }
/// ```
#[derive(Debug, Clone)]
pub struct MessageCodec {
    max_length: usize,
}

impl MessageCodec {
    /// Create a new codec with default max length (512 bytes).
    pub fn new() -> Self {
        Self {
            max_length: MAX_MESSAGE_LEN,
        }
    }

    /// Create a codec with a custom max length.
    pub fn with_max_length(max_length: usize) -> Self {
        Self { max_length }
    }
}

impl Default for MessageCodec {
    fn default() -> Self {
        Self::new()
    }
}

impl Decoder for MessageCodec {
    type Item = Message;
    type Error = ParseError;

    fn decode(&mut self, src: &mut BytesMut) -> Result<Option<Message>, ParseError> {
        // Look for CRLF or just LF (be lenient)
        let newline_pos = src.iter().position(|&b| b == b'\n');

        match newline_pos {
            Some(pos) => {
                // Check length before CRLF
                let line_len = if pos > 0 && src[pos - 1] == b'\r' {
                    pos - 1
                } else {
                    pos
                };

                if line_len > self.max_length - 2 {
                    // Clear the invalid line
                    src.advance(pos + 1);
                    return Err(ParseError::MessageTooLong(line_len + 2));
                }

                // Extract the line including the newline
                let line = src.split_to(pos + 1);

                // Skip empty lines
                if line_len == 0 {
                    return Ok(None);
                }

                // Parse the message
                Message::parse(&line[..]).map(Some)
            }
            None => {
                // No complete line yet
                if src.len() > self.max_length {
                    // Line is too long, will never be valid
                    src.clear();
                    return Err(ParseError::MessageTooLong(src.len()));
                }
                Ok(None)
            }
        }
    }
}

impl Encoder<Message> for MessageCodec {
    type Error = std::io::Error;

    fn encode(&mut self, msg: Message, dst: &mut BytesMut) -> Result<(), Self::Error> {
        let bytes = msg.to_bytes();

        if bytes.len() > self.max_length {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                format!("message too long: {} bytes", bytes.len()),
            ));
        }

        dst.extend_from_slice(&bytes);
        Ok(())
    }
}

impl Encoder<&Message> for MessageCodec {
    type Error = std::io::Error;

    fn encode(&mut self, msg: &Message, dst: &mut BytesMut) -> Result<(), Self::Error> {
        let bytes = msg.to_bytes();

        if bytes.len() > self.max_length {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                format!("message too long: {} bytes", bytes.len()),
            ));
        }

        dst.extend_from_slice(&bytes);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_decode_simple() {
        let mut codec = MessageCodec::new();
        let mut buf = BytesMut::from("PING :server\r\n");

        let msg = codec.decode(&mut buf).unwrap().unwrap();
        assert!(matches!(msg.command, crate::Command::Ping { .. }));
        assert!(buf.is_empty());
    }

    #[test]
    fn test_decode_multiple() {
        let mut codec = MessageCodec::new();
        let mut buf = BytesMut::from("PING :a\r\nPING :b\r\n");

        let msg1 = codec.decode(&mut buf).unwrap().unwrap();
        let msg2 = codec.decode(&mut buf).unwrap().unwrap();

        assert!(matches!(msg1.command, crate::Command::Ping { .. }));
        assert!(matches!(msg2.command, crate::Command::Ping { .. }));
    }

    #[test]
    fn test_decode_partial() {
        let mut codec = MessageCodec::new();
        let mut buf = BytesMut::from("PING :ser");

        // Not complete yet
        assert!(codec.decode(&mut buf).unwrap().is_none());

        // Add the rest
        buf.extend_from_slice(b"ver\r\n");
        let msg = codec.decode(&mut buf).unwrap().unwrap();
        assert!(matches!(msg.command, crate::Command::Ping { .. }));
    }

    #[test]
    fn test_decode_lf_only() {
        let mut codec = MessageCodec::new();
        let mut buf = BytesMut::from("PING :server\n");

        // Should accept LF-only
        let msg = codec.decode(&mut buf).unwrap().unwrap();
        assert!(matches!(msg.command, crate::Command::Ping { .. }));
    }

    #[test]
    fn test_encode() {
        let mut codec = MessageCodec::new();
        let mut buf = BytesMut::new();

        let msg = Message::new(crate::Command::Ping {
            server1: "test".into(),
            server2: None,
        });

        codec.encode(msg, &mut buf).unwrap();
        assert_eq!(&buf[..], b"PING test\r\n");
    }
}
