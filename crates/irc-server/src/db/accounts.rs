//! Account database operations.

use chrono::{DateTime, TimeZone, Utc};
use rusqlite::{params, OptionalExtension};

use super::PooledConnection;
use crate::error::{Error, Result};

/// An account record from the database.
#[derive(Debug, Clone)]
pub struct Account {
    pub id: i64,
    pub name: String,
    pub password_hash: String,
    pub email: Option<String>,
    pub registered_at: DateTime<Utc>,
    pub last_seen: Option<DateTime<Utc>>,
}

/// Create a new account.
///
/// Returns the new account ID.
pub fn create(
    conn: &PooledConnection,
    name: &str,
    password_hash: &str,
    email: Option<&str>,
) -> Result<i64> {
    let now = Utc::now().timestamp();

    conn.execute(
        "INSERT INTO accounts (name, password_hash, email, registered_at) VALUES (?1, ?2, ?3, ?4)",
        params![name, password_hash, email, now],
    )
    .map_err(|e| {
        if let rusqlite::Error::SqliteFailure(ref err, _) = e
            && err.extended_code == rusqlite::ffi::SQLITE_CONSTRAINT_UNIQUE
        {
            return Error::AccountExists(name.to_string());
        }
        Error::Database(format!("Failed to create account: {}", e))
    })?;

    Ok(conn.last_insert_rowid())
}

/// Find an account by name (case-insensitive).
pub fn find_by_name(conn: &PooledConnection, name: &str) -> Result<Option<Account>> {
    let mut stmt = conn
        .prepare(
            "SELECT id, name, password_hash, email, registered_at, last_seen
             FROM accounts WHERE name = ?1 COLLATE NOCASE",
        )
        .map_err(|e| Error::Database(format!("Failed to prepare statement: {}", e)))?;

    let result = stmt
        .query_row(params![name], |row| {
            Ok(Account {
                id: row.get(0)?,
                name: row.get(1)?,
                password_hash: row.get(2)?,
                email: row.get(3)?,
                registered_at: Utc.timestamp_opt(row.get::<_, i64>(4)?, 0).unwrap(),
                last_seen: row.get::<_, Option<i64>>(5)?.map(|ts| Utc.timestamp_opt(ts, 0).unwrap()),
            })
        })
        .optional()
        .map_err(|e| Error::Database(format!("Failed to query account: {}", e)))?;

    Ok(result)
}

/// Find an account by ID.
pub fn find_by_id(conn: &PooledConnection, id: i64) -> Result<Option<Account>> {
    let mut stmt = conn
        .prepare(
            "SELECT id, name, password_hash, email, registered_at, last_seen
             FROM accounts WHERE id = ?1",
        )
        .map_err(|e| Error::Database(format!("Failed to prepare statement: {}", e)))?;

    let result = stmt
        .query_row(params![id], |row| {
            Ok(Account {
                id: row.get(0)?,
                name: row.get(1)?,
                password_hash: row.get(2)?,
                email: row.get(3)?,
                registered_at: Utc.timestamp_opt(row.get::<_, i64>(4)?, 0).unwrap(),
                last_seen: row.get::<_, Option<i64>>(5)?.map(|ts| Utc.timestamp_opt(ts, 0).unwrap()),
            })
        })
        .optional()
        .map_err(|e| Error::Database(format!("Failed to query account: {}", e)))?;

    Ok(result)
}

/// Check if an account exists by name.
pub fn exists(conn: &PooledConnection, name: &str) -> Result<bool> {
    let count: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM accounts WHERE name = ?1 COLLATE NOCASE",
            params![name],
            |row| row.get(0),
        )
        .map_err(|e| Error::Database(format!("Failed to check account: {}", e)))?;

    Ok(count > 0)
}

/// Update account password.
pub fn update_password(conn: &PooledConnection, id: i64, password_hash: &str) -> Result<()> {
    conn.execute(
        "UPDATE accounts SET password_hash = ?1 WHERE id = ?2",
        params![password_hash, id],
    )
    .map_err(|e| Error::Database(format!("Failed to update password: {}", e)))?;

    Ok(())
}

/// Update last seen timestamp.
pub fn update_last_seen(conn: &PooledConnection, id: i64) -> Result<()> {
    let now = Utc::now().timestamp();
    conn.execute(
        "UPDATE accounts SET last_seen = ?1 WHERE id = ?2",
        params![now, id],
    )
    .map_err(|e| Error::Database(format!("Failed to update last seen: {}", e)))?;

    Ok(())
}

/// Delete an account.
pub fn delete(conn: &PooledConnection, id: i64) -> Result<()> {
    conn.execute("DELETE FROM accounts WHERE id = ?1", params![id])
        .map_err(|e| Error::Database(format!("Failed to delete account: {}", e)))?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::Database;

    #[test]
    fn test_account_crud() {
        let db = Database::in_memory().unwrap();
        let conn = db.connection().unwrap();

        // Create
        let id = create(&conn, "TestUser", "hash123", Some("test@example.com")).unwrap();
        assert!(id > 0);

        // Find by name
        let account = find_by_name(&conn, "testuser").unwrap().unwrap();
        assert_eq!(account.name, "TestUser");
        assert_eq!(account.email, Some("test@example.com".to_string()));

        // Find by ID
        let account2 = find_by_id(&conn, id).unwrap().unwrap();
        assert_eq!(account2.name, "TestUser");

        // Exists
        assert!(exists(&conn, "TestUser").unwrap());
        assert!(!exists(&conn, "NonExistent").unwrap());

        // Update password
        update_password(&conn, id, "newhash").unwrap();
        let updated = find_by_id(&conn, id).unwrap().unwrap();
        assert_eq!(updated.password_hash, "newhash");

        // Delete
        delete(&conn, id).unwrap();
        assert!(!exists(&conn, "TestUser").unwrap());
    }

    #[test]
    fn test_duplicate_account() {
        let db = Database::in_memory().unwrap();
        let conn = db.connection().unwrap();

        create(&conn, "TestUser", "hash123", None).unwrap();

        // Try to create duplicate (case-insensitive)
        let result = create(&conn, "testuser", "hash456", None);
        assert!(matches!(result, Err(Error::AccountExists(_))));
    }
}
