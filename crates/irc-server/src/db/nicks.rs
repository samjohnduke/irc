//! Nickname registration database operations.

use chrono::{DateTime, TimeZone, Utc};
use rusqlite::{params, OptionalExtension};

use super::PooledConnection;
use crate::error::{Error, Result};

/// A registered nickname record.
#[derive(Debug, Clone)]
pub struct RegisteredNick {
    pub id: i64,
    pub nickname: String,
    pub account_id: i64,
    pub registered_at: DateTime<Utc>,
    pub is_primary: bool,
}

/// Register a nickname to an account.
///
/// Returns the registration ID.
pub fn register(
    conn: &PooledConnection,
    nickname: &str,
    account_id: i64,
    is_primary: bool,
) -> Result<i64> {
    let now = Utc::now().timestamp();

    conn.execute(
        "INSERT INTO registered_nicks (nickname, account_id, registered_at, is_primary) VALUES (?1, ?2, ?3, ?4)",
        params![nickname, account_id, now, is_primary as i32],
    )
    .map_err(|e| {
        if let rusqlite::Error::SqliteFailure(ref err, _) = e
            && err.extended_code == rusqlite::ffi::SQLITE_CONSTRAINT_UNIQUE
        {
            return Error::NickRegistered(nickname.to_string());
        }
        Error::Database(format!("Failed to register nickname: {}", e))
    })?;

    Ok(conn.last_insert_rowid())
}

/// Check if a nickname is registered.
pub fn is_registered(conn: &PooledConnection, nickname: &str) -> Result<bool> {
    let count: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM registered_nicks WHERE nickname = ?1 COLLATE NOCASE",
            params![nickname],
            |row| row.get(0),
        )
        .map_err(|e| Error::Database(format!("Failed to check nickname: {}", e)))?;

    Ok(count > 0)
}

/// Find nickname registration info.
pub fn find(conn: &PooledConnection, nickname: &str) -> Result<Option<RegisteredNick>> {
    let mut stmt = conn
        .prepare(
            "SELECT id, nickname, account_id, registered_at, is_primary
             FROM registered_nicks WHERE nickname = ?1 COLLATE NOCASE",
        )
        .map_err(|e| Error::Database(format!("Failed to prepare statement: {}", e)))?;

    let result = stmt
        .query_row(params![nickname], |row| {
            Ok(RegisteredNick {
                id: row.get(0)?,
                nickname: row.get(1)?,
                account_id: row.get(2)?,
                registered_at: Utc.timestamp_opt(row.get::<_, i64>(3)?, 0).unwrap(),
                is_primary: row.get::<_, i32>(4)? != 0,
            })
        })
        .optional()
        .map_err(|e| Error::Database(format!("Failed to query nickname: {}", e)))?;

    Ok(result)
}

/// Get all nicknames for an account.
pub fn get_for_account(conn: &PooledConnection, account_id: i64) -> Result<Vec<RegisteredNick>> {
    let mut stmt = conn
        .prepare(
            "SELECT id, nickname, account_id, registered_at, is_primary
             FROM registered_nicks WHERE account_id = ?1 ORDER BY is_primary DESC, nickname",
        )
        .map_err(|e| Error::Database(format!("Failed to prepare statement: {}", e)))?;

    let rows = stmt
        .query_map(params![account_id], |row| {
            Ok(RegisteredNick {
                id: row.get(0)?,
                nickname: row.get(1)?,
                account_id: row.get(2)?,
                registered_at: Utc.timestamp_opt(row.get::<_, i64>(3)?, 0).unwrap(),
                is_primary: row.get::<_, i32>(4)? != 0,
            })
        })
        .map_err(|e| Error::Database(format!("Failed to query nicknames: {}", e)))?;

    let mut nicks = Vec::new();
    for row in rows {
        nicks.push(row.map_err(|e| Error::Database(format!("Failed to read row: {}", e)))?);
    }

    Ok(nicks)
}

/// Get the account ID that owns a nickname.
pub fn get_owner_account(conn: &PooledConnection, nickname: &str) -> Result<Option<i64>> {
    let result: Option<i64> = conn
        .query_row(
            "SELECT account_id FROM registered_nicks WHERE nickname = ?1 COLLATE NOCASE",
            params![nickname],
            |row| row.get(0),
        )
        .optional()
        .map_err(|e| Error::Database(format!("Failed to query owner: {}", e)))?;

    Ok(result)
}

/// Unregister a nickname.
pub fn unregister(conn: &PooledConnection, nickname: &str) -> Result<bool> {
    let rows = conn
        .execute(
            "DELETE FROM registered_nicks WHERE nickname = ?1 COLLATE NOCASE",
            params![nickname],
        )
        .map_err(|e| Error::Database(format!("Failed to unregister nickname: {}", e)))?;

    Ok(rows > 0)
}

/// Unregister all nicknames for an account.
pub fn unregister_all(conn: &PooledConnection, account_id: i64) -> Result<usize> {
    let rows = conn
        .execute(
            "DELETE FROM registered_nicks WHERE account_id = ?1",
            params![account_id],
        )
        .map_err(|e| Error::Database(format!("Failed to unregister nicknames: {}", e)))?;

    Ok(rows)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::{accounts, Database};

    #[test]
    fn test_nick_registration() {
        let db = Database::in_memory().unwrap();
        let conn = db.connection().unwrap();

        // Create an account first
        let account_id = accounts::create(&conn, "TestUser", "hash123", None).unwrap();

        // Register nickname
        let nick_id = register(&conn, "TestUser", account_id, true).unwrap();
        assert!(nick_id > 0);

        // Check registration
        assert!(is_registered(&conn, "testuser").unwrap());
        assert!(!is_registered(&conn, "othernick").unwrap());

        // Find
        let nick = find(&conn, "TestUser").unwrap().unwrap();
        assert_eq!(nick.account_id, account_id);
        assert!(nick.is_primary);

        // Get for account
        let nicks = get_for_account(&conn, account_id).unwrap();
        assert_eq!(nicks.len(), 1);

        // Get owner
        let owner = get_owner_account(&conn, "TestUser").unwrap();
        assert_eq!(owner, Some(account_id));

        // Unregister
        assert!(unregister(&conn, "TestUser").unwrap());
        assert!(!is_registered(&conn, "TestUser").unwrap());
    }

    #[test]
    fn test_duplicate_nick() {
        let db = Database::in_memory().unwrap();
        let conn = db.connection().unwrap();

        let account_id = accounts::create(&conn, "User1", "hash", None).unwrap();
        register(&conn, "CoolNick", account_id, true).unwrap();

        // Try to register same nick (case-insensitive)
        let result = register(&conn, "coolnick", account_id, false);
        assert!(matches!(result, Err(Error::NickRegistered(_))));
    }
}
