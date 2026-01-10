//! Database module for the LCARS application.
//!
//! Provides database initialization, migrations, and models.

#![allow(dead_code)]

use rusqlite::Connection;
use std::path::Path;

pub mod models;
pub mod queries;

mod embedded {
    use refinery::embed_migrations;
    embed_migrations!("src/db/migrations");
}

#[derive(Debug)]
pub enum DbError {
    Connection(rusqlite::Error),
    Migration(refinery::Error),
}

impl std::fmt::Display for DbError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DbError::Connection(e) => write!(f, "Database connection error: {}", e),
            DbError::Migration(e) => write!(f, "Migration error: {}", e),
        }
    }
}

impl std::error::Error for DbError {}

impl From<rusqlite::Error> for DbError {
    fn from(err: rusqlite::Error) -> Self {
        DbError::Connection(err)
    }
}

impl From<refinery::Error> for DbError {
    fn from(err: refinery::Error) -> Self {
        DbError::Migration(err)
    }
}

/// Configure connection with recommended pragmas
fn configure_connection(conn: &Connection) -> Result<(), DbError> {
    conn.execute_batch(
        "PRAGMA foreign_keys = ON;
         PRAGMA journal_mode = WAL;
         PRAGMA synchronous = NORMAL;
         PRAGMA busy_timeout = 5000;",
    )?;
    Ok(())
}

/// Initialize database connection and run migrations
pub fn init_db<P: AsRef<Path>>(db_path: P) -> Result<Connection, DbError> {
    let mut conn = Connection::open(db_path)?;
    configure_connection(&conn)?;
    embedded::migrations::runner().run(&mut conn)?;
    Ok(conn)
}

/// Initialize an in-memory database (useful for testing)
pub fn init_db_memory() -> Result<Connection, DbError> {
    let mut conn = Connection::open_in_memory()?;
    configure_connection(&conn)?;
    embedded::migrations::runner().run(&mut conn)?;
    Ok(conn)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_init_db_memory() {
        let conn = init_db_memory().expect("Failed to initialize in-memory database");

        let tables: Vec<String> = conn
            .prepare("SELECT name FROM sqlite_master WHERE type='table' ORDER BY name")
            .unwrap()
            .query_map([], |row| row.get(0))
            .unwrap()
            .filter_map(|r| r.ok())
            .collect();

        assert!(tables.contains(&"users".to_string()));
        assert!(tables.contains(&"movies".to_string()));
        assert!(tables.contains(&"tv_shows".to_string()));
        assert!(tables.contains(&"episodes".to_string()));
        assert!(tables.contains(&"artists".to_string()));
        assert!(tables.contains(&"albums".to_string()));
        assert!(tables.contains(&"tracks".to_string()));
        assert!(tables.contains(&"indexers".to_string()));
        assert!(tables.contains(&"downloads".to_string()));
        assert!(tables.contains(&"activity".to_string()));
        assert!(tables.contains(&"sessions".to_string()));
    }

    #[test]
    fn test_default_indexers() {
        let conn = init_db_memory().expect("Failed to initialize in-memory database");

        let count: i32 = conn
            .query_row("SELECT COUNT(*) FROM indexers", [], |row| row.get(0))
            .unwrap();

        assert_eq!(count, 4);
    }

    #[test]
    fn test_foreign_keys_enabled() {
        let conn = init_db_memory().expect("Failed to initialize in-memory database");

        let fk_enabled: i32 = conn
            .query_row("PRAGMA foreign_keys", [], |row| row.get(0))
            .unwrap();

        assert_eq!(fk_enabled, 1, "Foreign keys should be enabled");
    }
}
