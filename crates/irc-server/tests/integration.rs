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

// ============================================================
// Channel operation tests
// ============================================================

#[tokio::test]
async fn test_join_creates_channel() {
    let server = TestServer::start().await;
    let mut client = TestClient::connect(server.addr()).await;

    // Register
    client.register("alice", "alice", "Alice").await;
    client.recv_until_numeric(errors::ERR_NOMOTD).await;

    // Join a channel
    client.join("#test").await;

    // Should receive JOIN, topic info (331 = no topic), and names list
    let messages = client.recv_until_numeric(replies::RPL_ENDOFNAMES).await;

    // Check for JOIN message
    assert!(
        messages.iter().any(|m| matches!(&m.command, Command::Join { channels } if channels.iter().any(|(c, _)| c == "#test"))),
        "Should receive JOIN message"
    );

    // Check for names reply (353) with @alice (first user gets ops)
    assert!(
        messages.iter().any(|m| matches!(
            &m.command,
            Command::Numeric { code, params, .. } if *code == replies::RPL_NAMREPLY && params.iter().any(|p| p.contains("@alice"))
        )),
        "Should receive NAMES with @alice (op)"
    );
}

#[tokio::test]
async fn test_join_existing_channel() {
    let server = TestServer::start().await;

    // First client creates the channel
    let mut alice = TestClient::connect(server.addr()).await;
    alice.register("alice", "alice", "Alice").await;
    alice.recv_until_numeric(errors::ERR_NOMOTD).await;
    alice.join("#test").await;
    alice.recv_until_numeric(replies::RPL_ENDOFNAMES).await;

    // Second client joins
    let mut bob = TestClient::connect(server.addr()).await;
    bob.register("bob", "bob", "Bob").await;
    bob.recv_until_numeric(errors::ERR_NOMOTD).await;
    bob.join("#test").await;

    // Bob should receive JOIN and names
    let bob_msgs = bob.recv_until_numeric(replies::RPL_ENDOFNAMES).await;
    assert!(
        bob_msgs.iter().any(|m| matches!(&m.command, Command::Join { .. })),
        "Bob should receive JOIN"
    );

    // Alice should see Bob's JOIN
    if let Some(msg) = alice.recv().await {
        match &msg.command {
            Command::Join { channels } => {
                assert!(channels.iter().any(|(c, _)| c == "#test"));
                assert_eq!(msg.prefix.as_ref().unwrap().nick(), Some("bob"));
            }
            _ => panic!("Expected JOIN from bob, got {:?}", msg.command),
        }
    }
}

#[tokio::test]
async fn test_join_with_key() {
    let server = TestServer::start().await;

    // Alice creates channel with key
    let mut alice = TestClient::connect(server.addr()).await;
    alice.register("alice", "alice", "Alice").await;
    alice.recv_until_numeric(errors::ERR_NOMOTD).await;
    alice.join("#secret").await;
    alice.recv_until_numeric(replies::RPL_ENDOFNAMES).await;

    // Set key
    alice.mode("#secret", Some("+k"), &["password123"]).await;
    alice.recv().await; // MODE reply

    // Bob tries without key
    let mut bob = TestClient::connect(server.addr()).await;
    bob.register("bob", "bob", "Bob").await;
    bob.recv_until_numeric(errors::ERR_NOMOTD).await;
    bob.join("#secret").await;

    // Should get ERR_BADCHANNELKEY (475)
    if let Some(msg) = bob.recv().await {
        assert!(
            matches!(&msg.command, Command::Numeric { code, .. } if *code == errors::ERR_BADCHANNELKEY),
            "Should get bad key error, got {:?}",
            msg.command
        );
    }

    // Bob tries with correct key
    bob.join_with_key("#secret", "password123").await;
    let msgs = bob.recv_until_numeric(replies::RPL_ENDOFNAMES).await;
    assert!(
        msgs.iter().any(|m| matches!(&m.command, Command::Join { .. })),
        "Bob should join with correct key"
    );
}

#[tokio::test]
async fn test_join_invite_only() {
    let server = TestServer::start().await;

    // Alice creates invite-only channel
    let mut alice = TestClient::connect(server.addr()).await;
    alice.register("alice", "alice", "Alice").await;
    alice.recv_until_numeric(errors::ERR_NOMOTD).await;
    alice.join("#private").await;
    alice.recv_until_numeric(replies::RPL_ENDOFNAMES).await;
    alice.mode("#private", Some("+i"), &[]).await;
    alice.recv().await;

    // Bob tries to join without invite
    let mut bob = TestClient::connect(server.addr()).await;
    bob.register("bob", "bob", "Bob").await;
    bob.recv_until_numeric(errors::ERR_NOMOTD).await;
    bob.join("#private").await;

    // Should get ERR_INVITEONLYCHAN (473)
    if let Some(msg) = bob.recv().await {
        assert!(
            matches!(&msg.command, Command::Numeric { code, .. } if *code == errors::ERR_INVITEONLYCHAN),
            "Should get invite only error, got {:?}",
            msg.command
        );
    }

    // Alice invites Bob
    alice.invite("bob", "#private").await;
    alice.recv().await; // RPL_INVITING
    bob.recv().await; // INVITE message

    // Now Bob can join
    bob.join("#private").await;
    let msgs = bob.recv_until_numeric(replies::RPL_ENDOFNAMES).await;
    assert!(
        msgs.iter().any(|m| matches!(&m.command, Command::Join { .. })),
        "Bob should join after invite"
    );
}

#[tokio::test]
async fn test_join_banned() {
    let server = TestServer::start().await;

    // Alice creates channel and sets ban
    let mut alice = TestClient::connect(server.addr()).await;
    alice.register("alice", "alice", "Alice").await;
    alice.recv_until_numeric(errors::ERR_NOMOTD).await;
    alice.join("#test").await;
    alice.recv_until_numeric(replies::RPL_ENDOFNAMES).await;
    alice.mode("#test", Some("+b"), &["bob!*@*"]).await;
    alice.recv().await;

    // Bob tries to join
    let mut bob = TestClient::connect(server.addr()).await;
    bob.register("bob", "bob", "Bob").await;
    bob.recv_until_numeric(errors::ERR_NOMOTD).await;
    bob.join("#test").await;

    // Should get ERR_BANNEDFROMCHAN (474)
    if let Some(msg) = bob.recv().await {
        assert!(
            matches!(&msg.command, Command::Numeric { code, .. } if *code == errors::ERR_BANNEDFROMCHAN),
            "Should get banned error, got {:?}",
            msg.command
        );
    }

    // Alice adds exception for bob
    alice.mode("#test", Some("+e"), &["bob!*@*"]).await;
    alice.recv().await;

    // Now Bob can join
    bob.join("#test").await;
    let msgs = bob.recv_until_numeric(replies::RPL_ENDOFNAMES).await;
    assert!(
        msgs.iter().any(|m| matches!(&m.command, Command::Join { .. })),
        "Bob should join with exception"
    );
}

#[tokio::test]
async fn test_part_channel() {
    let server = TestServer::start().await;

    let mut alice = TestClient::connect(server.addr()).await;
    alice.register("alice", "alice", "Alice").await;
    alice.recv_until_numeric(errors::ERR_NOMOTD).await;

    let mut bob = TestClient::connect(server.addr()).await;
    bob.register("bob", "bob", "Bob").await;
    bob.recv_until_numeric(errors::ERR_NOMOTD).await;

    // Both join
    alice.join("#test").await;
    alice.recv_until_numeric(replies::RPL_ENDOFNAMES).await;
    bob.join("#test").await;
    bob.recv_until_numeric(replies::RPL_ENDOFNAMES).await;
    alice.recv().await; // Bob's JOIN

    // Alice parts
    alice.part("#test", Some("Goodbye!")).await;

    // Alice should receive PART
    if let Some(msg) = alice.recv().await {
        match &msg.command {
            Command::Part { channels, message } => {
                assert!(channels.contains(&"#test".to_string()));
                assert_eq!(message.as_deref(), Some("Goodbye!"));
            }
            _ => panic!("Expected PART, got {:?}", msg.command),
        }
    }

    // Bob should see PART
    if let Some(msg) = bob.recv().await {
        match &msg.command {
            Command::Part { channels, .. } => {
                assert!(channels.contains(&"#test".to_string()));
                assert_eq!(msg.prefix.as_ref().unwrap().nick(), Some("alice"));
            }
            _ => panic!("Expected PART from alice, got {:?}", msg.command),
        }
    }
}

#[tokio::test]
async fn test_topic_get_set() {
    let server = TestServer::start().await;
    let mut client = TestClient::connect(server.addr()).await;

    client.register("alice", "alice", "Alice").await;
    client.recv_until_numeric(errors::ERR_NOMOTD).await;
    client.join("#test").await;
    client.recv_until_numeric(replies::RPL_ENDOFNAMES).await;

    // Query topic (should be no topic)
    client.topic("#test", None).await;
    if let Some(msg) = client.recv().await {
        assert!(
            matches!(&msg.command, Command::Numeric { code, .. } if *code == replies::RPL_NOTOPIC),
            "Should get no topic, got {:?}",
            msg.command
        );
    }

    // Set topic
    client.topic("#test", Some("Hello World!")).await;
    if let Some(msg) = client.recv().await {
        match &msg.command {
            Command::Topic { channel, topic } => {
                assert_eq!(channel, "#test");
                assert_eq!(topic.as_deref(), Some("Hello World!"));
            }
            _ => panic!("Expected TOPIC, got {:?}", msg.command),
        }
    }

    // Query topic again
    client.topic("#test", None).await;
    if let Some(msg) = client.recv().await {
        assert!(
            matches!(&msg.command, Command::Numeric { code, params, .. } if *code == replies::RPL_TOPIC && params.iter().any(|p| p == "Hello World!")),
            "Should get topic, got {:?}",
            msg.command
        );
    }
}

#[tokio::test]
async fn test_topic_locked() {
    let server = TestServer::start().await;

    let mut alice = TestClient::connect(server.addr()).await;
    alice.register("alice", "alice", "Alice").await;
    alice.recv_until_numeric(errors::ERR_NOMOTD).await;
    alice.join("#test").await;
    alice.recv_until_numeric(replies::RPL_ENDOFNAMES).await;

    // Set +t mode
    alice.mode("#test", Some("+t"), &[]).await;
    alice.recv().await;

    // Bob joins (non-op)
    let mut bob = TestClient::connect(server.addr()).await;
    bob.register("bob", "bob", "Bob").await;
    bob.recv_until_numeric(errors::ERR_NOMOTD).await;
    bob.join("#test").await;
    bob.recv_until_numeric(replies::RPL_ENDOFNAMES).await;
    alice.recv().await; // Bob's JOIN

    // Bob tries to set topic
    bob.topic("#test", Some("Bob's topic")).await;
    if let Some(msg) = bob.recv().await {
        assert!(
            matches!(&msg.command, Command::Numeric { code, .. } if *code == errors::ERR_CHANOPRIVSNEEDED),
            "Bob should not be able to set topic, got {:?}",
            msg.command
        );
    }
}

#[tokio::test]
async fn test_names_list() {
    let server = TestServer::start().await;

    let mut alice = TestClient::connect(server.addr()).await;
    alice.register("alice", "alice", "Alice").await;
    alice.recv_until_numeric(errors::ERR_NOMOTD).await;
    alice.join("#test").await;
    alice.recv_until_numeric(replies::RPL_ENDOFNAMES).await;

    let mut bob = TestClient::connect(server.addr()).await;
    bob.register("bob", "bob", "Bob").await;
    bob.recv_until_numeric(errors::ERR_NOMOTD).await;
    bob.join("#test").await;
    bob.recv_until_numeric(replies::RPL_ENDOFNAMES).await;
    alice.recv().await; // Bob's JOIN

    // Give bob voice
    alice.mode("#test", Some("+v"), &["bob"]).await;
    alice.recv().await; // MODE
    bob.recv().await; // MODE

    // Request names
    alice.names(Some(&["#test"])).await;
    let msgs = alice.recv_until_numeric(replies::RPL_ENDOFNAMES).await;

    // Check names reply has @alice and +bob
    let names_msg = msgs.iter().find(|m| {
        matches!(&m.command, Command::Numeric { code, .. } if *code == replies::RPL_NAMREPLY)
    });
    assert!(names_msg.is_some(), "Should have names reply");
    if let Some(msg) = names_msg {
        if let Command::Numeric { params, .. } = &msg.command {
            let names = params.last().unwrap();
            assert!(names.contains("@alice"), "Should have @alice");
            assert!(names.contains("+bob"), "Should have +bob");
        }
    }
}

#[tokio::test]
async fn test_list_channels() {
    let server = TestServer::start().await;

    let mut client = TestClient::connect(server.addr()).await;
    client.register("alice", "alice", "Alice").await;
    client.recv_until_numeric(errors::ERR_NOMOTD).await;

    // Create some channels
    client.join("#test1").await;
    client.recv_until_numeric(replies::RPL_ENDOFNAMES).await;
    client.join("#test2").await;
    client.recv_until_numeric(replies::RPL_ENDOFNAMES).await;

    // Set topic on one
    client.topic("#test1", Some("Channel 1")).await;
    client.recv().await;

    // Request LIST
    client.list(None).await;
    let msgs = client.recv_until_numeric(replies::RPL_LISTEND).await;

    // Should have RPL_LIST (322) for each channel
    let list_entries: Vec<_> = msgs
        .iter()
        .filter(|m| matches!(&m.command, Command::Numeric { code, .. } if *code == replies::RPL_LIST))
        .collect();

    assert!(list_entries.len() >= 2, "Should list at least 2 channels");
}

#[tokio::test]
async fn test_channel_modes() {
    let server = TestServer::start().await;

    let mut client = TestClient::connect(server.addr()).await;
    client.register("alice", "alice", "Alice").await;
    client.recv_until_numeric(errors::ERR_NOMOTD).await;
    client.join("#test").await;
    client.recv_until_numeric(replies::RPL_ENDOFNAMES).await;

    // Set multiple modes
    client.mode("#test", Some("+nt"), &[]).await;
    client.recv().await; // MODE broadcast

    // Query modes
    client.mode("#test", None, &[]).await;
    if let Some(msg) = client.recv().await {
        match &msg.command {
            Command::Numeric { code, params, .. } => {
                assert_eq!(*code, replies::RPL_CHANNELMODEIS);
                let mode_str = &params[1];
                assert!(mode_str.contains('n'), "Should have +n");
                assert!(mode_str.contains('t'), "Should have +t");
            }
            _ => panic!("Expected RPL_CHANNELMODEIS, got {:?}", msg.command),
        }
    }

    // Set limit
    client.mode("#test", Some("+l"), &["50"]).await;
    client.recv().await;

    // Query again
    client.mode("#test", None, &[]).await;
    if let Some(msg) = client.recv().await {
        if let Command::Numeric { params, .. } = &msg.command {
            let mode_str = &params[1];
            assert!(mode_str.contains('l'), "Should have +l");
        }
    }
}

#[tokio::test]
async fn test_kick_user() {
    let server = TestServer::start().await;

    let mut alice = TestClient::connect(server.addr()).await;
    alice.register("alice", "alice", "Alice").await;
    alice.recv_until_numeric(errors::ERR_NOMOTD).await;
    alice.join("#test").await;
    alice.recv_until_numeric(replies::RPL_ENDOFNAMES).await;

    let mut bob = TestClient::connect(server.addr()).await;
    bob.register("bob", "bob", "Bob").await;
    bob.recv_until_numeric(errors::ERR_NOMOTD).await;
    bob.join("#test").await;
    bob.recv_until_numeric(replies::RPL_ENDOFNAMES).await;
    alice.recv().await; // Bob's JOIN

    // Alice kicks Bob
    alice.kick("#test", "bob", Some("Bye!")).await;

    // Both should see KICK
    if let Some(msg) = alice.recv().await {
        match &msg.command {
            Command::Kick { channel, users, comment } => {
                assert_eq!(channel, "#test");
                assert!(users.contains(&"bob".to_string()));
                assert_eq!(comment.as_deref(), Some("Bye!"));
            }
            _ => panic!("Expected KICK, got {:?}", msg.command),
        }
    }

    if let Some(msg) = bob.recv().await {
        assert!(
            matches!(&msg.command, Command::Kick { .. }),
            "Bob should see KICK"
        );
    }
}

#[tokio::test]
async fn test_kick_requires_op() {
    let server = TestServer::start().await;

    let mut alice = TestClient::connect(server.addr()).await;
    alice.register("alice", "alice", "Alice").await;
    alice.recv_until_numeric(errors::ERR_NOMOTD).await;
    alice.join("#test").await;
    alice.recv_until_numeric(replies::RPL_ENDOFNAMES).await;

    let mut bob = TestClient::connect(server.addr()).await;
    bob.register("bob", "bob", "Bob").await;
    bob.recv_until_numeric(errors::ERR_NOMOTD).await;
    bob.join("#test").await;
    bob.recv_until_numeric(replies::RPL_ENDOFNAMES).await;
    alice.recv().await;

    // Bob tries to kick Alice (Bob is not op)
    bob.kick("#test", "alice", None).await;
    if let Some(msg) = bob.recv().await {
        assert!(
            matches!(&msg.command, Command::Numeric { code, .. } if *code == errors::ERR_CHANOPRIVSNEEDED),
            "Bob should not be able to kick, got {:?}",
            msg.command
        );
    }
}

#[tokio::test]
async fn test_invite_user() {
    let server = TestServer::start().await;

    let mut alice = TestClient::connect(server.addr()).await;
    alice.register("alice", "alice", "Alice").await;
    alice.recv_until_numeric(errors::ERR_NOMOTD).await;
    alice.join("#test").await;
    alice.recv_until_numeric(replies::RPL_ENDOFNAMES).await;

    let mut bob = TestClient::connect(server.addr()).await;
    bob.register("bob", "bob", "Bob").await;
    bob.recv_until_numeric(errors::ERR_NOMOTD).await;

    // Alice invites Bob
    alice.invite("bob", "#test").await;

    // Alice gets RPL_INVITING
    if let Some(msg) = alice.recv().await {
        assert!(
            matches!(&msg.command, Command::Numeric { code, .. } if *code == replies::RPL_INVITING),
            "Should get RPL_INVITING, got {:?}",
            msg.command
        );
    }

    // Bob gets INVITE
    if let Some(msg) = bob.recv().await {
        match &msg.command {
            Command::Invite { nickname, channel } => {
                assert_eq!(nickname, "bob");
                assert_eq!(channel, "#test");
            }
            _ => panic!("Expected INVITE, got {:?}", msg.command),
        }
    }
}

#[tokio::test]
async fn test_privmsg_to_channel() {
    let server = TestServer::start().await;

    let mut alice = TestClient::connect(server.addr()).await;
    alice.register("alice", "alice", "Alice").await;
    alice.recv_until_numeric(errors::ERR_NOMOTD).await;

    let mut bob = TestClient::connect(server.addr()).await;
    bob.register("bob", "bob", "Bob").await;
    bob.recv_until_numeric(errors::ERR_NOMOTD).await;

    // Both join
    alice.join("#test").await;
    alice.recv_until_numeric(replies::RPL_ENDOFNAMES).await;
    bob.join("#test").await;
    bob.recv_until_numeric(replies::RPL_ENDOFNAMES).await;
    alice.recv().await; // Bob's JOIN

    // Alice sends message to channel
    alice.privmsg("#test", "Hello channel!").await;

    // Bob should receive it
    if let Some(msg) = bob.recv().await {
        match &msg.command {
            Command::Privmsg { target, message } => {
                assert_eq!(target, "#test");
                assert_eq!(message, "Hello channel!");
                assert_eq!(msg.prefix.as_ref().unwrap().nick(), Some("alice"));
            }
            _ => panic!("Expected PRIVMSG, got {:?}", msg.command),
        }
    }
}

#[tokio::test]
async fn test_privmsg_moderated_channel() {
    let server = TestServer::start().await;

    let mut alice = TestClient::connect(server.addr()).await;
    alice.register("alice", "alice", "Alice").await;
    alice.recv_until_numeric(errors::ERR_NOMOTD).await;
    alice.join("#test").await;
    alice.recv_until_numeric(replies::RPL_ENDOFNAMES).await;

    let mut bob = TestClient::connect(server.addr()).await;
    bob.register("bob", "bob", "Bob").await;
    bob.recv_until_numeric(errors::ERR_NOMOTD).await;
    bob.join("#test").await;
    bob.recv_until_numeric(replies::RPL_ENDOFNAMES).await;
    alice.recv().await;

    // Set +m
    alice.mode("#test", Some("+m"), &[]).await;
    alice.recv().await;
    bob.recv().await;

    // Bob (non-voiced) tries to speak
    bob.privmsg("#test", "Can you hear me?").await;
    if let Some(msg) = bob.recv().await {
        assert!(
            matches!(&msg.command, Command::Numeric { code, .. } if *code == errors::ERR_CANNOTSENDTOCHAN),
            "Bob should not be able to speak, got {:?}",
            msg.command
        );
    }

    // Give Bob voice
    alice.mode("#test", Some("+v"), &["bob"]).await;
    alice.recv().await;
    bob.recv().await;

    // Now Bob can speak
    bob.privmsg("#test", "Now I can speak!").await;
    if let Some(msg) = alice.recv().await {
        assert!(
            matches!(&msg.command, Command::Privmsg { message, .. } if message == "Now I can speak!"),
            "Alice should receive Bob's message, got {:?}",
            msg.command
        );
    }
}

#[tokio::test]
async fn test_quit_broadcasts_to_channels() {
    let server = TestServer::start().await;

    let mut alice = TestClient::connect(server.addr()).await;
    alice.register("alice", "alice", "Alice").await;
    alice.recv_until_numeric(errors::ERR_NOMOTD).await;

    let mut bob = TestClient::connect(server.addr()).await;
    bob.register("bob", "bob", "Bob").await;
    bob.recv_until_numeric(errors::ERR_NOMOTD).await;

    // Both join same channel
    alice.join("#test").await;
    alice.recv_until_numeric(replies::RPL_ENDOFNAMES).await;
    bob.join("#test").await;
    bob.recv_until_numeric(replies::RPL_ENDOFNAMES).await;
    alice.recv().await; // Bob's JOIN

    // Bob quits
    bob.quit(Some("Leaving")).await;

    // Alice should see QUIT
    if let Some(msg) = alice.recv().await {
        match &msg.command {
            Command::Quit { message } => {
                assert_eq!(message.as_deref(), Some("Leaving"));
                assert_eq!(msg.prefix.as_ref().unwrap().nick(), Some("bob"));
            }
            _ => panic!("Expected QUIT, got {:?}", msg.command),
        }
    }
}
