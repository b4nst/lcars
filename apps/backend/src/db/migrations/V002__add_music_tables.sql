-- Artists table
CREATE TABLE artists (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    mbid TEXT UNIQUE NOT NULL,
    name TEXT NOT NULL,
    sort_name TEXT,
    disambiguation TEXT,
    artist_type TEXT,
    country TEXT,
    begin_date TEXT,
    end_date TEXT,
    overview TEXT,
    image_path TEXT,
    monitored INTEGER NOT NULL DEFAULT 1,
    quality_limit TEXT DEFAULT 'flac',
    added_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now')),
    added_by INTEGER REFERENCES users(id)
);

-- Artists FTS virtual table
CREATE VIRTUAL TABLE artists_fts USING fts5(
    name, sort_name, disambiguation, overview,
    content='artists', content_rowid='id'
);

-- Artists FTS triggers
CREATE TRIGGER artists_ai AFTER INSERT ON artists BEGIN
    INSERT INTO artists_fts(rowid, name, sort_name, disambiguation, overview)
    VALUES (new.id, new.name, new.sort_name, new.disambiguation, new.overview);
END;

CREATE TRIGGER artists_ad AFTER DELETE ON artists BEGIN
    INSERT INTO artists_fts(artists_fts, rowid, name, sort_name, disambiguation, overview)
    VALUES ('delete', old.id, old.name, old.sort_name, old.disambiguation, old.overview);
END;

CREATE TRIGGER artists_au AFTER UPDATE ON artists BEGIN
    INSERT INTO artists_fts(artists_fts, rowid, name, sort_name, disambiguation, overview)
    VALUES ('delete', old.id, old.name, old.sort_name, old.disambiguation, old.overview);
    INSERT INTO artists_fts(rowid, name, sort_name, disambiguation, overview)
    VALUES (new.id, new.name, new.sort_name, new.disambiguation, new.overview);
END;

-- Albums table
CREATE TABLE albums (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    mbid TEXT UNIQUE NOT NULL,
    artist_id INTEGER NOT NULL REFERENCES artists(id) ON DELETE CASCADE,
    title TEXT NOT NULL,
    album_type TEXT,
    release_date TEXT,
    overview TEXT,
    cover_path TEXT,
    total_tracks INTEGER,
    status TEXT NOT NULL DEFAULT 'missing'
        CHECK (status IN ('missing', 'searching', 'downloading', 'processing', 'partial', 'available')),
    monitored INTEGER NOT NULL DEFAULT 1,
    quality_limit TEXT DEFAULT 'flac',
    added_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now'))
);

-- Albums FTS virtual table
CREATE VIRTUAL TABLE albums_fts USING fts5(
    title, overview,
    content='albums', content_rowid='id'
);

-- Albums FTS triggers
CREATE TRIGGER albums_ai AFTER INSERT ON albums BEGIN
    INSERT INTO albums_fts(rowid, title, overview)
    VALUES (new.id, new.title, new.overview);
END;

CREATE TRIGGER albums_ad AFTER DELETE ON albums BEGIN
    INSERT INTO albums_fts(albums_fts, rowid, title, overview)
    VALUES ('delete', old.id, old.title, old.overview);
END;

CREATE TRIGGER albums_au AFTER UPDATE ON albums BEGIN
    INSERT INTO albums_fts(albums_fts, rowid, title, overview)
    VALUES ('delete', old.id, old.title, old.overview);
    INSERT INTO albums_fts(rowid, title, overview)
    VALUES (new.id, new.title, new.overview);
END;

CREATE INDEX idx_albums_artist ON albums(artist_id);
CREATE INDEX idx_albums_status ON albums(status);

-- Tracks table
CREATE TABLE tracks (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    mbid TEXT,
    album_id INTEGER NOT NULL REFERENCES albums(id) ON DELETE CASCADE,
    artist_id INTEGER REFERENCES artists(id),
    title TEXT NOT NULL,
    track_number INTEGER NOT NULL,
    disc_number INTEGER DEFAULT 1,
    duration_ms INTEGER,
    status TEXT NOT NULL DEFAULT 'missing'
        CHECK (status IN ('missing', 'searching', 'downloading', 'processing', 'available')),
    monitored INTEGER NOT NULL DEFAULT 1,
    file_path TEXT,
    file_size INTEGER,
    audio_format TEXT,
    bitrate INTEGER,
    sample_rate INTEGER,
    bit_depth INTEGER,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now')),
    UNIQUE(album_id, disc_number, track_number)
);

CREATE INDEX idx_tracks_album ON tracks(album_id);
CREATE INDEX idx_tracks_artist ON tracks(artist_id);
CREATE INDEX idx_tracks_status ON tracks(status);

-- Add Rutracker to indexers
INSERT INTO indexers (name, indexer_type, url, priority, categories) VALUES
    ('Rutracker', 'public', 'https://rutracker.org', 70, '["music"]');
