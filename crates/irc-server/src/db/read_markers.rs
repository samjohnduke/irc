//! Read marker database operations for draft/read-marker support.

use chrono::{DateTime, TimeZone, Utc};
use rusqlite::{OptionalExtension, params};

use super::PooledConnection;
use crate::error::{Error, Result};

/// A read marker record.
#[derive(Debug, Clone)]
pub struct ReadMarker {
    pub account_id: i64,
    pub target: String,
    pub timestamp: DateTime<Utc>,
}

/// Get a read marker for an account and target.
pub fn get(conn: &PooledConnection, account_id: i64, target: &str) -> Result<Option<ReadMarker>> {
    let result = conn
        .query_row(
            "SELECT account_id, target, timestamp FROM read_markers
             WHERE account_id = ?1 AND target = ?2 COLLATE NOCASE",
            params![account_id, target],
            |row| {
                let ts: i64 = row.get(2)?;
                Ok(ReadMarker {
                    account_id: row.get(0)?,
                    target: row.get(1)?,
                    timestamp: Utc
                        .timestamp_millis_opt(ts)
                        .single()
                        .unwrap_or_else(Utc::now),
                })
            },
        )
        .optional()
        .map_err(|e| Error::Database(format!("Failed to get read marker: {}", e)))?;

    Ok(result)
}

/// Set a read marker for an account and target.
pub fn set(
    conn: &PooledConnection,
    account_id: i64,
    target: &str,
    timestamp: DateTime<Utc>,
) -> Result<()> {
    conn.execute(
        "INSERT INTO read_markers (account_id, target, timestamp) VALUES (?1, ?2, ?3)
         ON CONFLICT(account_id, target) DO UPDATE SET timestamp = excluded.timestamp",
        params![account_id, target, timestamp.timestamp_millis()],
    )
    .map_err(|e| Error::Database(format!("Failed to set read marker: {}", e)))?;

    Ok(())
}

/// Delete a read marker.
pub fn delete(conn: &PooledConnection, account_id: i64, target: &str) -> Result<()> {
    conn.execute(
        "DELETE FROM read_markers WHERE account_id = ?1 AND target = ?2 COLLATE NOCASE",
        params![account_id, target],
    )
    .map_err(|e| Error::Database(format!("Failed to delete read marker: {}", e)))?;

    Ok(())
}

/// Get all read markers for an account.
pub fn get_all(conn: &PooledConnection, account_id: i64) -> Result<Vec<ReadMarker>> {
    let mut stmt = conn
        .prepare("SELECT account_id, target, timestamp FROM read_markers WHERE account_id = ?1")
        .map_err(|e| Error::Database(format!("Failed to prepare statement: {}", e)))?;

    let markers = stmt
        .query_map(params![account_id], |row| {
            let ts: i64 = row.get(2)?;
            Ok(ReadMarker {
                account_id: row.get(0)?,
                target: row.get(1)?,
                timestamp: Utc
                    .timestamp_millis_opt(ts)
                    .single()
                    .unwrap_or_else(Utc::now),
            })
        })
        .map_err(|e| Error::Database(format!("Failed to get read markers: {}", e)))?
        .collect::<std::result::Result<Vec<_>, _>>()
        .map_err(|e| Error::Database(format!("Failed to collect markers: {}", e)))?;

    Ok(markers)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::Database;

    #[test]
    fn test_read_marker_crud() {
        let db = Database::in_memory().unwrap();
        let conn = db.connection().unwrap();

        // Create a test account first
        conn.execute(
            "INSERT INTO accounts (name, password_hash, registered_at) VALUES ('test', 'hash', 0)",
            [],
        )
        .unwrap();

        let now = Utc::now();

        // Set marker
        set(&conn, 1, "#channel", now).unwrap();

        // Get marker
        let marker = get(&conn, 1, "#channel").unwrap().unwrap();
        assert_eq!(marker.target, "#channel");

        // Update marker
        let later = now + chrono::Duration::hours(1);
        set(&conn, 1, "#channel", later).unwrap();
        let marker2 = get(&conn, 1, "#channel").unwrap().unwrap();
        assert!(marker2.timestamp > marker.timestamp);

        // Get all
        set(&conn, 1, "#other", now).unwrap();
        let all = get_all(&conn, 1).unwrap();
        assert_eq!(all.len(), 2);

        // Delete
        delete(&conn, 1, "#channel").unwrap();
        assert!(get(&conn, 1, "#channel").unwrap().is_none());
    }
}
