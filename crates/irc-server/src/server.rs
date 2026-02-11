//! Main server implementation.

use std::sync::Arc;

use tokio::signal;

use crate::config::ServerConfig;
use crate::connection::{handle_connection, ListenerManager};
use crate::error::Result;
use crate::state::ServerState;

/// The IRC server.
pub struct Server {
    config: ServerConfig,
}

impl Server {
    /// Create a new server with the given configuration.
    pub fn new(config: ServerConfig) -> Self {
        Self { config }
    }

    /// Run the server.
    pub async fn run(self) -> Result<()> {
        tracing::info!("Starting IRC server: {}", self.config.server_name);
        tracing::info!("Network: {}", self.config.network_name);

        // Initialize server state
        let state = Arc::new(ServerState::new(self.config.clone()));

        // Load MOTD if configured
        if let Some(ref motd_path) = self.config.motd_file {
            match std::fs::read_to_string(motd_path) {
                Ok(content) => {
                    let lines: Vec<String> = content.lines().map(String::from).collect();
                    tracing::info!("Loaded MOTD from {:?} ({} lines)", motd_path, lines.len());
                    *state.motd.write().await = Some(lines);
                }
                Err(e) => {
                    tracing::warn!("Failed to load MOTD from {:?}: {}", motd_path, e);
                }
            }
        }

        // Bind listeners
        let listeners = ListenerManager::bind(&self.config.listen).await?;

        // Accept connections on all listeners
        let mut handles = Vec::new();

        for listener in listeners.into_listeners() {
            let state = Arc::clone(&state);
            let _is_tls = listener.is_tls;
            let tls_acceptor = listener.tls;

            let handle = tokio::spawn(async move {
                loop {
                    match listener.tcp.accept().await {
                        Ok((stream, addr)) => {
                            let state = Arc::clone(&state);

                            if let Some(ref acceptor) = tls_acceptor {
                                // TLS connection
                                let acceptor = acceptor.clone();
                                tokio::spawn(async move {
                                    match acceptor.accept(stream).await {
                                        Ok(tls_stream) => {
                                            if let Err(e) =
                                                handle_connection(tls_stream, addr, state, true)
                                                    .await
                                            {
                                                tracing::debug!(
                                                    %addr,
                                                    error = %e,
                                                    "Connection error"
                                                );
                                            }
                                        }
                                        Err(e) => {
                                            tracing::debug!(
                                                %addr,
                                                error = %e,
                                                "TLS handshake failed"
                                            );
                                        }
                                    }
                                });
                            } else {
                                // Plain TCP connection
                                tokio::spawn(async move {
                                    if let Err(e) =
                                        handle_connection(stream, addr, state, false).await
                                    {
                                        tracing::debug!(%addr, error = %e, "Connection error");
                                    }
                                });
                            }
                        }
                        Err(e) => {
                            tracing::error!(error = %e, "Accept error");
                        }
                    }
                }
            });

            handles.push(handle);
        }

        tracing::info!("Server is ready");

        // Wait for shutdown signal
        match signal::ctrl_c().await {
            Ok(()) => {
                tracing::info!("Received shutdown signal");
            }
            Err(e) => {
                tracing::error!(error = %e, "Failed to listen for shutdown signal");
            }
        }

        // Abort all listener tasks
        for handle in handles {
            handle.abort();
        }

        tracing::info!("Server shutdown complete");
        Ok(())
    }
}
