//! Message history storage for CHATHISTORY support.

use chrono::{DateTime, TimeZone, Utc};
use irc_proto::{Command, Message, Prefix};
use rusqlite::OptionalExtension;

use super::PooledConnection;
use crate::error::{Error, Result};

/// Target type for history messages.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TargetType {
    Channel,
    User,
}

impl TargetType {
    pub fn as_str(&self) -> &'static str {
        match self {
            TargetType::Channel => "channel",
            TargetType::User => "user",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "channel" => Some(TargetType::Channel),
            "user" => Some(TargetType::User),
            _ => None,
        }
    }
}

/// Sender information for a history message.
#[derive(Debug, Clone)]
pub struct HistorySender {
    pub account: Option<String>,
    pub nick: String,
    pub user: String,
    pub host: String,
}

impl HistorySender {
    pub fn prefix(&self) -> Prefix {
        Prefix::from_user(self.nick.clone(), self.user.clone(), self.host.clone())
    }
}

/// A stored message for history replay.
#[derive(Debug, Clone)]
pub struct HistoryMessage {
    pub id: i64,
    pub msgid: String,
    pub timestamp: DateTime<Utc>,
    pub sender: HistorySender,
    pub target: String,
    pub target_type: TargetType,
    pub command: String, // "PRIVMSG" or "NOTICE"
    pub message: String,
}

impl HistoryMessage {
    /// Convert to an IRC message for replay.
    pub fn to_irc_message(&self) -> Message {
        let command = if self.command == "NOTICE" {
            Command::Notice {
                target: self.target.clone(),
                message: self.message.clone(),
            }
        } else {
            Command::Privmsg {
                target: self.target.clone(),
                message: self.message.clone(),
            }
        };

        let mut msg = Message::with_prefix(self.sender.prefix(), command);

        // Add time and msgid tags
        let mut tags = irc_proto::Tags::new();
        tags.set("time", self.timestamp.to_rfc3339_opts(chrono::SecondsFormat::Millis, true));
        tags.set("msgid", &self.msgid);
        if let Some(ref account) = self.sender.account {
            tags.set("account", account);
        }
        msg.tags = Some(tags);

        msg
    }
}

/// Store a message in history.
pub fn store_message(conn: &PooledConnection, msg: &HistoryMessage) -> Result<()> {
    conn.execute(
        "INSERT INTO message_history
         (msgid, timestamp, sender_account, sender_nick, sender_user, sender_host,
          target, target_type, command, message)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
        rusqlite::params![
            msg.msgid,
            msg.timestamp.timestamp_millis(),
            msg.sender.account,
            msg.sender.nick,
            msg.sender.user,
            msg.sender.host,
            msg.target,
            msg.target_type.as_str(),
            msg.command,
            msg.message,
        ],
    )
    .map_err(|e| Error::Database(format!("Failed to store message: {}", e)))?;

    Ok(())
}

/// Get the latest messages for a target.
pub fn get_latest(conn: &PooledConnection, target: &str, limit: usize) -> Result<Vec<HistoryMessage>> {
    let mut stmt = conn
        .prepare(
            "SELECT id, msgid, timestamp, sender_account, sender_nick, sender_user,
                    sender_host, target, target_type, command, message
             FROM message_history
             WHERE target = ?1 COLLATE NOCASE
             ORDER BY timestamp DESC
             LIMIT ?2",
        )
        .map_err(|e| Error::Database(format!("Failed to prepare statement: {}", e)))?;

    let messages = stmt
        .query_map(rusqlite::params![target, limit as i64], row_to_message)
        .map_err(|e| Error::Database(format!("Failed to get latest messages: {}", e)))?
        .collect::<std::result::Result<Vec<_>, _>>()
        .map_err(|e| Error::Database(format!("Failed to collect messages: {}", e)))?;

    // Reverse to get chronological order
    let mut messages = messages;
    messages.reverse();
    Ok(messages)
}

/// Get messages before a specific msgid.
pub fn get_before(
    conn: &PooledConnection,
    target: &str,
    before_msgid: &str,
    limit: usize,
) -> Result<Vec<HistoryMessage>> {
    // First get the timestamp of the reference message
    let reference_ts: Option<i64> = conn
        .query_row(
            "SELECT timestamp FROM message_history WHERE msgid = ?1",
            rusqlite::params![before_msgid],
            |row| row.get(0),
        )
        .optional()
        .map_err(|e| Error::Database(format!("Failed to find reference message: {}", e)))?;

    let Some(reference_ts) = reference_ts else {
        return Ok(Vec::new());
    };

    let mut stmt = conn
        .prepare(
            "SELECT id, msgid, timestamp, sender_account, sender_nick, sender_user,
                    sender_host, target, target_type, command, message
             FROM message_history
             WHERE target = ?1 COLLATE NOCASE AND timestamp < ?2
             ORDER BY timestamp DESC
             LIMIT ?3",
        )
        .map_err(|e| Error::Database(format!("Failed to prepare statement: {}", e)))?;

    let messages = stmt
        .query_map(rusqlite::params![target, reference_ts, limit as i64], row_to_message)
        .map_err(|e| Error::Database(format!("Failed to get messages: {}", e)))?
        .collect::<std::result::Result<Vec<_>, _>>()
        .map_err(|e| Error::Database(format!("Failed to collect messages: {}", e)))?;

    let mut messages = messages;
    messages.reverse();
    Ok(messages)
}

/// Get messages after a specific msgid.
pub fn get_after(
    conn: &PooledConnection,
    target: &str,
    after_msgid: &str,
    limit: usize,
) -> Result<Vec<HistoryMessage>> {
    let reference_ts: Option<i64> = conn
        .query_row(
            "SELECT timestamp FROM message_history WHERE msgid = ?1",
            rusqlite::params![after_msgid],
            |row| row.get(0),
        )
        .optional()
        .map_err(|e| Error::Database(format!("Failed to find reference message: {}", e)))?;

    let Some(reference_ts) = reference_ts else {
        return Ok(Vec::new());
    };

    let mut stmt = conn
        .prepare(
            "SELECT id, msgid, timestamp, sender_account, sender_nick, sender_user,
                    sender_host, target, target_type, command, message
             FROM message_history
             WHERE target = ?1 COLLATE NOCASE AND timestamp > ?2
             ORDER BY timestamp ASC
             LIMIT ?3",
        )
        .map_err(|e| Error::Database(format!("Failed to prepare statement: {}", e)))?;

    let messages = stmt
        .query_map(rusqlite::params![target, reference_ts, limit as i64], row_to_message)
        .map_err(|e| Error::Database(format!("Failed to get messages: {}", e)))?
        .collect::<std::result::Result<Vec<_>, _>>()
        .map_err(|e| Error::Database(format!("Failed to collect messages: {}", e)))?;

    Ok(messages)
}

/// Get messages between two msgids.
pub fn get_between(
    conn: &PooledConnection,
    target: &str,
    start_msgid: &str,
    end_msgid: &str,
    limit: usize,
) -> Result<Vec<HistoryMessage>> {
    let start_ts: Option<i64> = conn
        .query_row(
            "SELECT timestamp FROM message_history WHERE msgid = ?1",
            rusqlite::params![start_msgid],
            |row| row.get(0),
        )
        .optional()
        .map_err(|e| Error::Database(format!("Failed to find start message: {}", e)))?;

    let end_ts: Option<i64> = conn
        .query_row(
            "SELECT timestamp FROM message_history WHERE msgid = ?1",
            rusqlite::params![end_msgid],
            |row| row.get(0),
        )
        .optional()
        .map_err(|e| Error::Database(format!("Failed to find end message: {}", e)))?;

    let (Some(start_ts), Some(end_ts)) = (start_ts, end_ts) else {
        return Ok(Vec::new());
    };

    let mut stmt = conn
        .prepare(
            "SELECT id, msgid, timestamp, sender_account, sender_nick, sender_user,
                    sender_host, target, target_type, command, message
             FROM message_history
             WHERE target = ?1 COLLATE NOCASE AND timestamp >= ?2 AND timestamp <= ?3
             ORDER BY timestamp ASC
             LIMIT ?4",
        )
        .map_err(|e| Error::Database(format!("Failed to prepare statement: {}", e)))?;

    let messages = stmt
        .query_map(rusqlite::params![target, start_ts, end_ts, limit as i64], row_to_message)
        .map_err(|e| Error::Database(format!("Failed to get messages: {}", e)))?
        .collect::<std::result::Result<Vec<_>, _>>()
        .map_err(|e| Error::Database(format!("Failed to collect messages: {}", e)))?;

    Ok(messages)
}

/// Clean up old messages based on retention period.
pub fn cleanup_old(conn: &PooledConnection, max_age_days: u32) -> Result<usize> {
    if max_age_days == 0 {
        return Ok(0); // Unlimited retention
    }

    let cutoff = Utc::now() - chrono::Duration::days(max_age_days as i64);
    let cutoff_ts = cutoff.timestamp_millis();

    let rows = conn
        .execute(
            "DELETE FROM message_history WHERE timestamp < ?1",
            rusqlite::params![cutoff_ts],
        )
        .map_err(|e| Error::Database(format!("Failed to cleanup old messages: {}", e)))?;

    Ok(rows)
}

/// Generate a unique message ID.
pub fn generate_msgid() -> String {
    use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};

    let now = Utc::now().timestamp_micros();
    let random: u32 = rand::random();
    let bytes = [now.to_be_bytes().as_slice(), &random.to_be_bytes()].concat();
    URL_SAFE_NO_PAD.encode(&bytes)
}

fn row_to_message(row: &rusqlite::Row) -> rusqlite::Result<HistoryMessage> {
    let id: i64 = row.get(0)?;
    let msgid: String = row.get(1)?;
    let timestamp_ms: i64 = row.get(2)?;
    let sender_account: Option<String> = row.get(3)?;
    let sender_nick: String = row.get(4)?;
    let sender_user: String = row.get(5)?;
    let sender_host: String = row.get(6)?;
    let target: String = row.get(7)?;
    let target_type_str: String = row.get(8)?;
    let command: String = row.get(9)?;
    let message: String = row.get(10)?;

    let timestamp = Utc.timestamp_millis_opt(timestamp_ms).single().unwrap_or_else(Utc::now);
    let target_type = TargetType::parse(&target_type_str).unwrap_or(TargetType::Channel);

    Ok(HistoryMessage {
        id,
        msgid,
        timestamp,
        sender: HistorySender {
            account: sender_account,
            nick: sender_nick,
            user: sender_user,
            host: sender_host,
        },
        target,
        target_type,
        command,
        message,
    })
}
