//! Integration tests for the IRC server.

mod common;

use irc_proto::{errors, replies, Command};

use common::{TestClient, TestServer};

#[tokio::test]
async fn test_basic_registration() {
    let server = TestServer::start().await;
    let mut client = TestClient::connect(server.addr()).await;

    // Register with NICK and USER
    client.register("testuser", "testuser", "Test User").await;

    // Wait for welcome messages (001-005)
    let messages = client.recv_until_numeric(replies::RPL_ISUPPORT).await;

    // Should have received RPL_WELCOME (001)
    assert!(
        messages.iter().any(|m| matches!(
            &m.command,
            Command::Numeric { code, params, .. } if *code == replies::RPL_WELCOME && params[0].contains("testuser")
        )),
        "Should receive welcome message with nick"
    );

    // Should have received RPL_YOURHOST (002)
    assert!(
        messages
            .iter()
            .any(|m| matches!(&m.command, Command::Numeric { code, .. } if *code == replies::RPL_YOURHOST)),
        "Should receive yourhost message"
    );

    // Should have received RPL_CREATED (003)
    assert!(
        messages
            .iter()
            .any(|m| matches!(&m.command, Command::Numeric { code, .. } if *code == replies::RPL_CREATED)),
        "Should receive created message"
    );

    // Should have received RPL_MYINFO (004)
    assert!(
        messages
            .iter()
            .any(|m| matches!(&m.command, Command::Numeric { code, .. } if *code == replies::RPL_MYINFO)),
        "Should receive myinfo message"
    );
}

#[tokio::test]
async fn test_nick_collision() {
    let server = TestServer::start().await;

    // First client registers
    let mut client1 = TestClient::connect(server.addr()).await;
    client1.register("testnick", "user1", "User 1").await;
    client1.recv_until_numeric(replies::RPL_WELCOME).await;

    // Second client tries the same nick
    let mut client2 = TestClient::connect(server.addr()).await;
    client2.nick("testnick").await;

    // Should get ERR_NICKNAMEINUSE
    let messages = client2.recv_until_numeric(errors::ERR_NICKNAMEINUSE).await;
    assert!(
        messages.iter().any(|m| matches!(
            &m.command,
            Command::Numeric { code, .. } if *code == errors::ERR_NICKNAMEINUSE
        )),
        "Should receive nick in use error"
    );
}

#[tokio::test]
async fn test_ping_pong() {
    let server = TestServer::start().await;
    let mut client = TestClient::connect(server.addr()).await;

    // PING/PONG works without registration
    assert!(client.ping("test123").await, "Should receive PONG response");
}

#[tokio::test]
async fn test_privmsg_to_user() {
    let server = TestServer::start().await;

    // Register two clients
    let mut client1 = TestClient::connect(server.addr()).await;
    client1.register("alice", "alice", "Alice").await;
    client1.recv_until_numeric(errors::ERR_NOMOTD).await;

    let mut client2 = TestClient::connect(server.addr()).await;
    client2.register("bob", "bob", "Bob").await;
    client2.recv_until_numeric(errors::ERR_NOMOTD).await;

    // Alice sends a message to Bob
    client1
        .send(irc_proto::Message::new(Command::Privmsg {
            target: "bob".to_string(),
            message: "Hello Bob!".to_string(),
        }))
        .await;

    // Bob should receive the message
    if let Some(msg) = client2.recv().await {
        match &msg.command {
            Command::Privmsg { target, message } => {
                assert_eq!(target, "bob");
                assert_eq!(message, "Hello Bob!");
                assert!(msg.prefix.is_some());
                assert_eq!(msg.prefix.as_ref().unwrap().nick(), Some("alice"));
            }
            _ => panic!("Expected PRIVMSG, got {:?}", msg.command),
        }
    } else {
        panic!("Bob did not receive the message");
    }
}

#[tokio::test]
async fn test_privmsg_no_such_nick() {
    let server = TestServer::start().await;
    let mut client = TestClient::connect(server.addr()).await;

    // Register
    client.register("testuser", "testuser", "Test").await;
    client.recv_until_numeric(errors::ERR_NOMOTD).await;

    // Try to message a non-existent user
    client
        .send(irc_proto::Message::new(Command::Privmsg {
            target: "nobody".to_string(),
            message: "Hello?".to_string(),
        }))
        .await;

    // Should get ERR_NOSUCHNICK
    if let Some(msg) = client.recv().await {
        match &msg.command {
            Command::Numeric { code, .. } => {
                assert_eq!(*code, errors::ERR_NOSUCHNICK);
            }
            _ => panic!("Expected ERR_NOSUCHNICK, got {:?}", msg.command),
        }
    } else {
        panic!("Did not receive error reply");
    }
}

#[tokio::test]
async fn test_away_message() {
    let server = TestServer::start().await;

    // Register two clients
    let mut client1 = TestClient::connect(server.addr()).await;
    client1.register("alice", "alice", "Alice").await;
    client1.recv_until_numeric(errors::ERR_NOMOTD).await;

    let mut client2 = TestClient::connect(server.addr()).await;
    client2.register("bob", "bob", "Bob").await;
    client2.recv_until_numeric(errors::ERR_NOMOTD).await;

    // Bob sets away
    client2
        .send(irc_proto::Message::new(Command::Away {
            message: Some("Gone fishing".to_string()),
        }))
        .await;

    // Bob should get RPL_NOWAWAY
    if let Some(msg) = client2.recv().await {
        assert!(matches!(
            &msg.command,
            Command::Numeric { code, .. } if *code == replies::RPL_NOWAWAY
        ));
    }

    // Alice sends a message to Bob
    client1
        .send(irc_proto::Message::new(Command::Privmsg {
            target: "bob".to_string(),
            message: "Are you there?".to_string(),
        }))
        .await;

    // Alice should get RPL_AWAY
    if let Some(msg) = client1.recv().await {
        match &msg.command {
            Command::Numeric { code, params, .. } => {
                assert_eq!(*code, replies::RPL_AWAY);
                assert!(params.iter().any(|p| p.contains("Gone fishing")));
            }
            _ => panic!("Expected RPL_AWAY, got {:?}", msg.command),
        }
    }
}

#[tokio::test]
async fn test_nick_change() {
    let server = TestServer::start().await;
    let mut client = TestClient::connect(server.addr()).await;

    // Register
    client.register("oldnick", "user", "Test User").await;
    client.recv_until_numeric(errors::ERR_NOMOTD).await;

    // Change nick
    client.nick("newnick").await;

    // Should get NICK message back
    if let Some(msg) = client.recv().await {
        match &msg.command {
            Command::Nick { nickname } => {
                assert_eq!(nickname, "newnick");
            }
            _ => panic!("Expected NICK, got {:?}", msg.command),
        }
    }
}

#[tokio::test]
async fn test_invalid_nickname() {
    let server = TestServer::start().await;
    let mut client = TestClient::connect(server.addr()).await;

    // Try invalid nick (starts with digit)
    client.nick("123invalid").await;

    // Should get ERR_ERRONEUSNICKNAME
    if let Some(msg) = client.recv().await {
        assert!(matches!(
            &msg.command,
            Command::Numeric { code, .. } if *code == errors::ERR_ERRONEUSNICKNAME
        ));
    }
}

#[tokio::test]
async fn test_commands_require_registration() {
    let server = TestServer::start().await;
    let mut client = TestClient::connect(server.addr()).await;

    // Try to send PRIVMSG without registering
    client
        .send(irc_proto::Message::new(Command::Privmsg {
            target: "someone".to_string(),
            message: "Hello".to_string(),
        }))
        .await;

    // Should get ERR_NOTREGISTERED
    if let Some(msg) = client.recv().await {
        assert!(matches!(
            &msg.command,
            Command::Numeric { code, .. } if *code == errors::ERR_NOTREGISTERED
        ));
    }
}

#[tokio::test]
async fn test_quit() {
    let server = TestServer::start().await;
    let mut client = TestClient::connect(server.addr()).await;

    // Register
    client.register("testuser", "testuser", "Test").await;
    client.recv_until_numeric(errors::ERR_NOMOTD).await;

    // Send QUIT
    client.quit(Some("Goodbye!")).await;

    // Should get ERROR message
    if let Some(msg) = client.recv().await {
        match &msg.command {
            Command::Unknown { command, params } => {
                assert_eq!(command, "ERROR");
                assert!(params[0].contains("Goodbye!"));
            }
            _ => panic!("Expected ERROR, got {:?}", msg.command),
        }
    }
}
