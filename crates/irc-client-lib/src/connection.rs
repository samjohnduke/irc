//! TCP and TLS connection handling.
//!
//! This module handles establishing connections to IRC servers,
//! including TLS negotiation.

use std::io;
use std::net::SocketAddr;
use std::sync::Arc;

use futures::stream::{SplitSink, SplitStream};
use futures::{SinkExt, StreamExt};
use tokio::net::TcpStream;
use tokio_rustls::client::TlsStream;
use tokio_rustls::rustls::pki_types::ServerName;
use tokio_rustls::rustls::{ClientConfig as TlsConfig, RootCertStore};
use tokio_rustls::TlsConnector;
use tokio_util::codec::Framed;

use irc_proto::{Message, MessageCodec, MAX_MESSAGE_LEN_IRCV3};

use crate::config::ClientConfig;
use crate::error::{ConnectionError, TlsError};

/// A connection to an IRC server.
pub enum Connection {
    /// Plain TCP connection.
    Plain(Framed<TcpStream, MessageCodec>),

    /// TLS-encrypted connection.
    Tls(Framed<TlsStream<TcpStream>, MessageCodec>),
}

/// Split connection for reading and writing.
pub enum SplitConnection {
    Plain {
        read: SplitStream<Framed<TcpStream, MessageCodec>>,
        write: SplitSink<Framed<TcpStream, MessageCodec>, Message>,
    },
    Tls {
        read: SplitStream<Framed<TlsStream<TcpStream>, MessageCodec>>,
        write: SplitSink<Framed<TlsStream<TcpStream>, MessageCodec>, Message>,
    },
}

/// Reader half of a split connection.
pub enum ConnectionReader {
    Plain(SplitStream<Framed<TcpStream, MessageCodec>>),
    Tls(SplitStream<Framed<TlsStream<TcpStream>, MessageCodec>>),
}

/// Writer half of a split connection.
pub enum ConnectionWriter {
    Plain(SplitSink<Framed<TcpStream, MessageCodec>, Message>),
    Tls(SplitSink<Framed<TlsStream<TcpStream>, MessageCodec>, Message>),
}

impl Connection {
    /// Establish a connection to the server.
    pub async fn connect(config: &ClientConfig) -> Result<Self, ConnectionError> {
        let addr = resolve_address(&config.server, config.port).await?;

        tracing::debug!("Connecting to {} ({})", config.server, addr);

        let tcp_stream = TcpStream::connect(addr).await.map_err(|e| {
            ConnectionError::TcpConnect {
                addr: addr.to_string(),
                source: e,
            }
        })?;

        // Use IRCv3 max length to support message-tags (servers can send up to 8191 bytes)
        let codec = MessageCodec::with_max_length(MAX_MESSAGE_LEN_IRCV3);

        if config.tls {
            let tls_stream = establish_tls(tcp_stream, &config.server, config.tls_accept_invalid).await?;
            let framed = Framed::new(tls_stream, codec);
            Ok(Connection::Tls(framed))
        } else {
            let framed = Framed::new(tcp_stream, codec);
            Ok(Connection::Plain(framed))
        }
    }

    /// Split the connection into read and write halves.
    pub fn split(self) -> (ConnectionReader, ConnectionWriter) {
        match self {
            Connection::Plain(framed) => {
                let (write, read) = framed.split();
                (ConnectionReader::Plain(read), ConnectionWriter::Plain(write))
            }
            Connection::Tls(framed) => {
                let (write, read) = framed.split();
                (ConnectionReader::Tls(read), ConnectionWriter::Tls(write))
            }
        }
    }

    /// Send a message.
    pub async fn send(&mut self, msg: Message) -> io::Result<()> {
        match self {
            Connection::Plain(framed) => framed.send(msg).await,
            Connection::Tls(framed) => framed.send(msg).await,
        }
    }

    /// Receive the next message.
    pub async fn recv(&mut self) -> Option<Result<Message, irc_proto::ParseError>> {
        match self {
            Connection::Plain(framed) => framed.next().await,
            Connection::Tls(framed) => framed.next().await,
        }
    }
}

impl ConnectionReader {
    /// Receive the next message.
    pub async fn recv(&mut self) -> Option<Result<Message, irc_proto::ParseError>> {
        match self {
            ConnectionReader::Plain(stream) => stream.next().await,
            ConnectionReader::Tls(stream) => stream.next().await,
        }
    }
}

impl ConnectionWriter {
    /// Send a message.
    pub async fn send(&mut self, msg: Message) -> io::Result<()> {
        match self {
            ConnectionWriter::Plain(sink) => sink.send(msg).await,
            ConnectionWriter::Tls(sink) => sink.send(msg).await,
        }
    }

    /// Flush pending writes.
    pub async fn flush(&mut self) -> io::Result<()> {
        match self {
            ConnectionWriter::Plain(sink) => sink.flush().await,
            ConnectionWriter::Tls(sink) => sink.flush().await,
        }
    }
}

/// Resolve hostname to socket address.
async fn resolve_address(host: &str, port: u16) -> Result<SocketAddr, ConnectionError> {
    use tokio::net::lookup_host;

    let addr_string = format!("{}:{}", host, port);
    let addrs: Vec<_> = lookup_host(&addr_string)
        .await
        .map_err(|e| ConnectionError::DnsResolution {
            host: host.to_string(),
            source: e,
        })?
        .collect();

    // Prefer IPv4 for compatibility
    addrs
        .iter()
        .find(|a| a.is_ipv4())
        .or(addrs.first())
        .copied()
        .ok_or_else(|| {
            ConnectionError::DnsResolution {
                host: host.to_string(),
                source: io::Error::new(io::ErrorKind::NotFound, "no addresses found"),
            }
        })
}

/// Establish TLS connection.
async fn establish_tls(
    tcp_stream: TcpStream,
    server_name: &str,
    accept_invalid: bool,
) -> Result<TlsStream<TcpStream>, ConnectionError> {
    let tls_config = if accept_invalid {
        create_insecure_tls_config()?
    } else {
        create_tls_config()?
    };

    let connector = TlsConnector::from(Arc::new(tls_config));

    let server_name = ServerName::try_from(server_name.to_string())
        .map_err(|_| ConnectionError::InvalidServerName(server_name.to_string()))?;

    connector
        .connect(server_name, tcp_stream)
        .await
        .map_err(|e| ConnectionError::TlsHandshake(TlsError::Handshake(e.to_string())))
}

/// Create TLS config with system root certificates.
fn create_tls_config() -> Result<TlsConfig, ConnectionError> {
    let mut root_store = RootCertStore::empty();
    root_store.extend(webpki_roots::TLS_SERVER_ROOTS.iter().cloned());

    TlsConfig::builder()
        .with_root_certificates(root_store)
        .with_no_client_auth()
        .pipe(Ok)
}

/// Create insecure TLS config (for testing).
fn create_insecure_tls_config() -> Result<TlsConfig, ConnectionError> {
    use tokio_rustls::rustls::client::danger::{
        HandshakeSignatureValid, ServerCertVerified, ServerCertVerifier,
    };
    use tokio_rustls::rustls::pki_types::{CertificateDer, UnixTime};
    use tokio_rustls::rustls::{DigitallySignedStruct, SignatureScheme};

    #[derive(Debug)]
    struct InsecureVerifier;

    impl ServerCertVerifier for InsecureVerifier {
        fn verify_server_cert(
            &self,
            _end_entity: &CertificateDer<'_>,
            _intermediates: &[CertificateDer<'_>],
            _server_name: &ServerName<'_>,
            _ocsp_response: &[u8],
            _now: UnixTime,
        ) -> Result<ServerCertVerified, tokio_rustls::rustls::Error> {
            Ok(ServerCertVerified::assertion())
        }

        fn verify_tls12_signature(
            &self,
            _message: &[u8],
            _cert: &CertificateDer<'_>,
            _dss: &DigitallySignedStruct,
        ) -> Result<HandshakeSignatureValid, tokio_rustls::rustls::Error> {
            Ok(HandshakeSignatureValid::assertion())
        }

        fn verify_tls13_signature(
            &self,
            _message: &[u8],
            _cert: &CertificateDer<'_>,
            _dss: &DigitallySignedStruct,
        ) -> Result<HandshakeSignatureValid, tokio_rustls::rustls::Error> {
            Ok(HandshakeSignatureValid::assertion())
        }

        fn supported_verify_schemes(&self) -> Vec<SignatureScheme> {
            vec![
                SignatureScheme::RSA_PKCS1_SHA256,
                SignatureScheme::RSA_PKCS1_SHA384,
                SignatureScheme::RSA_PKCS1_SHA512,
                SignatureScheme::ECDSA_NISTP256_SHA256,
                SignatureScheme::ECDSA_NISTP384_SHA384,
                SignatureScheme::ECDSA_NISTP521_SHA512,
                SignatureScheme::RSA_PSS_SHA256,
                SignatureScheme::RSA_PSS_SHA384,
                SignatureScheme::RSA_PSS_SHA512,
                SignatureScheme::ED25519,
            ]
        }
    }

    TlsConfig::builder()
        .dangerous()
        .with_custom_certificate_verifier(Arc::new(InsecureVerifier))
        .with_no_client_auth()
        .pipe(Ok)
}

/// Helper trait for method chaining.
trait Pipe: Sized {
    fn pipe<F, R>(self, f: F) -> R
    where
        F: FnOnce(Self) -> R,
    {
        f(self)
    }
}

impl<T> Pipe for T {}
