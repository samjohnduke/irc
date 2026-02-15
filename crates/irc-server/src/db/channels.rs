//! Channel registration database operations.

use chrono::{DateTime, TimeZone, Utc};
use rusqlite::{OptionalExtension, params};

use super::PooledConnection;
use crate::error::{Error, Result};

/// A registered channel record.
#[derive(Debug, Clone)]
pub struct RegisteredChannel {
    pub id: i64,
    pub name: String,
    pub founder_account_id: i64,
    pub registered_at: DateTime<Utc>,
    pub topic: Option<String>,
    pub modes: Option<String>,
}

/// Channel access entry.
#[derive(Debug, Clone)]
pub struct ChannelAccess {
    pub id: i64,
    pub channel_id: i64,
    pub account_id: i64,
    pub flags: String,
}

/// Register a channel.
///
/// Returns the registration ID.
pub fn register(conn: &PooledConnection, name: &str, founder_account_id: i64) -> Result<i64> {
    let now = Utc::now().timestamp();

    conn.execute(
        "INSERT INTO registered_channels (name, founder_account_id, registered_at) VALUES (?1, ?2, ?3)",
        params![name, founder_account_id, now],
    )
    .map_err(|e| {
        if let rusqlite::Error::SqliteFailure(ref err, _) = e
            && err.extended_code == rusqlite::ffi::SQLITE_CONSTRAINT_UNIQUE
        {
            return Error::ChannelRegistered(name.to_string());
        }
        Error::Database(format!("Failed to register channel: {}", e))
    })?;

    let channel_id = conn.last_insert_rowid();

    // Add founder with full flags
    set_access(conn, channel_id, founder_account_id, "+voFfr")?;

    Ok(channel_id)
}

/// Check if a channel is registered.
pub fn is_registered(conn: &PooledConnection, name: &str) -> Result<bool> {
    let count: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM registered_channels WHERE name = ?1 COLLATE NOCASE",
            params![name],
            |row| row.get(0),
        )
        .map_err(|e| Error::Database(format!("Failed to check channel: {}", e)))?;

    Ok(count > 0)
}

/// Find channel registration info.
pub fn find(conn: &PooledConnection, name: &str) -> Result<Option<RegisteredChannel>> {
    let mut stmt = conn
        .prepare(
            "SELECT id, name, founder_account_id, registered_at, topic, modes
             FROM registered_channels WHERE name = ?1 COLLATE NOCASE",
        )
        .map_err(|e| Error::Database(format!("Failed to prepare statement: {}", e)))?;

    let result = stmt
        .query_row(params![name], |row| {
            Ok(RegisteredChannel {
                id: row.get(0)?,
                name: row.get(1)?,
                founder_account_id: row.get(2)?,
                registered_at: Utc.timestamp_opt(row.get::<_, i64>(3)?, 0).unwrap(),
                topic: row.get(4)?,
                modes: row.get(5)?,
            })
        })
        .optional()
        .map_err(|e| Error::Database(format!("Failed to query channel: {}", e)))?;

    Ok(result)
}

/// Get channel by ID.
pub fn find_by_id(conn: &PooledConnection, id: i64) -> Result<Option<RegisteredChannel>> {
    let mut stmt = conn
        .prepare(
            "SELECT id, name, founder_account_id, registered_at, topic, modes
             FROM registered_channels WHERE id = ?1",
        )
        .map_err(|e| Error::Database(format!("Failed to prepare statement: {}", e)))?;

    let result = stmt
        .query_row(params![id], |row| {
            Ok(RegisteredChannel {
                id: row.get(0)?,
                name: row.get(1)?,
                founder_account_id: row.get(2)?,
                registered_at: Utc.timestamp_opt(row.get::<_, i64>(3)?, 0).unwrap(),
                topic: row.get(4)?,
                modes: row.get(5)?,
            })
        })
        .optional()
        .map_err(|e| Error::Database(format!("Failed to query channel: {}", e)))?;

    Ok(result)
}

/// Update channel topic.
pub fn update_topic(conn: &PooledConnection, id: i64, topic: Option<&str>) -> Result<()> {
    conn.execute(
        "UPDATE registered_channels SET topic = ?1 WHERE id = ?2",
        params![topic, id],
    )
    .map_err(|e| Error::Database(format!("Failed to update topic: {}", e)))?;

    Ok(())
}

/// Update channel modes.
pub fn update_modes(conn: &PooledConnection, id: i64, modes: Option<&str>) -> Result<()> {
    conn.execute(
        "UPDATE registered_channels SET modes = ?1 WHERE id = ?2",
        params![modes, id],
    )
    .map_err(|e| Error::Database(format!("Failed to update modes: {}", e)))?;

    Ok(())
}

/// Unregister a channel.
pub fn unregister(conn: &PooledConnection, name: &str) -> Result<bool> {
    let rows = conn
        .execute(
            "DELETE FROM registered_channels WHERE name = ?1 COLLATE NOCASE",
            params![name],
        )
        .map_err(|e| Error::Database(format!("Failed to unregister channel: {}", e)))?;

    Ok(rows > 0)
}

// ============ Channel Access ============

/// Set access flags for an account on a channel.
pub fn set_access(
    conn: &PooledConnection,
    channel_id: i64,
    account_id: i64,
    flags: &str,
) -> Result<()> {
    conn.execute(
        "INSERT INTO channel_access (channel_id, account_id, flags) VALUES (?1, ?2, ?3)
         ON CONFLICT(channel_id, account_id) DO UPDATE SET flags = excluded.flags",
        params![channel_id, account_id, flags],
    )
    .map_err(|e| Error::Database(format!("Failed to set access: {}", e)))?;

    Ok(())
}

/// Get access flags for an account on a channel.
pub fn get_access(
    conn: &PooledConnection,
    channel_id: i64,
    account_id: i64,
) -> Result<Option<String>> {
    let result: Option<String> = conn
        .query_row(
            "SELECT flags FROM channel_access WHERE channel_id = ?1 AND account_id = ?2",
            params![channel_id, account_id],
            |row| row.get(0),
        )
        .optional()
        .map_err(|e| Error::Database(format!("Failed to get access: {}", e)))?;

    Ok(result)
}

/// Get access flags for an account on a channel by channel name.
pub fn get_user_flags(
    conn: &PooledConnection,
    channel_name: &str,
    account_name: &str,
) -> Result<Option<String>> {
    let result: Option<String> = conn
        .query_row(
            "SELECT ca.flags FROM channel_access ca
             JOIN registered_channels rc ON rc.id = ca.channel_id
             JOIN accounts a ON a.id = ca.account_id
             WHERE rc.name = ?1 COLLATE NOCASE AND a.name = ?2 COLLATE NOCASE",
            params![channel_name, account_name],
            |row| row.get(0),
        )
        .optional()
        .map_err(|e| Error::Database(format!("Failed to get user flags: {}", e)))?;

    Ok(result)
}

/// Remove access for an account on a channel.
pub fn remove_access(conn: &PooledConnection, channel_id: i64, account_id: i64) -> Result<bool> {
    let rows = conn
        .execute(
            "DELETE FROM channel_access WHERE channel_id = ?1 AND account_id = ?2",
            params![channel_id, account_id],
        )
        .map_err(|e| Error::Database(format!("Failed to remove access: {}", e)))?;

    Ok(rows > 0)
}

/// Get all access entries for a channel.
pub fn get_all_access(conn: &PooledConnection, channel_id: i64) -> Result<Vec<(String, String)>> {
    let mut stmt = conn
        .prepare(
            "SELECT a.name, ca.flags FROM channel_access ca
             JOIN accounts a ON a.id = ca.account_id
             WHERE ca.channel_id = ?1
             ORDER BY a.name",
        )
        .map_err(|e| Error::Database(format!("Failed to prepare statement: {}", e)))?;

    let rows = stmt
        .query_map(params![channel_id], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })
        .map_err(|e| Error::Database(format!("Failed to query access: {}", e)))?;

    let mut access = Vec::new();
    for row in rows {
        access.push(row.map_err(|e| Error::Database(format!("Failed to read row: {}", e)))?);
    }

    Ok(access)
}

/// Check if an account has specific flag(s) on a channel.
pub fn has_flag(
    conn: &PooledConnection,
    channel_name: &str,
    account_name: &str,
    flag: char,
) -> Result<bool> {
    if let Some(flags) = get_user_flags(conn, channel_name, account_name)? {
        Ok(flags.contains(flag))
    } else {
        Ok(false)
    }
}

/// Get founder account name for a channel.
pub fn get_founder(conn: &PooledConnection, channel_name: &str) -> Result<Option<String>> {
    let result: Option<String> = conn
        .query_row(
            "SELECT a.name FROM accounts a
             JOIN registered_channels rc ON rc.founder_account_id = a.id
             WHERE rc.name = ?1 COLLATE NOCASE",
            params![channel_name],
            |row| row.get(0),
        )
        .optional()
        .map_err(|e| Error::Database(format!("Failed to get founder: {}", e)))?;

    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::{Database, accounts};

    #[test]
    fn test_channel_registration() {
        let db = Database::in_memory().unwrap();
        let conn = db.connection().unwrap();

        // Create an account first
        let account_id = accounts::create(&conn, "Founder", "hash123", None).unwrap();

        // Register channel
        let channel_id = register(&conn, "#test", account_id).unwrap();
        assert!(channel_id > 0);

        // Check registration
        assert!(is_registered(&conn, "#test").unwrap());
        assert!(!is_registered(&conn, "#other").unwrap());

        // Find
        let channel = find(&conn, "#test").unwrap().unwrap();
        assert_eq!(channel.founder_account_id, account_id);

        // Founder should have full access
        let flags = get_access(&conn, channel_id, account_id).unwrap().unwrap();
        assert!(flags.contains('F'));
        assert!(flags.contains('o'));
        assert!(flags.contains('v'));

        // Get founder
        let founder = get_founder(&conn, "#test").unwrap().unwrap();
        assert_eq!(founder, "Founder");

        // Unregister
        assert!(unregister(&conn, "#test").unwrap());
        assert!(!is_registered(&conn, "#test").unwrap());
    }

    #[test]
    fn test_channel_access() {
        let db = Database::in_memory().unwrap();
        let conn = db.connection().unwrap();

        let founder_id = accounts::create(&conn, "Founder", "hash", None).unwrap();
        let user_id = accounts::create(&conn, "User", "hash", None).unwrap();

        let channel_id = register(&conn, "#test", founder_id).unwrap();

        // Set access for another user
        set_access(&conn, channel_id, user_id, "+vo").unwrap();

        // Check access
        let flags = get_access(&conn, channel_id, user_id).unwrap().unwrap();
        assert_eq!(flags, "+vo");

        // Check via helper
        assert!(has_flag(&conn, "#test", "User", 'v').unwrap());
        assert!(has_flag(&conn, "#test", "User", 'o').unwrap());
        assert!(!has_flag(&conn, "#test", "User", 'F').unwrap());

        // List all access
        let all = get_all_access(&conn, channel_id).unwrap();
        assert_eq!(all.len(), 2); // founder + user

        // Remove access
        remove_access(&conn, channel_id, user_id).unwrap();
        assert!(get_access(&conn, channel_id, user_id).unwrap().is_none());
    }

    #[test]
    fn test_duplicate_channel() {
        let db = Database::in_memory().unwrap();
        let conn = db.connection().unwrap();

        let account_id = accounts::create(&conn, "User", "hash", None).unwrap();
        register(&conn, "#test", account_id).unwrap();

        // Try to register same channel (case-insensitive)
        let result = register(&conn, "#TEST", account_id);
        assert!(matches!(result, Err(Error::ChannelRegistered(_))));
    }
}
