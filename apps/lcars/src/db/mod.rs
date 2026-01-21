//! Database module for the LCARS application.
//!
//! Provides database initialization, migrations, and models.

#![allow(dead_code)]

use rusqlite::Connection;
use std::path::Path;
use thiserror::Error;

pub mod models;
pub mod queries;

mod embedded {
    use refinery::embed_migrations;
    embed_migrations!("src/db/migrations");
}

#[derive(Error, Debug)]
pub enum DbError {
    #[error("Database connection error: {0}")]
    Connection(#[from] rusqlite::Error),

    #[error("Migration error: {0}")]
    Migration(#[from] refinery::Error),
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

    #[test]
    fn test_downloads_table_has_multi_source_columns() {
        let conn = init_db_memory().expect("Failed to initialize in-memory database");

        // Get column info for downloads table
        let mut stmt = conn
            .prepare("PRAGMA table_info(downloads)")
            .expect("Failed to prepare statement");

        let columns: Vec<String> = stmt
            .query_map([], |row| row.get::<_, String>(1))
            .unwrap()
            .filter_map(|r| r.ok())
            .collect();

        // V004 migration columns
        assert!(
            columns.contains(&"source_type".to_string()),
            "downloads should have source_type column"
        );
        assert!(
            columns.contains(&"source_id".to_string()),
            "downloads should have source_id column (renamed from info_hash)"
        );
        assert!(
            columns.contains(&"source_uri".to_string()),
            "downloads should have source_uri column (renamed from magnet)"
        );
        assert!(
            columns.contains(&"soulseek_username".to_string()),
            "downloads should have soulseek_username column"
        );
        assert!(
            columns.contains(&"soulseek_filename".to_string()),
            "downloads should have soulseek_filename column"
        );
        assert!(
            columns.contains(&"queue_position".to_string()),
            "downloads should have queue_position column"
        );
    }

    #[test]
    fn test_downloads_source_type_index_exists() {
        let conn = init_db_memory().expect("Failed to initialize in-memory database");

        let indexes: Vec<String> = conn
            .prepare("SELECT name FROM sqlite_master WHERE type='index' AND tbl_name='downloads'")
            .unwrap()
            .query_map([], |row| row.get(0))
            .unwrap()
            .filter_map(|r| r.ok())
            .collect();

        assert!(
            indexes.contains(&"idx_downloads_source_type".to_string()),
            "should have index on source_type"
        );
        assert!(
            indexes.contains(&"idx_downloads_source_unique".to_string()),
            "should have unique index on (source_type, source_id)"
        );
    }

    #[test]
    fn test_can_insert_torrent_download() {
        let conn = init_db_memory().expect("Failed to initialize in-memory database");

        // Insert a torrent download
        conn.execute(
            "INSERT INTO downloads (source_type, source_id, source_uri, name, status, size_bytes, downloaded_bytes, media_type, media_id)
             VALUES ('torrent', 'abc123hash', 'magnet:?xt=urn:btih:abc123', 'Test Movie', 'downloading', 1000000, 0, 'movie', 1)",
            [],
        )
        .expect("Should be able to insert torrent download");

        let count: i32 = conn
            .query_row(
                "SELECT COUNT(*) FROM downloads WHERE source_type = 'torrent'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(count, 1);
    }

    #[test]
    fn test_can_insert_soulseek_download() {
        let conn = init_db_memory().expect("Failed to initialize in-memory database");

        // Insert a Soulseek download
        conn.execute(
            "INSERT INTO downloads (source_type, source_id, source_uri, name, status, size_bytes, downloaded_bytes, media_type, media_id, soulseek_username, soulseek_filename, queue_position)
             VALUES ('soulseek', 'slsk-12345', 'soulseek://user/path/file.flac', 'Test Album Track', 'queued', 50000000, 0, 'track', 1, 'someuser', '/Music/Artist/Album/track.flac', 5)",
            [],
        )
        .expect("Should be able to insert Soulseek download");

        let (username, filename, queue_pos): (Option<String>, Option<String>, Option<i32>) = conn
            .query_row(
                "SELECT soulseek_username, soulseek_filename, queue_position FROM downloads WHERE source_type = 'soulseek'",
                [],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .unwrap();

        assert_eq!(username, Some("someuser".to_string()));
        assert_eq!(filename, Some("/Music/Artist/Album/track.flac".to_string()));
        assert_eq!(queue_pos, Some(5));
    }

    #[test]
    fn test_source_id_unique_constraint() {
        let conn = init_db_memory().expect("Failed to initialize in-memory database");

        // Insert first download
        conn.execute(
            "INSERT INTO downloads (source_type, source_id, source_uri, name, status, size_bytes, downloaded_bytes, media_type, media_id)
             VALUES ('torrent', 'uniquehash', 'magnet:?test', 'Test 1', 'downloading', 1000, 0, 'movie', 1)",
            [],
        )
        .expect("Should be able to insert first download");

        // Try to insert duplicate source_id - should fail (inherited UNIQUE constraint from V001)
        // Note: The original info_hash column had UNIQUE constraint which persists after rename
        let result = conn.execute(
            "INSERT INTO downloads (source_type, source_id, source_uri, name, status, size_bytes, downloaded_bytes, media_type, media_id)
             VALUES ('torrent', 'uniquehash', 'magnet:?test2', 'Test 2', 'downloading', 1000, 0, 'movie', 2)",
            [],
        );

        assert!(result.is_err(), "Duplicate source_id should be rejected");

        // Different source_id works fine
        conn.execute(
            "INSERT INTO downloads (source_type, source_id, source_uri, name, status, size_bytes, downloaded_bytes, media_type, media_id)
             VALUES ('soulseek', 'slsk-different', 'soulseek://test', 'Test 3', 'downloading', 1000, 0, 'track', 1)",
            [],
        )
        .expect("Different source_id should be allowed");
    }
}
