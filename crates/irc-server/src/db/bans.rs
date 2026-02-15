//! Server ban storage (K-lines, Z-lines).

use chrono::{DateTime, TimeZone, Utc};
use rusqlite::OptionalExtension;

use super::PooledConnection;
use crate::error::{Error, Result};

/// Server ban type.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BanType {
    /// K-line (user@host ban)
    Kline,
    /// Z-line (IP ban)
    Zline,
}

impl BanType {
    /// Convert to string for database storage.
    pub fn as_str(&self) -> &'static str {
        match self {
            BanType::Kline => "kline",
            BanType::Zline => "zline",
        }
    }

    /// Parse from string.
    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "kline" => Some(BanType::Kline),
            "zline" => Some(BanType::Zline),
            _ => None,
        }
    }
}

/// A server ban entry.
#[derive(Debug, Clone)]
pub struct ServerBan {
    /// Database ID (0 if not persisted yet).
    pub id: i64,
    /// Type of ban.
    pub ban_type: BanType,
    /// The mask (user@host for K-line, IP/CIDR for Z-line).
    pub mask: String,
    /// Optional reason.
    pub reason: Option<String>,
    /// Who set the ban.
    pub set_by: String,
    /// When the ban was set.
    pub set_at: DateTime<Utc>,
    /// When the ban expires (None = permanent).
    pub expires_at: Option<DateTime<Utc>>,
}

impl ServerBan {
    /// Create a new ban.
    pub fn new(
        ban_type: BanType,
        mask: String,
        reason: Option<String>,
        set_by: String,
        expires_at: Option<DateTime<Utc>>,
    ) -> Self {
        Self {
            id: 0,
            ban_type,
            mask,
            reason,
            set_by,
            set_at: Utc::now(),
            expires_at,
        }
    }

    /// Check if this ban has expired.
    pub fn is_expired(&self) -> bool {
        if let Some(expires) = self.expires_at {
            Utc::now() > expires
        } else {
            false
        }
    }
}

/// Add a ban to the database.
pub fn add_ban(conn: &PooledConnection, ban: &ServerBan) -> Result<i64> {
    conn.execute(
        "INSERT OR REPLACE INTO server_bans (ban_type, mask, reason, set_by, set_at, expires_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        rusqlite::params![
            ban.ban_type.as_str(),
            ban.mask,
            ban.reason,
            ban.set_by,
            ban.set_at.timestamp(),
            ban.expires_at.map(|t| t.timestamp()),
        ],
    )
    .map_err(|e| Error::Database(format!("Failed to add ban: {}", e)))?;

    Ok(conn.last_insert_rowid())
}

/// Remove a ban from the database.
pub fn remove_ban(conn: &PooledConnection, ban_type: BanType, mask: &str) -> Result<bool> {
    let rows = conn
        .execute(
            "DELETE FROM server_bans WHERE ban_type = ?1 AND mask = ?2",
            rusqlite::params![ban_type.as_str(), mask],
        )
        .map_err(|e| Error::Database(format!("Failed to remove ban: {}", e)))?;

    Ok(rows > 0)
}

/// Find a specific ban.
pub fn find_ban(
    conn: &PooledConnection,
    ban_type: BanType,
    mask: &str,
) -> Result<Option<ServerBan>> {
    let mut stmt = conn
        .prepare(
            "SELECT id, ban_type, mask, reason, set_by, set_at, expires_at
             FROM server_bans WHERE ban_type = ?1 AND mask = ?2",
        )
        .map_err(|e| Error::Database(format!("Failed to prepare statement: {}", e)))?;

    let result = stmt
        .query_row(rusqlite::params![ban_type.as_str(), mask], row_to_ban)
        .optional()
        .map_err(|e| Error::Database(format!("Failed to find ban: {}", e)))?;

    Ok(result)
}

/// List all bans of a given type.
pub fn list_bans(conn: &PooledConnection, ban_type: BanType) -> Result<Vec<ServerBan>> {
    let mut stmt = conn
        .prepare(
            "SELECT id, ban_type, mask, reason, set_by, set_at, expires_at
             FROM server_bans WHERE ban_type = ?1
             ORDER BY set_at DESC",
        )
        .map_err(|e| Error::Database(format!("Failed to prepare statement: {}", e)))?;

    let bans = stmt
        .query_map(rusqlite::params![ban_type.as_str()], row_to_ban)
        .map_err(|e| Error::Database(format!("Failed to list bans: {}", e)))?
        .collect::<std::result::Result<Vec<_>, _>>()
        .map_err(|e| Error::Database(format!("Failed to collect bans: {}", e)))?;

    Ok(bans)
}

/// Check if a value matches any active ban of the given type.
/// This does pattern matching for wildcards.
pub fn is_banned(
    conn: &PooledConnection,
    ban_type: BanType,
    value: &str,
) -> Result<Option<ServerBan>> {
    let bans = list_bans(conn, ban_type)?;
    let now = Utc::now();

    for ban in bans {
        // Skip expired bans
        if let Some(expires) = ban.expires_at
            && now > expires
        {
            continue;
        }

        // Check if value matches the mask pattern
        if crate::state::matches_mask(&ban.mask, value) {
            return Ok(Some(ban));
        }
    }

    Ok(None)
}

/// Clean up expired bans.
pub fn cleanup_expired(conn: &PooledConnection) -> Result<usize> {
    let now = Utc::now().timestamp();
    let rows = conn
        .execute(
            "DELETE FROM server_bans WHERE expires_at IS NOT NULL AND expires_at < ?1",
            rusqlite::params![now],
        )
        .map_err(|e| Error::Database(format!("Failed to cleanup expired bans: {}", e)))?;

    Ok(rows)
}

/// Convert a database row to a ServerBan.
fn row_to_ban(row: &rusqlite::Row) -> rusqlite::Result<ServerBan> {
    let id: i64 = row.get(0)?;
    let ban_type_str: String = row.get(1)?;
    let mask: String = row.get(2)?;
    let reason: Option<String> = row.get(3)?;
    let set_by: String = row.get(4)?;
    let set_at_ts: i64 = row.get(5)?;
    let expires_at_ts: Option<i64> = row.get(6)?;

    let ban_type = BanType::parse(&ban_type_str).unwrap_or(BanType::Kline);
    let set_at = Utc
        .timestamp_opt(set_at_ts, 0)
        .single()
        .unwrap_or_else(Utc::now);
    let expires_at = expires_at_ts.and_then(|ts| Utc.timestamp_opt(ts, 0).single());

    Ok(ServerBan {
        id,
        ban_type,
        mask,
        reason,
        set_by,
        set_at,
        expires_at,
    })
}

/// Parse a duration string like "1d", "2h", "30m" into seconds.
pub fn parse_duration(duration: &str) -> Option<i64> {
    let duration = duration.trim();
    if duration.is_empty() {
        return None;
    }

    let (num_str, unit) = if let Some(stripped) = duration.strip_suffix('d') {
        (stripped, 'd')
    } else if let Some(stripped) = duration.strip_suffix('h') {
        (stripped, 'h')
    } else if let Some(stripped) = duration.strip_suffix('m') {
        (stripped, 'm')
    } else if let Some(stripped) = duration.strip_suffix('s') {
        (stripped, 's')
    } else if duration.chars().all(|c| c.is_ascii_digit()) {
        // Assume seconds if just a number
        (duration, 's')
    } else {
        return None;
    };

    let num: i64 = num_str.parse().ok()?;

    let seconds = match unit {
        'd' => num * 24 * 60 * 60,
        'h' => num * 60 * 60,
        'm' => num * 60,
        's' => num,
        _ => return None,
    };

    Some(seconds)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_duration() {
        assert_eq!(parse_duration("1d"), Some(86400));
        assert_eq!(parse_duration("2h"), Some(7200));
        assert_eq!(parse_duration("30m"), Some(1800));
        assert_eq!(parse_duration("60s"), Some(60));
        assert_eq!(parse_duration("60"), Some(60));
        assert_eq!(parse_duration(""), None);
        assert_eq!(parse_duration("invalid"), None);
    }

    #[test]
    fn test_ban_type_roundtrip() {
        assert_eq!(
            BanType::parse(BanType::Kline.as_str()),
            Some(BanType::Kline)
        );
        assert_eq!(
            BanType::parse(BanType::Zline.as_str()),
            Some(BanType::Zline)
        );
    }
}
