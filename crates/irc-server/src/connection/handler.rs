//! Per-connection handler.

use std::net::SocketAddr;
use std::sync::Arc;

use futures::StreamExt;
use irc_proto::{Message, MessageCodec};
use tokio::io::{AsyncRead, AsyncWrite};
use tokio::sync::mpsc;
use tokio_util::codec::Framed;

use crate::error::Result;
use crate::handler::handle_message;
use crate::state::{Client, ServerState};

/// Handle a new client connection.
///
/// This spawns the read and write loops for the connection.
pub async fn handle_connection<S>(
    stream: S,
    addr: SocketAddr,
    state: Arc<ServerState>,
    tls: bool,
) -> Result<()>
where
    S: AsyncRead + AsyncWrite + Send + Unpin + 'static,
{
    let client_id = state.next_client_id();
    tracing::info!(%client_id, %addr, tls, "New connection");

    // Create bounded channel for outgoing messages (prevents backpressure from slow clients)
    let buffer_size = state.config.limits.send_buffer_size;
    let (tx, rx) = mpsc::channel::<Message>(buffer_size);

    // Create client
    let client = Arc::new(Client::new(client_id, addr, tx, tls));
    state.add_client(Arc::clone(&client));

    // Split the stream for read/write
    let framed = Framed::new(stream, MessageCodec::new());
    let (writer, reader) = framed.split();

    // Spawn write task
    let write_handle = tokio::spawn(write_loop(rx, writer));

    // Run read loop in current task
    let result = read_loop(reader, Arc::clone(&client), Arc::clone(&state)).await;

    // Clean up
    tracing::info!(%client_id, "Connection closed");

    // Cancel write task
    write_handle.abort();

    // Remove client from state
    let _ = state.remove_client(client_id);

    result
}

/// Read loop - receives messages from the client and dispatches them.
async fn read_loop<S>(
    mut reader: futures::stream::SplitStream<Framed<S, MessageCodec>>,
    client: Arc<Client>,
    state: Arc<ServerState>,
) -> Result<()>
where
    S: AsyncRead + AsyncWrite + Unpin,
{
    while let Some(result) = reader.next().await {
        match result {
            Ok(message) => {
                tracing::trace!(client_id = %client.id, ?message, "Received");

                if let Err(e) = handle_message(&client, &state, message).await {
                    tracing::debug!(client_id = %client.id, error = %e, "Handler error");
                    // Most errors are sent as numeric replies, not connection errors
                }
            }
            Err(e) => {
                tracing::debug!(client_id = %client.id, error = %e, "Parse error");
                // Continue on parse errors, only disconnect on I/O errors
                if matches!(e, irc_proto::ParseError::MessageTooLong(_)) {
                    continue;
                }
            }
        }
    }

    Ok(())
}

/// Write loop - sends messages to the client.
async fn write_loop<S>(
    mut rx: mpsc::Receiver<Message>,
    mut writer: futures::stream::SplitSink<Framed<S, MessageCodec>, Message>,
) where
    S: AsyncRead + AsyncWrite + Unpin,
{
    use futures::SinkExt;

    while let Some(message) = rx.recv().await {
        tracing::trace!(?message, "Sending");

        if let Err(e) = writer.send(message).await {
            tracing::debug!(error = %e, "Write error");
            break;
        }
    }
}
