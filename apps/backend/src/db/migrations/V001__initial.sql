-- Users table
CREATE TABLE users (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    username TEXT UNIQUE NOT NULL,
    password_hash TEXT NOT NULL,
    role TEXT NOT NULL CHECK (role IN ('admin', 'user')),
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE TRIGGER users_updated_at AFTER UPDATE ON users BEGIN
    UPDATE users SET updated_at = datetime('now') WHERE id = NEW.id;
END;

-- Movies table
CREATE TABLE movies (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    tmdb_id INTEGER UNIQUE NOT NULL,
    imdb_id TEXT,
    title TEXT NOT NULL,
    original_title TEXT,
    year INTEGER NOT NULL,
    overview TEXT,
    poster_path TEXT,
    backdrop_path TEXT,
    runtime_minutes INTEGER,
    genres TEXT,
    status TEXT NOT NULL DEFAULT 'missing'
        CHECK (status IN ('missing', 'searching', 'downloading', 'processing', 'available')),
    monitored INTEGER NOT NULL DEFAULT 1,
    quality_limit TEXT DEFAULT '1080p',
    file_path TEXT,
    file_size INTEGER,
    added_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now')),
    added_by INTEGER REFERENCES users(id) ON DELETE SET NULL
);

CREATE INDEX idx_movies_status ON movies(status);
CREATE INDEX idx_movies_added_by ON movies(added_by);

CREATE TRIGGER movies_updated_at AFTER UPDATE ON movies BEGIN
    UPDATE movies SET updated_at = datetime('now') WHERE id = NEW.id;
END;

-- Movies FTS virtual table
CREATE VIRTUAL TABLE movies_fts USING fts5(
    title, original_title, overview,
    content='movies', content_rowid='id'
);

-- Movies FTS triggers
CREATE TRIGGER movies_ai AFTER INSERT ON movies BEGIN
    INSERT INTO movies_fts(rowid, title, original_title, overview)
    VALUES (NEW.id, NEW.title, COALESCE(NEW.original_title, ''), COALESCE(NEW.overview, ''));
END;

CREATE TRIGGER movies_ad AFTER DELETE ON movies BEGIN
    INSERT INTO movies_fts(movies_fts, rowid, title, original_title, overview)
    VALUES ('delete', OLD.id, OLD.title, COALESCE(OLD.original_title, ''), COALESCE(OLD.overview, ''));
END;

CREATE TRIGGER movies_au_fts AFTER UPDATE ON movies BEGIN
    INSERT INTO movies_fts(movies_fts, rowid, title, original_title, overview)
    VALUES ('delete', OLD.id, OLD.title, COALESCE(OLD.original_title, ''), COALESCE(OLD.overview, ''));
    INSERT INTO movies_fts(rowid, title, original_title, overview)
    VALUES (NEW.id, NEW.title, COALESCE(NEW.original_title, ''), COALESCE(NEW.overview, ''));
END;

-- TV Shows table
CREATE TABLE tv_shows (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    tmdb_id INTEGER UNIQUE NOT NULL,
    imdb_id TEXT,
    title TEXT NOT NULL,
    original_title TEXT,
    year_start INTEGER,
    year_end INTEGER,
    overview TEXT,
    poster_path TEXT,
    backdrop_path TEXT,
    status TEXT NOT NULL DEFAULT 'continuing'
        CHECK (status IN ('continuing', 'ended', 'canceled', 'upcoming')),
    monitored INTEGER NOT NULL DEFAULT 1,
    quality_limit TEXT DEFAULT '1080p',
    added_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now')),
    added_by INTEGER REFERENCES users(id) ON DELETE SET NULL
);

CREATE INDEX idx_tv_shows_status ON tv_shows(status);
CREATE INDEX idx_tv_shows_added_by ON tv_shows(added_by);

CREATE TRIGGER tv_shows_updated_at AFTER UPDATE ON tv_shows BEGIN
    UPDATE tv_shows SET updated_at = datetime('now') WHERE id = NEW.id;
END;

-- TV Shows FTS virtual table
CREATE VIRTUAL TABLE tv_shows_fts USING fts5(
    title, original_title, overview,
    content='tv_shows', content_rowid='id'
);

-- TV Shows FTS triggers
CREATE TRIGGER tv_shows_ai AFTER INSERT ON tv_shows BEGIN
    INSERT INTO tv_shows_fts(rowid, title, original_title, overview)
    VALUES (NEW.id, NEW.title, COALESCE(NEW.original_title, ''), COALESCE(NEW.overview, ''));
END;

CREATE TRIGGER tv_shows_ad AFTER DELETE ON tv_shows BEGIN
    INSERT INTO tv_shows_fts(tv_shows_fts, rowid, title, original_title, overview)
    VALUES ('delete', OLD.id, OLD.title, COALESCE(OLD.original_title, ''), COALESCE(OLD.overview, ''));
END;

CREATE TRIGGER tv_shows_au_fts AFTER UPDATE ON tv_shows BEGIN
    INSERT INTO tv_shows_fts(tv_shows_fts, rowid, title, original_title, overview)
    VALUES ('delete', OLD.id, OLD.title, COALESCE(OLD.original_title, ''), COALESCE(OLD.overview, ''));
    INSERT INTO tv_shows_fts(rowid, title, original_title, overview)
    VALUES (NEW.id, NEW.title, COALESCE(NEW.original_title, ''), COALESCE(NEW.overview, ''));
END;

-- Episodes table
CREATE TABLE episodes (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    show_id INTEGER NOT NULL REFERENCES tv_shows(id) ON DELETE CASCADE,
    tmdb_id INTEGER,
    season_number INTEGER NOT NULL,
    episode_number INTEGER NOT NULL,
    title TEXT,
    overview TEXT,
    air_date TEXT,
    runtime_minutes INTEGER,
    still_path TEXT,
    status TEXT NOT NULL DEFAULT 'missing'
        CHECK (status IN ('missing', 'searching', 'downloading', 'processing', 'available')),
    monitored INTEGER NOT NULL DEFAULT 1,
    file_path TEXT,
    file_size INTEGER,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now')),
    UNIQUE(show_id, season_number, episode_number)
);

CREATE INDEX idx_episodes_show ON episodes(show_id);
CREATE INDEX idx_episodes_status ON episodes(status);

CREATE TRIGGER episodes_updated_at AFTER UPDATE ON episodes BEGIN
    UPDATE episodes SET updated_at = datetime('now') WHERE id = NEW.id;
END;

-- Indexers table
CREATE TABLE indexers (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    name TEXT NOT NULL,
    indexer_type TEXT NOT NULL,
    url TEXT NOT NULL,
    api_key TEXT,
    enabled INTEGER NOT NULL DEFAULT 1,
    priority INTEGER NOT NULL DEFAULT 50,
    categories TEXT,
    last_check TEXT,
    last_error TEXT,
    created_at TEXT NOT NULL DEFAULT (datetime('now'))
);

-- Default indexers
INSERT INTO indexers (name, indexer_type, url, priority, categories) VALUES
    ('1337x', 'public', 'https://1337x.to', 50, '["movies", "tv", "music"]'),
    ('EZTV', 'public', 'https://eztv.re', 60, '["tv"]'),
    ('YTS', 'public', 'https://yts.mx', 40, '["movies"]');

-- Downloads table
CREATE TABLE downloads (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    info_hash TEXT UNIQUE NOT NULL,
    name TEXT NOT NULL,
    media_type TEXT NOT NULL CHECK (media_type IN ('movie', 'episode', 'album', 'track')),
    media_id INTEGER NOT NULL,
    magnet TEXT NOT NULL,
    status TEXT NOT NULL
        CHECK (status IN ('queued', 'downloading', 'seeding', 'processing', 'completed', 'failed', 'paused')),
    progress REAL NOT NULL DEFAULT 0 CHECK (progress >= 0 AND progress <= 100),
    download_speed INTEGER DEFAULT 0 CHECK (download_speed >= 0),
    upload_speed INTEGER DEFAULT 0 CHECK (upload_speed >= 0),
    size_bytes INTEGER CHECK (size_bytes >= 0),
    downloaded_bytes INTEGER DEFAULT 0 CHECK (downloaded_bytes >= 0),
    uploaded_bytes INTEGER DEFAULT 0 CHECK (uploaded_bytes >= 0),
    ratio REAL DEFAULT 0 CHECK (ratio >= 0),
    peers INTEGER DEFAULT 0 CHECK (peers >= 0),
    error_message TEXT,
    added_at TEXT NOT NULL DEFAULT (datetime('now')),
    started_at TEXT,
    completed_at TEXT
);

CREATE INDEX idx_downloads_status ON downloads(status);
CREATE INDEX idx_downloads_media ON downloads(media_type, media_id);

-- Activity log table
CREATE TABLE activity (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    event_type TEXT NOT NULL,
    message TEXT NOT NULL,
    media_type TEXT,
    media_id INTEGER,
    download_id INTEGER,
    user_id INTEGER REFERENCES users(id) ON DELETE SET NULL,
    metadata TEXT,
    created_at TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE INDEX idx_activity_type ON activity(event_type);
CREATE INDEX idx_activity_created ON activity(created_at);
CREATE INDEX idx_activity_user ON activity(user_id);

-- Sessions table
CREATE TABLE sessions (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    user_id INTEGER NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    token_hash TEXT UNIQUE NOT NULL,
    expires_at TEXT NOT NULL,
    created_at TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE INDEX idx_sessions_user ON sessions(user_id);
CREATE INDEX idx_sessions_expires ON sessions(expires_at);
