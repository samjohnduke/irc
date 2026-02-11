//! TCP/TLS listener management.

use std::fs::File;
use std::io::BufReader;
use std::sync::Arc;

use rustls::pki_types::CertificateDer;
use tokio::net::TcpListener;
use tokio_rustls::TlsAcceptor;

use crate::config::{ListenConfig, TlsConfig};
use crate::error::{Error, Result};

/// A configured listener (TCP or TLS).
pub struct Listener {
    /// The TCP listener.
    pub tcp: TcpListener,
    /// TLS acceptor if TLS is enabled.
    pub tls: Option<TlsAcceptor>,
    /// Whether this listener uses TLS.
    pub is_tls: bool,
}

/// Manages multiple listeners.
pub struct ListenerManager {
    listeners: Vec<Listener>,
}

impl ListenerManager {
    /// Create a new listener manager and bind to the configured addresses.
    pub async fn bind(configs: &[ListenConfig]) -> Result<Self> {
        let mut listeners = Vec::with_capacity(configs.len());

        for config in configs {
            let tcp = TcpListener::bind(config.address).await?;
            tracing::info!("Listening on {}", config.address);

            let tls = if let Some(ref tls_config) = config.tls {
                let acceptor = create_tls_acceptor(tls_config)?;
                tracing::info!("  TLS enabled");
                Some(acceptor)
            } else {
                None
            };

            listeners.push(Listener {
                tcp,
                tls,
                is_tls: config.tls.is_some(),
            });
        }

        Ok(Self { listeners })
    }

    /// Get an iterator over the listeners.
    pub fn iter(&self) -> impl Iterator<Item = &Listener> {
        self.listeners.iter()
    }

    /// Take ownership of the listeners.
    pub fn into_listeners(self) -> Vec<Listener> {
        self.listeners
    }
}

/// Create a TLS acceptor from the configuration.
fn create_tls_acceptor(config: &TlsConfig) -> Result<TlsAcceptor> {
    // Load certificates
    let cert_file = File::open(&config.cert_file).map_err(|e| {
        Error::Config(format!(
            "Failed to open certificate file {:?}: {}",
            config.cert_file, e
        ))
    })?;
    let mut cert_reader = BufReader::new(cert_file);
    let certs: Vec<CertificateDer<'static>> = rustls_pemfile::certs(&mut cert_reader)
        .collect::<std::result::Result<Vec<_>, _>>()
        .map_err(|e| Error::Config(format!("Failed to parse certificates: {}", e)))?;

    if certs.is_empty() {
        return Err(Error::Config(
            "No certificates found in certificate file".into(),
        ));
    }

    // Load private key
    let key_file = File::open(&config.key_file).map_err(|e| {
        Error::Config(format!(
            "Failed to open key file {:?}: {}",
            config.key_file, e
        ))
    })?;
    let mut key_reader = BufReader::new(key_file);
    let key = rustls_pemfile::private_key(&mut key_reader)
        .map_err(|e| Error::Config(format!("Failed to parse private key: {}", e)))?
        .ok_or_else(|| Error::Config("No private key found in key file".into()))?;

    // Build TLS config
    let tls_config = rustls::ServerConfig::builder()
        .with_no_client_auth()
        .with_single_cert(certs, key)
        .map_err(|e| Error::Config(format!("Failed to configure TLS: {}", e)))?;

    Ok(TlsAcceptor::from(Arc::new(tls_config)))
}
