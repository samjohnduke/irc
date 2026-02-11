//! Test utilities for integration tests.

use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;

use futures::{SinkExt, StreamExt};
use irc_proto::{Command, Message, MessageCodec};
use tokio::net::TcpStream;
use tokio::sync::oneshot;
use tokio_util::codec::Framed;

use irc_server::{ServerConfig, ServerState};

/// A test server that runs on a random port.
pub struct TestServer {
    pub addr: SocketAddr,
    shutdown_tx: Option<oneshot::Sender<()>>,
}

impl TestServer {
    /// Start a test server on a random available port.
    pub async fn start() -> Self {
        Self::start_with_config(ServerConfig::default()).await
    }

    /// Start a test server with custom config.
    pub async fn start_with_config(mut config: ServerConfig) -> Self {
        // Use port 0 to get a random available port
        config.listen = vec![irc_server::config::ListenConfig {
            address: "127.0.0.1:0".parse().unwrap(),
            tls: None,
        }];

        let state = Arc::new(ServerState::new(config.clone()));
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();

        let (shutdown_tx, mut shutdown_rx) = oneshot::channel::<()>();

        // Spawn the server accept loop
        let server_state = Arc::clone(&state);
        tokio::spawn(async move {
            loop {
                tokio::select! {
                    result = listener.accept() => {
                        if let Ok((stream, client_addr)) = result {
                            let state = Arc::clone(&server_state);
                            tokio::spawn(async move {
                                let _ = irc_server::connection::handle_connection(
                                    stream,
                                    client_addr,
                                    state,
                                    false,
                                ).await;
                            });
                        }
                    }
                    _ = &mut shutdown_rx => {
                        break;
                    }
                }
            }
        });

        Self {
            addr,
            shutdown_tx: Some(shutdown_tx),
        }
    }

    /// Get the server address.
    pub fn addr(&self) -> SocketAddr {
        self.addr
    }
}

impl Drop for TestServer {
    fn drop(&mut self) {
        if let Some(tx) = self.shutdown_tx.take() {
            let _ = tx.send(());
        }
    }
}

/// A test client for connecting to the server.
pub struct TestClient {
    framed: Framed<TcpStream, MessageCodec>,
}

impl TestClient {
    /// Connect to a server.
    pub async fn connect(addr: SocketAddr) -> Self {
        let stream = TcpStream::connect(addr).await.unwrap();
        let framed = Framed::new(stream, MessageCodec::new());
        Self { framed }
    }

    /// Send a raw message.
    pub async fn send(&mut self, msg: Message) {
        self.framed.send(msg).await.unwrap();
    }

    /// Send a NICK command.
    pub async fn nick(&mut self, nickname: &str) {
        self.send(Message::new(Command::Nick {
            nickname: nickname.to_string(),
        }))
        .await;
    }

    /// Send a USER command.
    pub async fn user(&mut self, username: &str, realname: &str) {
        self.send(Message::new(Command::User {
            username: username.to_string(),
            mode: 0,
            realname: realname.to_string(),
        }))
        .await;
    }

    /// Register with NICK and USER.
    pub async fn register(&mut self, nick: &str, user: &str, realname: &str) {
        self.nick(nick).await;
        self.user(user, realname).await;
    }

    /// Receive a message (with timeout).
    pub async fn recv(&mut self) -> Option<Message> {
        match tokio::time::timeout(Duration::from_secs(5), self.framed.next()).await {
            Ok(Some(Ok(msg))) => Some(msg),
            _ => None,
        }
    }

    /// Receive all messages until a predicate is satisfied.
    pub async fn recv_until<F>(&mut self, mut pred: F) -> Vec<Message>
    where
        F: FnMut(&Message) -> bool,
    {
        let mut messages = Vec::new();
        while let Some(msg) = self.recv().await {
            let done = pred(&msg);
            messages.push(msg);
            if done {
                break;
            }
        }
        messages
    }

    /// Receive until we get a specific numeric code.
    pub async fn recv_until_numeric(&mut self, code: u16) -> Vec<Message> {
        self.recv_until(|msg| matches!(&msg.command, Command::Numeric { code: c, .. } if *c == code))
            .await
    }

    /// Send PING and wait for PONG.
    pub async fn ping(&mut self, token: &str) -> bool {
        self.send(Message::new(Command::Ping {
            server1: token.to_string(),
            server2: None,
        }))
        .await;

        if let Some(msg) = self.recv().await {
            matches!(&msg.command, Command::Pong { .. })
        } else {
            false
        }
    }

    /// Send QUIT.
    pub async fn quit(&mut self, message: Option<&str>) {
        self.send(Message::new(Command::Quit {
            message: message.map(String::from),
        }))
        .await;
    }

    /// Send JOIN.
    pub async fn join(&mut self, channel: &str) {
        self.send(Message::new(Command::Join {
            channels: vec![(channel.to_string(), None)],
        }))
        .await;
    }

    /// Send JOIN with key.
    pub async fn join_with_key(&mut self, channel: &str, key: &str) {
        self.send(Message::new(Command::Join {
            channels: vec![(channel.to_string(), Some(key.to_string()))],
        }))
        .await;
    }

    /// Send PART.
    pub async fn part(&mut self, channel: &str, message: Option<&str>) {
        self.send(Message::new(Command::Part {
            channels: vec![channel.to_string()],
            message: message.map(String::from),
        }))
        .await;
    }

    /// Send TOPIC.
    pub async fn topic(&mut self, channel: &str, topic: Option<&str>) {
        self.send(Message::new(Command::Topic {
            channel: channel.to_string(),
            topic: topic.map(String::from),
        }))
        .await;
    }

    /// Send NAMES.
    pub async fn names(&mut self, channels: Option<&[&str]>) {
        self.send(Message::new(Command::Names {
            channels: channels.map(|c| c.iter().map(|s| s.to_string()).collect()),
        }))
        .await;
    }

    /// Send LIST.
    pub async fn list(&mut self, channels: Option<&[&str]>) {
        self.send(Message::new(Command::List {
            channels: channels.map(|c| c.iter().map(|s| s.to_string()).collect()),
        }))
        .await;
    }

    /// Send MODE.
    pub async fn mode(&mut self, target: &str, modes: Option<&str>, params: &[&str]) {
        self.send(Message::new(Command::Mode {
            target: target.to_string(),
            modes: modes.map(String::from),
            params: params.iter().map(|s| s.to_string()).collect(),
        }))
        .await;
    }

    /// Send KICK.
    pub async fn kick(&mut self, channel: &str, user: &str, comment: Option<&str>) {
        self.send(Message::new(Command::Kick {
            channel: channel.to_string(),
            users: vec![user.to_string()],
            comment: comment.map(String::from),
        }))
        .await;
    }

    /// Send INVITE.
    pub async fn invite(&mut self, nickname: &str, channel: &str) {
        self.send(Message::new(Command::Invite {
            nickname: nickname.to_string(),
            channel: channel.to_string(),
        }))
        .await;
    }

    /// Send PRIVMSG.
    pub async fn privmsg(&mut self, target: &str, message: &str) {
        self.send(Message::new(Command::Privmsg {
            target: target.to_string(),
            message: message.to_string(),
        }))
        .await;
    }

    /// Send WHO.
    pub async fn who(&mut self, mask: &str) {
        self.send(Message::new(Command::Who {
            mask: mask.to_string(),
            operators_only: false,
        }))
        .await;
    }

    /// Send WHOIS.
    pub async fn whois(&mut self, nicknames: &[&str]) {
        self.send(Message::new(Command::Whois {
            server: None,
            nicknames: nicknames.iter().map(|s| s.to_string()).collect(),
        }))
        .await;
    }

    /// Send WHOWAS.
    pub async fn whowas(&mut self, nickname: &str, count: Option<u32>) {
        self.send(Message::new(Command::Whowas {
            nickname: nickname.to_string(),
            count,
            server: None,
        }))
        .await;
    }

    /// Send MOTD.
    pub async fn motd(&mut self) {
        self.send(Message::new(Command::Motd { server: None }))
            .await;
    }

    /// Send LUSERS.
    pub async fn lusers(&mut self) {
        self.send(Message::new(Command::Lusers {
            mask: None,
            server: None,
        }))
        .await;
    }

    /// Send VERSION.
    pub async fn version(&mut self) {
        self.send(Message::new(Command::Version { server: None }))
            .await;
    }

    /// Send TIME.
    pub async fn time(&mut self) {
        self.send(Message::new(Command::Time { server: None }))
            .await;
    }

    /// Send ADMIN.
    pub async fn admin(&mut self) {
        self.send(Message::new(Command::Admin { server: None }))
            .await;
    }

    /// Send INFO.
    pub async fn info(&mut self) {
        self.send(Message::new(Command::Info { server: None }))
            .await;
    }

    /// Send STATS.
    pub async fn stats(&mut self, query: Option<char>) {
        self.send(Message::new(Command::Stats {
            query,
            server: None,
        }))
        .await;
    }

    /// Send OPER.
    pub async fn oper(&mut self, name: &str, password: &str) {
        self.send(Message::new(Command::Oper {
            name: name.to_string(),
            password: password.to_string(),
        }))
        .await;
    }

    /// Send KILL.
    pub async fn kill(&mut self, nickname: &str, comment: &str) {
        self.send(Message::new(Command::Kill {
            nickname: nickname.to_string(),
            comment: comment.to_string(),
        }))
        .await;
    }

    /// Send WALLOPS.
    pub async fn wallops(&mut self, message: &str) {
        self.send(Message::new(Command::Wallops {
            message: message.to_string(),
        }))
        .await;
    }
}
