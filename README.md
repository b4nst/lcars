# LCARS - Library Computer Access and Retrieval System

> A lightweight, self-hosted media collection manager with integrated torrent downloading and VPN-secured traffic. Single binary, ARM-first, Star Trek LCARS-inspired interface.

---

## Project Overview

LCARS combines the core functionality of Radarr/Sonarr into a single, efficient application:

- **Media Management**: Track and organize movies, TV series and music
- **Automated Search**: Find releases across public indexers
- **Torrent Integration**: Built-in download client with VPN interface binding
- **Post-Processing**: Rename, move, copy files to configured destinations
- **Optional Transcoding**: FFmpeg integration for format harmonization

### Design Principles

1. **Minimal Resource Usage**: Rust backend, client-side SPA, SQLite database
2. **Single Binary Distribution**: Embedded frontend, simple deployment
3. **ARM-First**: Primary target is ARM64 (Raspberry Pi, ARM VPS)
4. **LCARS Aesthetic**: Retro-futuristic Star Trek interface theme

### User Roles

- **Admin**: Full access including user management, indexer config, system settings
- **User**: Can browse library, add media, trigger downloads

---

## Technical Stack

### Backend (Rust)

| Component | Crate | Purpose |
|-----------|-------|---------|
| HTTP Framework | `axum` | Async web framework |
| Database | `rusqlite` | SQLite with bundled features |
| Migrations | `refinery` | SQL migration management |
| Async Runtime | `tokio` | Async executor |
| BitTorrent | `librqbit` | Pure Rust torrent client |
| HTTP Client | `reqwest` | External API calls |
| Config | `config` | TOML configuration |
| Auth | `argon2` + `jsonwebtoken` | Password hashing, JWT |
| FFmpeg | `ffmpeg-sidecar` | Spawn ffmpeg processes |
| Logging | `tracing` + `tracing-subscriber` | Structured logging |
| Serialization | `serde` + `serde_json` | JSON handling |
| Embed Assets | `rust-embed` | Compile frontend into binary |
| Cron | `tokio-cron-scheduler` | Background jobs |

### Frontend (Next.js)

| Component | Package | Purpose |
|-----------|---------|---------|
| Framework | `next` (v14+) | Static export SPA |
| State | `zustand` | Lightweight state management |
| Data Fetching | `@tanstack/react-query` | Caching, background sync |
| UI Components | `shadcn/ui` | Accessible component primitives |
| Styling | `tailwindcss` | Utility-first CSS |
| Icons | `lucide-react` | Icon library |
| WebSocket | Native + `reconnecting-websocket` | Real-time updates |

### Build Tooling

| Tool | Purpose |
|------|---------|
| Moon | Monorepo task orchestration |
| Nix | Development environment |
| GitHub Actions | CI/CD, release builds |
| Cross | Cross-compilation for ARM |

### Build Targets (Priority Order)

1. `aarch64-unknown-linux-musl` (ARM64 Linux)
2. `x86_64-unknown-linux-musl` (x64 Linux)
3. `aarch64-apple-darwin` (Apple Silicon)
4. `x86_64-apple-darwin` (Intel Mac)

---

## Repository Structure

```
lcars/
├── .github/
│   └── workflows/
│       ├── ci.yml
│       └── release.yml
├── .moon/
│   ├── workspace.yml
│   └── toolchain.yml
├── apps/
│   ├── backend/                    # Rust backend binary
│   │   ├── Cargo.toml
│   │   ├── build.rs
│   │   ├── moon.yml
│   │   └── src/
│   │       ├── main.rs
│   │       ├── config.rs
│   │       ├── error.rs
│   │       ├── db/
│   │       │   ├── mod.rs
│   │       │   ├── migrations/
│   │       │   │   └── V001__initial.sql
│   │       │   ├── models.rs
│   │       │   └── queries.rs
│   │       ├── api/
│   │       │   ├── mod.rs
│   │       │   ├── auth.rs
│   │       │   ├── movies.rs
│   │       │   ├── tv.rs
│   │       │   ├── music.rs
│   │       │   ├── downloads.rs
│   │       │   ├── search.rs
│   │       │   ├── system.rs
│   │       │   └── ws.rs
│   │       └── services/
│   │           ├── mod.rs
│   │           ├── tmdb.rs
│   │           ├── musicbrainz.rs
│   │           ├── indexer/
│   │           │   ├── mod.rs
│   │           │   ├── parser.rs
│   │           │   └── providers/
│   │           │       ├── mod.rs
│   │           │       ├── leetx.rs
│   │           │       ├── eztv.rs
│   │           │       └── rutracker.rs
│   │           ├── torrent.rs
│   │           ├── storage/
│   │           │   ├── mod.rs
│   │           │   ├── local.rs
│   │           │   └── smb.rs
│   │           ├── media.rs
│   │           └── scheduler.rs
│   └── web/                      # Next.js frontend
│       ├── package.json
│       ├── moon.yml
│       ├── next.config.js
│       ├── tailwind.config.js
│       ├── tsconfig.json
│       ├── app/
│       │   ├── layout.tsx
│       │   ├── page.tsx
│       │   ├── movies/
│       │   │   ├── page.tsx
│       │   │   └── [id]/page.tsx
│       │   ├── tv/
│       │   │   ├── page.tsx
│       │   │   └── [id]/page.tsx
│       │   ├── music/
│       │   │   ├── page.tsx
│       │   │   ├── artists/
│       │   │   │   └── [id]/page.tsx
│       │   │   └── albums/
│       │   │       └── [id]/page.tsx
│       │   ├── downloads/
│       │   │   └── page.tsx
│       │   └── settings/
│       │       └── page.tsx
│       ├── components/
│       │   ├── ui/              # shadcn components
│       │   ├── lcars/           # LCARS-specific components
│       │   │   ├── frame.tsx
│       │   │   ├── button.tsx
│       │   │   ├── panel.tsx
│       │   │   └── sidebar.tsx
│       │   ├── media-card.tsx
│       │   ├── download-item.tsx
│       │   └── search-modal.tsx
│       └── lib/
│           ├── api.ts
│           ├── ws.ts
│           └── stores/
│               ├── auth.ts
│               └── downloads.ts
├── flake.nix
├── flake.lock
├── .gitignore
├── README.md
└── config.example.toml
```

---

## Configuration

### config.toml

```toml
[server]
host = "0.0.0.0"
port = 8080
jwt_secret = "change-me-generate-a-secure-random-string"

[database]
path = "./data/lcars.db"

[tmdb]
api_key = "your-tmdb-api-key"

[musicbrainz]
# No API key required - MusicBrainz is free
# User-Agent is built from app name/version (required by MusicBrainz)
rate_limit_ms = 1000  # MusicBrainz requires max 1 request/second

[torrent]
download_dir = "./downloads"
bind_interface = ""  # VPN interface name, empty for default
max_connections = 100
port_range = [6881, 6889]

[torrent.seeding]
enabled = true
ratio_limit = 1.0
time_limit_hours = 48

[[storage.mounts]]
name = "local"
type = "local"
path = "/media/library"
enabled = true

[[storage.mounts]]
name = "nas"
type = "smb"
host = "192.168.1.100"
share = "media"
username = "user"
password = "pass"
mount_point = "/mnt/nas"
enabled = false

[storage.naming]
movie_pattern = "movie/{title} ({year})/{title} ({year}) - {quality}.{ext}"
tv_pattern = "tv/{title}/S{season:02}/{title} - S{season:02}E{episode:02} - {episode_title}.{ext}"
music_pattern = "music/{artist}/{album}/{title}.{ext}"

[[storage.rules]]
action = "move"
destination = "local"
media_types = ["movie", "episode", "album"]

[[storage.rules]]
action = "copy"
destination = "nas"
media_types = ["movie"]

[scheduler]
search_missing = "0 0 */6 * * *"
refresh_metadata = "0 0 2 * * *"
check_new_episodes = "0 0 */12 * * *"
check_new_releases = "0 0 3 * * *"  # Check for new albums from monitored artists
cleanup_completed = "0 0 * * * *"
```

---

## Database Schema

### migrations/V001__initial.sql

```sql
-- Users
CREATE TABLE users (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    username TEXT UNIQUE NOT NULL,
    password_hash TEXT NOT NULL,
    role TEXT NOT NULL CHECK (role IN ('admin', 'user')),
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now'))
);

-- Movies
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
    added_by INTEGER REFERENCES users(id)
);

CREATE VIRTUAL TABLE movies_fts USING fts5(
    title, original_title, overview,
    content='movies', content_rowid='id'
);

CREATE TRIGGER movies_ai AFTER INSERT ON movies BEGIN
    INSERT INTO movies_fts(rowid, title, original_title, overview)
    VALUES (new.id, new.title, new.original_title, new.overview);
END;

CREATE TRIGGER movies_ad AFTER DELETE ON movies BEGIN
    INSERT INTO movies_fts(movies_fts, rowid, title, original_title, overview)
    VALUES ('delete', old.id, old.title, old.original_title, old.overview);
END;

CREATE TRIGGER movies_au AFTER UPDATE ON movies BEGIN
    INSERT INTO movies_fts(movies_fts, rowid, title, original_title, overview)
    VALUES ('delete', old.id, old.title, old.original_title, old.overview);
    INSERT INTO movies_fts(rowid, title, original_title, overview)
    VALUES (new.id, new.title, new.original_title, new.overview);
END;

-- TV Shows
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
    added_by INTEGER REFERENCES users(id)
);

CREATE VIRTUAL TABLE tv_shows_fts USING fts5(
    title, original_title, overview,
    content='tv_shows', content_rowid='id'
);

CREATE TRIGGER tv_shows_ai AFTER INSERT ON tv_shows BEGIN
    INSERT INTO tv_shows_fts(rowid, title, original_title, overview)
    VALUES (new.id, new.title, new.original_title, new.overview);
END;

CREATE TRIGGER tv_shows_ad AFTER DELETE ON tv_shows BEGIN
    INSERT INTO tv_shows_fts(tv_shows_fts, rowid, title, original_title, overview)
    VALUES ('delete', old.id, old.title, old.original_title, old.overview);
END;

CREATE TRIGGER tv_shows_au AFTER UPDATE ON tv_shows BEGIN
    INSERT INTO tv_shows_fts(tv_shows_fts, rowid, title, original_title, overview)
    VALUES ('delete', old.id, old.title, old.original_title, old.overview);
    INSERT INTO tv_shows_fts(rowid, title, original_title, overview)
    VALUES (new.id, new.title, new.original_title, new.overview);
END;

-- Episodes
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

-- Artists
CREATE TABLE artists (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    mbid TEXT UNIQUE NOT NULL,           -- MusicBrainz artist ID
    name TEXT NOT NULL,
    sort_name TEXT,
    disambiguation TEXT,                  -- e.g., "UK rock band"
    artist_type TEXT,                     -- person, group, orchestra, choir, etc.
    country TEXT,                         -- ISO 3166-1 alpha-2
    begin_date TEXT,                      -- Formation/birth date
    end_date TEXT,                        -- Dissolution/death date
    overview TEXT,
    image_path TEXT,
    monitored INTEGER NOT NULL DEFAULT 1,
    quality_limit TEXT DEFAULT 'flac',
    added_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now')),
    added_by INTEGER REFERENCES users(id)
);

CREATE VIRTUAL TABLE artists_fts USING fts5(
    name, sort_name, disambiguation, overview,
    content='artists', content_rowid='id'
);

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

-- Albums (Release Groups in MusicBrainz terminology)
CREATE TABLE albums (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    mbid TEXT UNIQUE NOT NULL,           -- MusicBrainz release group ID
    artist_id INTEGER NOT NULL REFERENCES artists(id) ON DELETE CASCADE,
    title TEXT NOT NULL,
    album_type TEXT,                      -- album, single, EP, compilation, soundtrack, etc.
    release_date TEXT,                    -- First release date
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

CREATE VIRTUAL TABLE albums_fts USING fts5(
    title, overview,
    content='albums', content_rowid='id'
);

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

-- Tracks
CREATE TABLE tracks (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    mbid TEXT,                            -- MusicBrainz recording ID
    album_id INTEGER NOT NULL REFERENCES albums(id) ON DELETE CASCADE,
    artist_id INTEGER REFERENCES artists(id), -- For tracks with different artists
    title TEXT NOT NULL,
    track_number INTEGER NOT NULL,
    disc_number INTEGER DEFAULT 1,
    duration_ms INTEGER,
    status TEXT NOT NULL DEFAULT 'missing'
        CHECK (status IN ('missing', 'searching', 'downloading', 'processing', 'available')),
    monitored INTEGER NOT NULL DEFAULT 1,
    file_path TEXT,
    file_size INTEGER,
    audio_format TEXT,                    -- flac, mp3, aac, etc.
    bitrate INTEGER,                      -- kbps
    sample_rate INTEGER,                  -- Hz
    bit_depth INTEGER,                    -- 16, 24, etc.
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now')),
    UNIQUE(album_id, disc_number, track_number)
);

CREATE INDEX idx_tracks_album ON tracks(album_id);
CREATE INDEX idx_tracks_artist ON tracks(artist_id);
CREATE INDEX idx_tracks_status ON tracks(status);

-- Indexers
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

INSERT INTO indexers (name, indexer_type, url, priority, categories) VALUES
    ('1337x', 'public', 'https://1337x.to', 50, '["movies", "tv", "music"]'),
    ('EZTV', 'public', 'https://eztv.re', 60, '["tv"]'),
    ('YTS', 'public', 'https://yts.mx', 40, '["movies"]'),
    ('Rutracker', 'public', 'https://rutracker.org', 70, '["music"]');

-- Downloads
CREATE TABLE downloads (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    info_hash TEXT UNIQUE NOT NULL,
    name TEXT NOT NULL,
    media_type TEXT NOT NULL CHECK (media_type IN ('movie', 'episode', 'album', 'track')),
    media_id INTEGER NOT NULL,
    magnet TEXT NOT NULL,
    status TEXT NOT NULL
        CHECK (status IN ('queued', 'downloading', 'seeding', 'processing', 'completed', 'failed', 'paused')),
    progress REAL NOT NULL DEFAULT 0,
    download_speed INTEGER DEFAULT 0,
    upload_speed INTEGER DEFAULT 0,
    size_bytes INTEGER,
    downloaded_bytes INTEGER DEFAULT 0,
    uploaded_bytes INTEGER DEFAULT 0,
    ratio REAL DEFAULT 0,
    peers INTEGER DEFAULT 0,
    error_message TEXT,
    added_at TEXT NOT NULL DEFAULT (datetime('now')),
    started_at TEXT,
    completed_at TEXT
);

CREATE INDEX idx_downloads_status ON downloads(status);
CREATE INDEX idx_downloads_media ON downloads(media_type, media_id);

-- Activity log
CREATE TABLE activity (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    event_type TEXT NOT NULL,
    message TEXT NOT NULL,
    media_type TEXT,
    media_id INTEGER,
    download_id INTEGER,
    user_id INTEGER REFERENCES users(id),
    metadata TEXT,
    created_at TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE INDEX idx_activity_type ON activity(event_type);
CREATE INDEX idx_activity_created ON activity(created_at);

-- Sessions
CREATE TABLE sessions (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    user_id INTEGER NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    token_hash TEXT UNIQUE NOT NULL,
    expires_at TEXT NOT NULL,
    created_at TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE INDEX idx_sessions_user ON sessions(user_id);
CREATE INDEX idx_sessions_expires ON sessions(expires_at);
```

---

## API Specification

### Authentication

All endpoints except `/api/auth/login` require JWT bearer token:
```
Authorization: Bearer <jwt_token>
```

### Endpoints

#### Auth
```
POST   /api/auth/login           { username, password } -> { token, user }
POST   /api/auth/logout          -> { success }
GET    /api/auth/me              -> User
```

#### Users (admin only)
```
GET    /api/users                -> User[]
POST   /api/users                { username, password, role } -> User
PUT    /api/users/:id            { username?, password?, role? } -> User
DELETE /api/users/:id            -> { success }
```

#### Movies
```
GET    /api/movies               ?status&monitored&search&page&limit -> { items, total, page, pages }
POST   /api/movies               { tmdb_id, monitored?, quality_limit? } -> Movie
GET    /api/movies/:id           -> Movie
PUT    /api/movies/:id           { monitored?, quality_limit? } -> Movie
DELETE /api/movies/:id           ?delete_files -> { success }
POST   /api/movies/:id/search    -> Release[]
POST   /api/movies/:id/download  { release_id | magnet } -> Download
POST   /api/movies/:id/refresh   -> Movie
```

#### TV Shows
```
GET    /api/tv                   ?status&monitored&search&page&limit -> { items, total, page, pages }
POST   /api/tv                   { tmdb_id, monitored?, quality_limit? } -> TvShow
GET    /api/tv/:id               -> TvShow (with seasons/episodes)
PUT    /api/tv/:id               { monitored?, quality_limit? } -> TvShow
DELETE /api/tv/:id               ?delete_files -> { success }
GET    /api/tv/:id/season/:s     -> Episode[]
PUT    /api/tv/:id/season/:s     { monitored } -> Episode[]
PUT    /api/tv/:id/season/:s/episode/:e  { monitored? } -> Episode
POST   /api/tv/:id/season/:s/episode/:e/search -> Release[]
POST   /api/tv/:id/season/:s/episode/:e/download { release_id | magnet } -> Download
POST   /api/tv/:id/refresh       -> TvShow
```

#### Artists
```
GET    /api/artists              ?status&monitored&search&page&limit -> { items, total, page, pages }
POST   /api/artists              { mbid, monitored?, quality_limit? } -> Artist
GET    /api/artists/:id          -> Artist (with albums)
PUT    /api/artists/:id          { monitored?, quality_limit? } -> Artist
DELETE /api/artists/:id          ?delete_files -> { success }
POST   /api/artists/:id/refresh  -> Artist
```

#### Albums
```
GET    /api/albums               ?artist_id&status&monitored&search&page&limit -> { items, total, page, pages }
GET    /api/albums/:id           -> Album (with tracks)
PUT    /api/albums/:id           { monitored?, quality_limit? } -> Album
DELETE /api/albums/:id           ?delete_files -> { success }
POST   /api/albums/:id/search    -> Release[]
POST   /api/albums/:id/download  { release_id | magnet } -> Download
POST   /api/albums/:id/refresh   -> Album
```

#### Tracks
```
GET    /api/tracks               ?album_id&status&page&limit -> { items, total, page, pages }
PUT    /api/tracks/:id           { monitored? } -> Track
POST   /api/tracks/:id/search    -> Release[]
POST   /api/tracks/:id/download  { release_id | magnet } -> Download
```

#### Downloads
```
GET    /api/downloads            ?status -> Download[]
GET    /api/downloads/:id        -> Download
DELETE /api/downloads/:id        ?delete_files -> { success }
POST   /api/downloads/:id/pause  -> Download
POST   /api/downloads/:id/resume -> Download
POST   /api/downloads/:id/retry  -> Download
```

#### Search
```
GET    /api/search/tmdb/movies   ?q&year -> TmdbMovie[]
GET    /api/search/tmdb/tv       ?q -> TmdbTvShow[]
GET    /api/search/musicbrainz/artists ?q -> MusicBrainzArtist[]
GET    /api/search/musicbrainz/albums  ?q&artist_mbid -> MusicBrainzAlbum[]
GET    /api/search/releases      ?q&type&tmdb_id&imdb_id&mbid&season&episode -> Release[]
```

#### System (admin only)
```
GET    /api/system/status        -> SystemStatus
GET    /api/system/activity      ?type&limit&before -> Activity[]
GET    /api/indexers             -> Indexer[]
POST   /api/indexers             { name, indexer_type, url, api_key?, priority? } -> Indexer
PUT    /api/indexers/:id         { name?, url?, api_key?, enabled?, priority? } -> Indexer
DELETE /api/indexers/:id         -> { success }
POST   /api/indexers/:id/test    -> { success, message?, response_time_ms? }
GET    /api/storage/mounts       -> MountInfo[]
POST   /api/storage/mounts/:name/test -> { success, message?, free_space_bytes? }
```

#### WebSocket
```
GET    /api/ws                   -> WebSocket connection

Messages (server -> client):
- download:added      { Download }
- download:progress   { id, progress, download_speed, upload_speed, peers }
- download:status     { id, status, error_message? }
- download:completed  { id, media_type, media_id }
- media:added         { type, id }
- media:updated       { type, id, status }
- media:deleted       { type, id }
- system:status       { vpn_connected, active_downloads }
```

---

## Data Models

### TypeScript (Frontend)

```typescript
interface User {
  id: number;
  username: string;
  role: 'admin' | 'user';
  created_at: string;
}

interface Movie {
  id: number;
  tmdb_id: number;
  imdb_id?: string;
  title: string;
  original_title?: string;
  year: number;
  overview?: string;
  poster_path?: string;
  backdrop_path?: string;
  runtime_minutes?: number;
  genres: string[];
  status: MediaStatus;
  monitored: boolean;
  quality_limit: string;
  file_path?: string;
  file_size?: number;
  added_at: string;
}

interface TvShow {
  id: number;
  tmdb_id: number;
  imdb_id?: string;
  title: string;
  original_title?: string;
  year_start?: number;
  year_end?: number;
  overview?: string;
  poster_path?: string;
  backdrop_path?: string;
  status: ShowStatus;
  monitored: boolean;
  quality_limit: string;
  seasons: Season[];
  added_at: string;
}

interface Season {
  season_number: number;
  episode_count: number;
  available_count: number;
  episodes: Episode[];
}

interface Episode {
  id: number;
  show_id: number;
  tmdb_id?: number;
  season_number: number;
  episode_number: number;
  title?: string;
  overview?: string;
  air_date?: string;
  runtime_minutes?: number;
  still_path?: string;
  status: MediaStatus;
  monitored: boolean;
  file_path?: string;
  file_size?: number;
}

type MediaStatus = 'missing' | 'searching' | 'downloading' | 'processing' | 'available';
type AlbumStatus = 'missing' | 'searching' | 'downloading' | 'processing' | 'partial' | 'available';
type ShowStatus = 'continuing' | 'ended' | 'canceled' | 'upcoming';

interface Artist {
  id: number;
  mbid: string;
  name: string;
  sort_name?: string;
  disambiguation?: string;
  artist_type?: string;
  country?: string;
  begin_date?: string;
  end_date?: string;
  overview?: string;
  image_path?: string;
  monitored: boolean;
  quality_limit: string;
  albums: Album[];
  added_at: string;
}

interface Album {
  id: number;
  mbid: string;
  artist_id: number;
  title: string;
  album_type?: string;
  release_date?: string;
  overview?: string;
  cover_path?: string;
  total_tracks?: number;
  status: AlbumStatus;
  monitored: boolean;
  quality_limit: string;
  tracks: Track[];
  added_at: string;
}

interface Track {
  id: number;
  mbid?: string;
  album_id: number;
  artist_id?: number;
  title: string;
  track_number: number;
  disc_number: number;
  duration_ms?: number;
  status: MediaStatus;
  monitored: boolean;
  file_path?: string;
  file_size?: number;
  audio_format?: string;
  bitrate?: number;
  sample_rate?: number;
  bit_depth?: number;
}

type AudioQuality = 'flac' | 'alac' | '320' | '256' | '192' | '128' | 'unknown';

interface Download {
  id: number;
  info_hash: string;
  name: string;
  media_type: 'movie' | 'episode' | 'album' | 'track';
  media_id: number;
  status: DownloadStatus;
  progress: number;
  download_speed: number;
  upload_speed: number;
  size_bytes?: number;
  downloaded_bytes: number;
  uploaded_bytes: number;
  ratio: number;
  peers: number;
  error_message?: string;
  added_at: string;
  started_at?: string;
  completed_at?: string;
}

type DownloadStatus = 'queued' | 'downloading' | 'seeding' | 'processing' | 'completed' | 'failed' | 'paused';

interface Release {
  id: string;
  title: string;
  indexer: string;
  magnet: string;
  size_bytes: number;
  seeders: number;
  leechers: number;
  quality: Quality;
  source: Source;
  codec?: string;
  audio?: string;
  group?: string;
  proper: boolean;
  repack: boolean;
  uploaded_at: string;
}

type Quality = '2160p' | '1080p' | '720p' | '480p' | 'unknown';
type Source = 'bluray' | 'webdl' | 'webrip' | 'hdtv' | 'dvd' | 'cam' | 'unknown';

interface Indexer {
  id: number;
  name: string;
  indexer_type: 'public' | 'torznab' | 'rss';
  url: string;
  enabled: boolean;
  priority: number;
  categories: string[];
  last_check?: string;
  last_error?: string;
}

interface SystemStatus {
  version: string;
  uptime_seconds: number;
  database_size_bytes: number;
  downloads: {
    active: number;
    queued: number;
    seeding: number;
  };
  storage: {
    mounts: MountStatus[];
  };
  vpn: {
    enabled: boolean;
    interface: string;
    connected: boolean;
    public_ip?: string;
  };
}

interface MountStatus {
  name: string;
  type: 'local' | 'smb';
  available: boolean;
  total_bytes?: number;
  free_bytes?: number;
  error?: string;
}
```

---

## Service Implementations

### TMDB Service

```rust
// src/services/tmdb.rs

const TMDB_BASE_URL: &str = "https://api.themoviedb.org/3";
const TMDB_IMAGE_BASE: &str = "https://image.tmdb.org/t/p";

pub struct TmdbClient {
    client: reqwest::Client,
    api_key: String,
}

impl TmdbClient {
    pub fn new(api_key: String) -> Self;
    pub async fn search_movies(&self, query: &str, year: Option<i32>) -> Result<Vec<TmdbMovie>>;
    pub async fn search_tv(&self, query: &str) -> Result<Vec<TmdbTvShow>>;
    pub async fn get_movie(&self, id: i32) -> Result<TmdbMovieDetails>;
    pub async fn get_tv(&self, id: i32) -> Result<TmdbTvDetails>;
    pub async fn get_season(&self, show_id: i32, season_number: i32) -> Result<TmdbSeason>;
    pub fn poster_url(&self, path: &str, size: &str) -> String;
}
```

### MusicBrainz Service

```rust
// src/services/musicbrainz.rs

const MB_BASE_URL: &str = "https://musicbrainz.org/ws/2";
const COVER_ART_BASE: &str = "https://coverartarchive.org";

pub struct MusicBrainzClient {
    client: reqwest::Client,
    user_agent: String,  // Required by MusicBrainz API
    rate_limiter: RateLimiter,  // 1 request per second
}

impl MusicBrainzClient {
    pub fn new(app_name: &str, app_version: &str, contact: &str) -> Self;

    // Artist operations
    pub async fn search_artists(&self, query: &str) -> Result<Vec<MbArtist>>;
    pub async fn get_artist(&self, mbid: &str) -> Result<MbArtistDetails>;
    pub async fn get_artist_releases(&self, mbid: &str) -> Result<Vec<MbReleaseGroup>>;

    // Release group (album) operations
    pub async fn search_release_groups(&self, query: &str, artist_mbid: Option<&str>) -> Result<Vec<MbReleaseGroup>>;
    pub async fn get_release_group(&self, mbid: &str) -> Result<MbReleaseGroupDetails>;

    // Release operations (specific editions of albums)
    pub async fn get_releases_for_group(&self, release_group_mbid: &str) -> Result<Vec<MbRelease>>;
    pub async fn get_release(&self, mbid: &str) -> Result<MbReleaseDetails>;  // Includes track list

    // Cover art
    pub async fn get_cover_art(&self, release_mbid: &str) -> Result<Option<CoverArt>>;
    pub fn cover_url(&self, release_mbid: &str, size: &str) -> String;  // size: "250", "500", "1200"
}

#[derive(Debug)]
pub struct MbArtist {
    pub mbid: String,
    pub name: String,
    pub sort_name: String,
    pub disambiguation: Option<String>,
    pub artist_type: Option<String>,  // Person, Group, Orchestra, Choir, Character, Other
    pub country: Option<String>,
    pub life_span: Option<LifeSpan>,
    pub score: u8,  // Search relevance score
}

#[derive(Debug)]
pub struct MbReleaseGroup {
    pub mbid: String,
    pub title: String,
    pub primary_type: Option<String>,  // Album, Single, EP, Broadcast, Other
    pub secondary_types: Vec<String>,  // Compilation, Soundtrack, Spokenword, Interview, etc.
    pub first_release_date: Option<String>,
    pub artist_credit: Vec<ArtistCredit>,
}

#[derive(Debug)]
pub struct MbRelease {
    pub mbid: String,
    pub title: String,
    pub status: Option<String>,  // Official, Promotion, Bootleg, Pseudo-Release
    pub country: Option<String>,
    pub date: Option<String>,
    pub barcode: Option<String>,
    pub media: Vec<MbMedium>,
}

#[derive(Debug)]
pub struct MbMedium {
    pub position: u32,
    pub format: Option<String>,  // CD, Vinyl, Digital Media, etc.
    pub track_count: u32,
    pub tracks: Vec<MbTrack>,
}

#[derive(Debug)]
pub struct MbTrack {
    pub mbid: String,           // Track MBID
    pub recording_mbid: String, // Recording MBID (abstract work)
    pub title: String,
    pub position: u32,
    pub length_ms: Option<u32>,
    pub artist_credit: Vec<ArtistCredit>,
}

#[derive(Debug)]
pub struct ArtistCredit {
    pub mbid: String,
    pub name: String,
    pub join_phrase: Option<String>,  // e.g., " & ", " feat. "
}

// Rate limiting: MusicBrainz requires max 1 request/second
struct RateLimiter {
    last_request: Mutex<Instant>,
}
```

### Indexer Service

```rust
// src/services/indexer/mod.rs

#[async_trait]
pub trait IndexerProvider: Send + Sync {
    fn name(&self) -> &str;
    fn supports_movies(&self) -> bool;
    fn supports_tv(&self) -> bool;
    fn supports_music(&self) -> bool;
    async fn search(&self, query: SearchQuery) -> Result<Vec<Release>>;
    async fn test(&self) -> Result<IndexerTestResult>;
}

pub struct SearchQuery {
    pub query: String,
    pub media_type: MediaSearchType,
    pub imdb_id: Option<String>,
    pub tmdb_id: Option<i32>,
    pub mbid: Option<String>,          // MusicBrainz ID for music searches
    pub year: Option<i32>,
    pub season: Option<i32>,
    pub episode: Option<i32>,
    pub artist: Option<String>,        // Artist name for music searches
    pub album: Option<String>,         // Album name for music searches
}

pub struct IndexerManager {
    providers: Vec<Box<dyn IndexerProvider>>,
}

impl IndexerManager {
    pub fn new() -> Self;  // Initializes with built-in public indexers
    pub async fn search(&self, query: SearchQuery) -> Result<Vec<Release>>;
}

// Built-in providers: 1337x, EZTV, YTS, Rutracker
```

### Release Name Parser

```rust
// src/services/indexer/parser.rs

pub struct ParsedRelease {
    pub title: String,
    pub year: Option<i32>,
    pub season: Option<i32>,
    pub episode: Option<i32>,
    pub quality: Quality,
    pub source: Source,
    pub codec: Option<String>,
    pub audio: Option<String>,
    pub group: Option<String>,
    pub proper: bool,
    pub repack: bool,
    // Music-specific fields
    pub artist: Option<String>,
    pub album: Option<String>,
    pub audio_format: Option<AudioFormat>,
    pub bitrate: Option<u32>,
    pub sample_rate: Option<u32>,
    pub bit_depth: Option<u32>,
}

pub fn parse_release_name(name: &str) -> ParsedRelease;
pub fn parse_music_release(name: &str) -> ParsedRelease;

// Regex patterns for video:
// - Quality: 2160p, 1080p, 720p, 480p
// - Source: BluRay, WEB-DL, WEBRip, HDTV, DVDRip, CAM
// - Season/Episode: S01E01
// - Year: (2024) or .2024.
// - Codec: x264, x265, HEVC
// - Audio: AAC, AC3, DTS, Atmos
// - Group: -GROUP at end
// - Proper/Repack flags

// Regex patterns for music:
// - Format: FLAC, MP3, AAC, ALAC, WAV, OGG
// - Bitrate: 320, V0, V2, 256, 192, 128
// - Sample rate: 44.1kHz, 48kHz, 96kHz, 192kHz
// - Bit depth: 16bit, 24bit
// - Source: CD, WEB, Vinyl
```

### Torrent Service

```rust
// src/services/torrent.rs

pub struct TorrentEngine {
    session: Arc<librqbit::Session>,
    config: TorrentConfig,
    event_tx: broadcast::Sender<TorrentEvent>,
}

pub enum TorrentEvent {
    Added { info_hash: String, name: String },
    Progress { info_hash: String, progress: f64, download_speed: u64, upload_speed: u64, peers: usize },
    Completed { info_hash: String },
    Error { info_hash: String, message: String },
}

pub struct MediaRef {
    pub media_type: MediaType,
    pub media_id: i64,
}

impl TorrentEngine {
    pub async fn new(config: TorrentConfig) -> Result<Self>;
    pub async fn add_magnet(&self, magnet: &str, media_ref: MediaRef) -> Result<String>;
    pub async fn get_status(&self, info_hash: &str) -> Result<TorrentStatus>;
    pub async fn pause(&self, info_hash: &str) -> Result<()>;
    pub async fn resume(&self, info_hash: &str) -> Result<()>;
    pub async fn remove(&self, info_hash: &str, delete_files: bool) -> Result<()>;
    pub fn subscribe(&self) -> broadcast::Receiver<TorrentEvent>;
}

// VPN binding: Configure librqbit to bind to specific network interface
// via config.bind_interface setting
```

### Storage Service

```rust
// src/services/storage/mod.rs

#[async_trait]
pub trait Mount: Send + Sync {
    fn name(&self) -> &str;
    fn mount_type(&self) -> &str;
    async fn available(&self) -> bool;
    async fn free_space(&self) -> Result<u64>;
    async fn exists(&self, path: &Path) -> bool;
    async fn write_file(&self, source: &Path, dest: &Path) -> Result<()>;
    async fn delete_file(&self, path: &Path) -> Result<()>;
}

pub struct LocalMount { root: PathBuf }
pub struct SmbMount { config: SmbConfig }

pub struct StorageManager {
    mounts: HashMap<String, Box<dyn Mount>>,
    rules: Vec<PostDownloadRule>,
    naming: NamingConfig,
}

impl StorageManager {
    pub fn new(mounts_config: Vec<MountConfig>, rules: StorageRules) -> Result<Self>;

    /// Process completed download: find media files, rename, move/copy per rules
    pub async fn process_completed_download(
        &self,
        download_path: &Path,
        media_type: MediaType,
        media_id: i64,
    ) -> Result<PathBuf>;

    async fn generate_movie_name(&self, movie_id: i64) -> Result<String>;
    async fn generate_episode_name(&self, episode_id: i64) -> Result<String>;
    async fn generate_track_name(&self, track_id: i64) -> Result<String>;
}

// Naming patterns support placeholders:
// Movies/TV: {title}, {year}, {quality}, {source}, {codec}, {group}, {ext}
//            {season:02}, {episode:02}, {episode_title}
// Music:     {artist}, {album}, {title}, {track:02}, {disc:02}, {format}, {ext}
```

### Media Processing Service

```rust
// src/services/media.rs

pub struct MediaProcessor {
    ffmpeg_path: String,
    ffprobe_path: String,
}

impl MediaProcessor {
    pub fn new() -> Self;
    pub async fn probe(&self, path: &Path) -> Result<MediaInfo>;
    pub async fn transcode(
        &self,
        input: &Path,
        output: &Path,
        profile: TranscodeProfile,
        progress_tx: mpsc::Sender<f64>,
    ) -> Result<()>;
    pub async fn generate_thumbnail(&self, input: &Path, time_secs: u32) -> Result<Vec<u8>>;
}

pub struct MediaInfo {
    pub duration_seconds: f64,
    pub size_bytes: u64,
    pub video_codec: Option<String>,
    pub audio_codec: Option<String>,
    pub resolution: Option<(u32, u32)>,
    pub container: String,
}
```

### Scheduler Service

```rust
// src/services/scheduler.rs

pub struct Scheduler {
    scheduler: tokio_cron_scheduler::JobScheduler,
}

impl Scheduler {
    pub async fn new(config: SchedulerConfig) -> Result<Self>;
    pub async fn start(&self) -> Result<()>;
}

// Scheduled jobs:
// - search_missing: Search indexers for missing monitored media (movies, episodes, albums)
// - refresh_metadata: Update metadata from TMDB and MusicBrainz
// - check_new_episodes: Check for new episodes of continuing shows
// - check_new_releases: Check for new albums from monitored artists
// - cleanup_completed: Remove downloads that meet seeding requirements
```

---

## Frontend Implementation

### LCARS Design System

#### Color Palette

```css
:root {
  --lcars-orange: #ff9900;
  --lcars-yellow: #ffcc00;
  --lcars-blue: #9999ff;
  --lcars-purple: #cc99cc;
  --lcars-red: #cc6666;
  --lcars-peach: #ffcc99;
  --lcars-tan: #cc9966;
  --lcars-lavender: #9999cc;
  --lcars-black: #000000;
  --lcars-dark: #1a1a2e;
  --lcars-text: #ff9900;
  --lcars-text-dim: #cc7700;
  --status-available: #66cc66;
  --status-missing: #cc6666;
  --status-downloading: #6699cc;
  --status-processing: #cc99cc;
}
```

#### Typography

```css
@import url('https://fonts.googleapis.com/css2?family=Antonio:wght@400;700&display=swap');

body {
  font-family: 'Antonio', 'Helvetica Neue', sans-serif;
  text-transform: uppercase;
  letter-spacing: 0.05em;
}
```

#### Visual Pattern

```
┌─────────────────────────────────────────────────────────────┐
│  ████████████████  ████  ████████████████████████████████  │
│  ██              ██    ██                                ██  │
│  ██   LCARS      ██    ██    PAGE TITLE                 ██  │
│  ██              ██    ██                                ██  │
│  ████████████████      ████████████████████████████████████  │
│                                                             │
│  ┌──────────────────────────────────────────────────────┐  │
│  │                    CONTENT AREA                      │  │
│  └──────────────────────────────────────────────────────┘  │
│                                                             │
│  ████  ████  ████  ████  ████  ████  ████  ████  ████  ██  │
└─────────────────────────────────────────────────────────────┘
```

### Key Components

- `LcarsFrame`: Main layout with top/bottom bars and sidebar
- `LcarsButton`: Rounded pill buttons in LCARS colors
- `LcarsPanel`: Content panels with colored left accent bar
- `LcarsSidebar`: Navigation with animated indicators
- `MediaCard`: Poster card with status overlay (movies, TV, albums)
- `ArtistCard`: Artist card with image and album count
- `TrackList`: Album track listing with status indicators
- `DownloadItem`: Progress bar with speed indicators
- `SearchModal`: TMDB/MusicBrainz search with add functionality

### Pages

- **Dashboard** (`/`): Stats cards, active downloads, recent additions
- **Movies** (`/movies`): Grid with filters, add modal
- **Movie Detail** (`/movies/[id]`): Metadata, file info, manual search
- **TV Shows** (`/tv`): Grid with filters
- **Show Detail** (`/tv/[id]`): Season/episode accordion
- **Music** (`/music`): Artists grid with filters, add artist modal
- **Artist Detail** (`/music/artists/[id]`): Artist info, albums list, discography
- **Album Detail** (`/music/albums/[id]`): Track list, cover art, manual search
- **Downloads** (`/downloads`): Active queue with controls
- **Settings** (`/settings`): Users, indexers, storage config

---

## Build Configuration

### Nix Development Environment

```nix
# flake.nix
{
  description = "LCARS - Media Collection Manager";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    rust-overlay.url = "github:oxalica/rust-overlay";
    flake-utils.url = "github:numtide/flake-utils";
  };

  outputs = { self, nixpkgs, rust-overlay, flake-utils }:
    flake-utils.lib.eachDefaultSystem (system:
      let
        overlays = [ (import rust-overlay) ];
        pkgs = import nixpkgs { inherit system overlays; };
        rustToolchain = pkgs.rust-bin.stable.latest.default.override {
          extensions = [ "rust-src" "rust-analyzer" ];
          targets = [ "aarch64-unknown-linux-musl" "x86_64-unknown-linux-musl" ];
        };
      in {
        devShells.default = pkgs.mkShell {
          buildInputs = with pkgs; [
            rustToolchain
            cargo-watch cargo-audit cargo-nextest
            nodejs_20 nodePackages.pnpm
            moon cross musl
            ffmpeg sqlite
            just watchexec
          ];
          shellHook = ''
            export RUST_BACKTRACE=1
            export DATABASE_URL="sqlite:./data/lcars.db"
            echo "LCARS dev environment - run 'moon run :dev'"
          '';
        };
      }
    );
}
```

### Moon Workspace

```yaml
# .moon/workspace.yml
$schema: 'https://moonrepo.dev/schemas/workspace.json'
projects:
  - 'apps/*'
vcs:
  manager: git
  defaultBranch: main
```

```yaml
# .moon/toolchain.yml
$schema: 'https://moonrepo.dev/schemas/toolchain.json'
node:
  version: '20.10.0'
  packageManager: pnpm
rust:
  version: '1.75.0'
```

### Backend Moon Tasks

```yaml
# apps/lcars/moon.yml
language: rust
type: application

tasks:
  build:
    command: cargo build --release
    inputs: ['src/**/*', 'Cargo.toml']
    outputs: ['target/release/lcars']

  build-embedded:
    command: cargo build --release --features embed-frontend
    deps: ['~:build', 'web:build']
    inputs: ['src/**/*', 'Cargo.toml', '../web/out/**/*']

  dev:
    command: cargo watch -x run
    local: true

  test:
    command: cargo nextest run

  lint:
    command: cargo clippy -- -D warnings

  format:
    command: cargo fmt --check
```

### Frontend Moon Tasks

```yaml
# apps/web/moon.yml
language: javascript
type: application

tasks:
  build:
    command: pnpm build
    inputs: ['app/**/*', 'components/**/*', 'lib/**/*', 'package.json', '*.config.js']
    outputs: ['out']

  dev:
    command: pnpm dev
    local: true

  lint:
    command: pnpm lint

  typecheck:
    command: pnpm tsc --noEmit
```

### Next.js Config

```javascript
// apps/web/next.config.js
module.exports = {
  output: 'export',
  distDir: 'out',
  trailingSlash: true,
  images: {
    unoptimized: true,
    remotePatterns: [{ protocol: 'https', hostname: 'image.tmdb.org' }],
  },
  async rewrites() {
    if (process.env.NODE_ENV === 'development') {
      return [{ source: '/api/:path*', destination: 'http://localhost:8080/api/:path*' }];
    }
    return [];
  },
};
```

---

## GitHub Actions

### CI Workflow

```yaml
# .github/workflows/ci.yml
name: CI

on:
  push:
    branches: [main]
  pull_request:
    branches: [main]

jobs:
  lint:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-action@stable
        with:
          components: clippy, rustfmt
      - uses: actions/setup-node@v4
        with:
          node-version: '20'
      - uses: pnpm/action-setup@v2
        with:
          version: 8
      - run: npm install -g @moonrepo/cli
      - run: moon run lcars:format lcars:lint
      - run: moon run web:lint web:typecheck

  test:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-action@stable
      - uses: taiki-e/install-action@nextest
      - run: cargo nextest run

  build:
    runs-on: ubuntu-latest
    needs: [lint, test]
    steps:
      - uses: actions/checkout@v4
      - uses: actions/setup-node@v4
        with:
          node-version: '20'
      - uses: pnpm/action-setup@v2
        with:
          version: 8
      - run: npm install -g @moonrepo/cli
      - run: moon run web:build
      - uses: dtolnay/rust-action@stable
      - run: moon run lcars:build-embedded
```

### Release Workflow

```yaml
# .github/workflows/release.yml
name: Release

on:
  push:
    tags: ['v*']

permissions:
  contents: write
  packages: write

jobs:
  build-frontend:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: actions/setup-node@v4
        with:
          node-version: '20'
      - uses: pnpm/action-setup@v2
        with:
          version: 8
      - run: cd apps/web && pnpm install && pnpm build
      - uses: actions/upload-artifact@v4
        with:
          name: frontend
          path: apps/web/out

  build-binaries:
    needs: build-frontend
    runs-on: ${{ matrix.os }}
    strategy:
      fail-fast: false
      matrix:
        include:
          - target: aarch64-unknown-linux-musl
            os: ubuntu-latest
            cross: true
          - target: x86_64-unknown-linux-musl
            os: ubuntu-latest
            cross: true
          - target: aarch64-apple-darwin
            os: macos-latest
            cross: false
          - target: x86_64-apple-darwin
            os: macos-latest
            cross: false
    steps:
      - uses: actions/checkout@v4
      - uses: actions/download-artifact@v4
        with:
          name: frontend
          path: apps/web/out
      - uses: dtolnay/rust-action@stable
        with:
          targets: ${{ matrix.target }}
      - if: matrix.cross
        uses: taiki-e/install-action@cross
      - if: matrix.cross
        run: cd apps/lcars && cross build --release --target ${{ matrix.target }} --features embed-frontend
      - if: ${{ !matrix.cross }}
        run: cd apps/lcars && cargo build --release --target ${{ matrix.target }} --features embed-frontend
      - run: |
          cd apps/lcars/target/${{ matrix.target }}/release
          tar -czvf lcars-${{ matrix.target }}.tar.gz lcars
          mv lcars-${{ matrix.target }}.tar.gz ${{ github.workspace }}/
      - uses: actions/upload-artifact@v4
        with:
          name: lcars-${{ matrix.target }}
          path: lcars-${{ matrix.target }}.tar.gz

  release:
    needs: build-binaries
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: actions/download-artifact@v4
        with:
          path: artifacts
      - uses: softprops/action-gh-release@v1
        with:
          files: artifacts/lcars-*/lcars-*.tar.gz
          generate_release_notes: true

  docker:
    needs: build-frontend
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: actions/download-artifact@v4
        with:
          name: frontend
          path: apps/web/out
      - uses: docker/setup-qemu-action@v3
      - uses: docker/setup-buildx-action@v3
      - uses: docker/login-action@v3
        with:
          registry: ghcr.io
          username: ${{ github.actor }}
          password: ${{ secrets.GITHUB_TOKEN }}
      - id: version
        run: echo "version=${GITHUB_REF#refs/tags/v}" >> $GITHUB_OUTPUT
      - uses: docker/build-push-action@v5
        with:
          context: .
          platforms: linux/amd64,linux/arm64
          push: true
          tags: |
            ghcr.io/${{ github.repository }}:${{ steps.version.outputs.version }}
            ghcr.io/${{ github.repository }}:latest
```

---

## Dockerfile

```dockerfile
FROM node:20-alpine AS frontend
WORKDIR /app/web
COPY apps/web/package*.json ./
RUN npm install -g pnpm && pnpm install
COPY apps/web/ ./
RUN pnpm build

FROM rust:1.75-alpine AS backend
RUN apk add --no-cache musl-dev
WORKDIR /app
COPY Cargo.toml Cargo.lock ./
COPY apps/lcars/ ./apps/lcars/
COPY --from=frontend /app/web/out ./apps/web/out
RUN cd apps/lcars && cargo build --release --features embed-frontend

FROM alpine:3.19
RUN apk add --no-cache ffmpeg ca-certificates
COPY --from=backend /app/apps/lcars/target/release/lcars /usr/local/bin/
EXPOSE 8080
VOLUME ["/data", "/downloads", "/media"]
CMD ["lcars", "--config", "/data/config.toml"]
```

---

## Key Implementation Notes

### VPN/Split Networking

The torrent engine can bind to a specific network interface:

1. User configures VPN externally (WireGuard, OpenVPN, etc.)
2. User specifies interface name in config (`bind_interface = "wg0"`)
3. `librqbit` binds all torrent connections to that interface
4. API server binds to default interface (or specified in `server.host`)

### Post-Download Processing

Simple rule-based system:

1. Download completes → find largest video file
2. Generate destination path using naming pattern + media metadata
3. Execute rules in order:
   - `move`: Move file to destination mount, update DB with new path
   - `copy`: Copy file to destination mount (source remains)
4. Clean up empty directories
5. Update media status to `available`

### Quality Limiting

First version implements simple quality ceiling:

**Video (Movies/TV):**
- User sets `quality_limit` per media (default: 1080p)
- Search results filtered to exclude higher qualities
- Selection prefers highest quality within limit
- No automatic upgrades in v1

**Music:**
- User sets `quality_limit` per artist/album (default: flac)
- Quality hierarchy: flac > alac > 320 > 256 > 192 > 128
- Selection prefers lossless formats when within limit
- Considers sample rate and bit depth for lossless formats

### Metadata Provider Abstraction

TMDB handles movies and TV, MusicBrainz handles music. The traits allow future expansion:

```rust
#[async_trait]
pub trait VideoMetadataProvider: Send + Sync {
    async fn search_movies(&self, query: &str, year: Option<i32>) -> Result<Vec<MovieSearchResult>>;
    async fn search_tv(&self, query: &str) -> Result<Vec<TvSearchResult>>;
    async fn get_movie_details(&self, id: &str) -> Result<MovieDetails>;
    async fn get_tv_details(&self, id: &str) -> Result<TvDetails>;
}

#[async_trait]
pub trait MusicMetadataProvider: Send + Sync {
    async fn search_artists(&self, query: &str) -> Result<Vec<ArtistSearchResult>>;
    async fn search_albums(&self, query: &str, artist_id: Option<&str>) -> Result<Vec<AlbumSearchResult>>;
    async fn get_artist_details(&self, id: &str) -> Result<ArtistDetails>;
    async fn get_album_details(&self, id: &str) -> Result<AlbumDetails>;
    async fn get_album_tracks(&self, id: &str) -> Result<Vec<TrackDetails>>;
}
```

---

## Getting Started (Development)

```bash
# Clone repository
git clone https://github.com/username/lcars.git
cd lcars

# Enter Nix shell (or install deps manually)
nix develop

# Copy example config
cp config.example.toml config.toml
# Edit config.toml with your TMDB API key

# Start development servers
moon run :dev

# Or separately:
moon run lcars:dev  # Backend on :8080
moon run web:dev    # Frontend on :3000 (proxies API)
```

## Building for Production

```bash
# Build frontend
moon run web:build

# Build backend with embedded frontend
moon run lcars:build-embedded

# Binary at: apps/lcars/target/release/lcars
```

## Running

```bash
# With config file
./lcars --config /path/to/config.toml

# Or with environment variables
LCARS_SERVER__PORT=8080 \
LCARS_TMDB__API_KEY=xxx \
./lcars
```

---

## License

MIT
