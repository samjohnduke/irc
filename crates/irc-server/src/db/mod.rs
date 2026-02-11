//! SQLite database layer for persistent storage.
//!
//! This module provides database access for:
//! - Account registration and authentication
//! - Nickname registration
//! - Channel registration and access control
//! - Server bans (K-lines, Z-lines)
//! - Message history

pub mod accounts;
pub mod bans;
pub mod channels;
pub mod history;
pub mod nicks;

use std::path::Path;

use r2d2::Pool;
use r2d2_sqlite::SqliteConnectionManager;

use crate::error::{Error, Result};

/// Connection from the pool.
pub type PooledConnection = r2d2::PooledConnection<SqliteConnectionManager>;

/// Database wrapper with connection pool.
pub struct Database {
    pool: Pool<SqliteConnectionManager>,
}

impl Database {
    /// Create a new database connection pool.
    ///
    /// If `path` is `:memory:`, creates an in-memory database.
    pub fn new(path: &Path) -> Result<Self> {
        let manager = SqliteConnectionManager::file(path);
        let pool = Pool::builder()
            .max_size(10)
            .build(manager)
            .map_err(|e| Error::Database(format!("Failed to create connection pool: {}", e)))?;

        let db = Self { pool };
        db.init_schema()?;
        Ok(db)
    }

    /// Create an in-memory database (useful for testing).
    pub fn in_memory() -> Result<Self> {
        let manager = SqliteConnectionManager::memory();
        let pool = Pool::builder()
            .max_size(1) // In-memory databases don't share across connections
            .build(manager)
            .map_err(|e| Error::Database(format!("Failed to create connection pool: {}", e)))?;

        let db = Self { pool };
        db.init_schema()?;
        Ok(db)
    }

    /// Get a connection from the pool.
    pub fn connection(&self) -> Result<PooledConnection> {
        self.pool
            .get()
            .map_err(|e| Error::Database(format!("Failed to get connection: {}", e)))
    }

    /// Initialize the database schema.
    fn init_schema(&self) -> Result<()> {
        let conn = self.connection()?;

        conn.execute_batch(
            r#"
            -- Accounts table
            CREATE TABLE IF NOT EXISTS accounts (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                name TEXT NOT NULL UNIQUE COLLATE NOCASE,
                password_hash TEXT NOT NULL,
                email TEXT,
                registered_at INTEGER NOT NULL,
                last_seen INTEGER
            );

            -- Registered nicknames table
            CREATE TABLE IF NOT EXISTS registered_nicks (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                nickname TEXT NOT NULL UNIQUE COLLATE NOCASE,
                account_id INTEGER NOT NULL REFERENCES accounts(id) ON DELETE CASCADE,
                registered_at INTEGER NOT NULL,
                is_primary INTEGER NOT NULL DEFAULT 0
            );

            -- Registered channels table
            CREATE TABLE IF NOT EXISTS registered_channels (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                name TEXT NOT NULL UNIQUE COLLATE NOCASE,
                founder_account_id INTEGER NOT NULL REFERENCES accounts(id) ON DELETE CASCADE,
                registered_at INTEGER NOT NULL,
                topic TEXT,
                modes TEXT
            );

            -- Channel access list table
            CREATE TABLE IF NOT EXISTS channel_access (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                channel_id INTEGER NOT NULL REFERENCES registered_channels(id) ON DELETE CASCADE,
                account_id INTEGER NOT NULL REFERENCES accounts(id) ON DELETE CASCADE,
                flags TEXT NOT NULL,
                UNIQUE(channel_id, account_id)
            );

            -- Indices for faster lookups
            CREATE INDEX IF NOT EXISTS idx_registered_nicks_account ON registered_nicks(account_id);
            CREATE INDEX IF NOT EXISTS idx_registered_channels_founder ON registered_channels(founder_account_id);
            CREATE INDEX IF NOT EXISTS idx_channel_access_channel ON channel_access(channel_id);
            CREATE INDEX IF NOT EXISTS idx_channel_access_account ON channel_access(account_id);

            -- Server bans table (K-lines, Z-lines)
            CREATE TABLE IF NOT EXISTS server_bans (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                ban_type TEXT NOT NULL,
                mask TEXT NOT NULL,
                reason TEXT,
                set_by TEXT NOT NULL,
                set_at INTEGER NOT NULL,
                expires_at INTEGER,
                UNIQUE(ban_type, mask)
            );

            CREATE INDEX IF NOT EXISTS idx_server_bans_type ON server_bans(ban_type);
            CREATE INDEX IF NOT EXISTS idx_server_bans_expires ON server_bans(expires_at);

            -- Message history table
            CREATE TABLE IF NOT EXISTS message_history (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                msgid TEXT UNIQUE,
                timestamp INTEGER NOT NULL,
                sender_account TEXT,
                sender_nick TEXT NOT NULL,
                sender_user TEXT NOT NULL,
                sender_host TEXT NOT NULL,
                target TEXT NOT NULL,
                target_type TEXT NOT NULL,
                command TEXT NOT NULL,
                message TEXT NOT NULL
            );

            CREATE INDEX IF NOT EXISTS idx_history_target ON message_history(target, timestamp);
            CREATE INDEX IF NOT EXISTS idx_history_msgid ON message_history(msgid);

            -- Enable foreign keys
            PRAGMA foreign_keys = ON;
            "#,
        )
        .map_err(|e| Error::Database(format!("Failed to initialize schema: {}", e)))?;

        Ok(())
    }
}

impl std::fmt::Debug for Database {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Database").finish_non_exhaustive()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_in_memory() {
        let db = Database::in_memory().expect("Failed to create in-memory database");
        let conn = db.connection().expect("Failed to get connection");

        // Verify tables exist
        let count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name IN ('accounts', 'registered_nicks', 'registered_channels', 'channel_access')",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(count, 4);
    }
}
