-- Multi-source downloads support
-- Extends the downloads table to support both torrent and Soulseek sources

-- Add source type to distinguish download engines
ALTER TABLE downloads ADD COLUMN source_type TEXT NOT NULL DEFAULT 'torrent';

-- Rename torrent-specific columns to be more generic
-- info_hash becomes source_id (torrent hash or soulseek transfer id)
ALTER TABLE downloads RENAME COLUMN info_hash TO source_id;

-- magnet becomes source_uri (magnet link or soulseek://user/path)
ALTER TABLE downloads RENAME COLUMN magnet TO source_uri;

-- Add Soulseek-specific columns (nullable for torrents)
ALTER TABLE downloads ADD COLUMN soulseek_username TEXT;
ALTER TABLE downloads ADD COLUMN soulseek_filename TEXT;
ALTER TABLE downloads ADD COLUMN queue_position INTEGER;

-- Create index for source type filtering
CREATE INDEX idx_downloads_source_type ON downloads(source_type);

-- Note: SQLite doesn't support dropping indexes and recreating with different names
-- in ALTER TABLE. The existing idx_downloads_info_hash will continue to work
-- since it's the same column, just renamed.

-- Create unique constraint for source type + source id
-- SQLite doesn't support CREATE UNIQUE INDEX IF NOT EXISTS directly,
-- so we'll create a new unique index with the proper naming
CREATE UNIQUE INDEX idx_downloads_source_unique ON downloads(source_type, source_id);
