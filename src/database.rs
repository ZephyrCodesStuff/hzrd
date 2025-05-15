use std::{
    path::Path,
    time::{SystemTime, UNIX_EPOCH},
};

use rusqlite::{params, Connection, Result as SQLiteResult};
use tracing::error;

use crate::structs::{config::DatabaseConfig, flag::FlagStatus};

// Initialize the database with required tables
pub fn init_db(db_config: &DatabaseConfig) -> SQLiteResult<Connection> {
    let db_path = Path::new(&db_config.file);
    let db_exists = db_path.exists();

    // Create database connection
    let conn = Connection::open(db_path)?;

    // Create tables if they don't exist
    if !db_exists {
        conn.execute(
            "CREATE TABLE flags (
                    id INTEGER PRIMARY KEY,
                    flag TEXT NOT NULL UNIQUE,
                    status TEXT NOT NULL,
                    points REAL DEFAULT 0.0,
                    captured_at INTEGER NOT NULL,
                    submitted_at INTEGER,
                    error_message TEXT
                )",
            [],
        )?;
    }

    Ok(conn)
}

// Store new flags in the database
pub fn store_flags(conn: &Connection, flags: &[String]) -> usize {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs();

    let mut count = 0;

    for flag in flags {
        match conn.execute(
            "INSERT OR IGNORE INTO flags (flag, status, captured_at) VALUES (?1, ?2, ?3)",
            params![flag, FlagStatus::Pending.to_string(), now],
        ) {
            Ok(rows) => count += rows,
            Err(e) => error!("Error storing flag {flag}: {e}"),
        }
    }

    count
}

// Get pending flags that need to be submitted
pub fn get_pending_flags(conn: &Connection) -> SQLiteResult<Vec<String>> {
    let mut stmt = conn.prepare("SELECT flag FROM flags WHERE status = ?1")?;
    let flags = stmt.query_map([FlagStatus::Pending.to_string()], |row| {
        row.get::<_, String>(0)
    })?;

    let mut result = Vec::new();
    for flag in flags {
        result.push(flag?);
    }

    Ok(result)
}

// Update flag status after submission
pub fn update_flag_status(
    conn: &Connection,
    flag: &str,
    status: FlagStatus,
    points: Option<f64>,
    error_message: Option<&str>,
) -> SQLiteResult<usize> {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs();

    conn.execute(
            "UPDATE flags SET status = ?1, points = ?2, submitted_at = ?3, error_message = ?4 WHERE flag = ?5",
            params![
                status.to_string(),
                points.unwrap_or(0.0),
                now,
                error_message,
                flag
            ],
        )
}

// Get summary of points earned
pub fn get_points_summary(conn: &Connection) -> SQLiteResult<f64> {
    let mut stmt = conn.prepare("SELECT SUM(points) FROM flags WHERE status = ?1")?;
    let points: f64 = stmt.query_row([FlagStatus::Accepted.to_string()], |row| {
        Ok(row.get(0).unwrap_or(0.0))
    })?;

    Ok(points)
}
