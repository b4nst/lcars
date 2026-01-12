# LCARS Configuration Reference

LCARS can be configured via a `config.toml` file or environment variables. Environment variables override file settings.

## Configuration Sources

1. Default values (built-in)
2. `config.toml` in the working directory
3. Environment variables with `LCARS_` prefix

Environment variable naming uses double underscores for nesting:
```bash
LCARS_SERVER__PORT=9000        # server.port
LCARS_DATABASE__PATH=/data/db  # database.path
```

## Server Configuration

| Option | Type | Default | Env Variable | Description |
|--------|------|---------|--------------|-------------|
| `server.host` | string | `0.0.0.0` | `LCARS_SERVER__HOST` | Listen address |
| `server.port` | integer | `8080` | `LCARS_SERVER__PORT` | Listen port |
| `server.jwt_secret` | string | *random* | `LCARS_SERVER__JWT_SECRET` | JWT signing secret (required in production) |

Example:
```toml
[server]
host = "0.0.0.0"
port = 8080
jwt_secret = "your-secret-key-at-least-32-characters"
```

## Database Configuration

| Option | Type | Default | Env Variable | Description |
|--------|------|---------|--------------|-------------|
| `database.path` | string | `./data/lcars.db` | `LCARS_DATABASE__PATH` | SQLite database file path |

Example:
```toml
[database]
path = "/var/lib/lcars/lcars.db"
```

## TMDB Configuration

| Option | Type | Default | Env Variable | Description |
|--------|------|---------|--------------|-------------|
| `tmdb.api_key` | string | *none* | `LCARS_TMDB__API_KEY` | TMDB API key (v3) |

Get your API key at: https://www.themoviedb.org/settings/api

Example:
```toml
[tmdb]
api_key = "your-tmdb-api-key"
```

## MusicBrainz Configuration

| Option | Type | Default | Env Variable | Description |
|--------|------|---------|--------------|-------------|
| `musicbrainz.rate_limit_ms` | integer | `1000` | `LCARS_MUSICBRAINZ__RATE_LIMIT_MS` | Minimum ms between requests |

MusicBrainz requires respecting their rate limit (1 request/second for unauthenticated users).

## Torrent Configuration

| Option | Type | Default | Env Variable | Description |
|--------|------|---------|--------------|-------------|
| `torrent.download_dir` | string | `./downloads` | `LCARS_TORRENT__DOWNLOAD_DIR` | Download directory |
| `torrent.bind_interface` | string | *none* | `LCARS_TORRENT__BIND_INTERFACE` | Network interface (for VPN) |
| `torrent.max_connections` | integer | `100` | `LCARS_TORRENT__MAX_CONNECTIONS` | Max peer connections |
| `torrent.port_range` | tuple | `[6881, 6889]` | - | Port range for incoming connections |
| `torrent.seeding.enabled` | boolean | `true` | `LCARS_TORRENT__SEEDING__ENABLED` | Enable seeding after download |
| `torrent.seeding.ratio_limit` | float | `1.0` | `LCARS_TORRENT__SEEDING__RATIO_LIMIT` | Stop seeding at this ratio |
| `torrent.seeding.time_limit_hours` | integer | `48` | `LCARS_TORRENT__SEEDING__TIME_LIMIT_HOURS` | Max seeding time |

Example:
```toml
[torrent]
download_dir = "/downloads"
bind_interface = "tun0"  # VPN interface
max_connections = 200

[torrent.seeding]
enabled = true
ratio_limit = 2.0
time_limit_hours = 72
```

## Storage Configuration

### Mounts

Configure storage mounts for organizing downloaded media.

```toml
[[storage.mounts]]
name = "movies"
type = "local"
path = "/media/movies"
enabled = true

[[storage.mounts]]
name = "nas"
type = "smb"
host = "192.168.1.100"
share = "media"
username = "user"
password = "pass"
mount_point = "/mnt/nas"
enabled = true
```

### Naming Patterns

Configure file naming patterns using placeholders.

| Option | Default | Description |
|--------|---------|-------------|
| `storage.naming.movie_pattern` | `movie/{title} ({year})/{title} ({year}) - {quality}.{ext}` | Movie file pattern |
| `storage.naming.tv_pattern` | `tv/{title}/S{season:02}/{title} - S{season:02}E{episode:02} - {episode_title}.{ext}` | Episode file pattern |
| `storage.naming.music_pattern` | `music/{artist}/{album}/{title}.{ext}` | Music file pattern |

Available placeholders:
- Movies: `{title}`, `{year}`, `{quality}`, `{ext}`
- TV: `{title}`, `{season}`, `{episode}`, `{episode_title}`, `{quality}`, `{ext}`
- Music: `{artist}`, `{album}`, `{title}`, `{track}`, `{ext}`

Padding: Use `{value:02}` for zero-padded numbers (e.g., `01`, `02`).

### Storage Rules

Define actions for post-download processing.

```toml
[[storage.rules]]
action = "move"
destination = "movies"
media_types = ["movie"]

[[storage.rules]]
action = "copy"
destination = "nas"
media_types = ["movie", "tv"]
```

Actions: `move`, `copy`

## Scheduler Configuration

Configure cron schedules for background jobs.

| Option | Default | Description |
|--------|---------|-------------|
| `scheduler.search_missing` | `0 0 */6 * * *` | Search for missing media |
| `scheduler.refresh_metadata` | `0 0 2 * * *` | Refresh metadata from TMDB/MB |
| `scheduler.check_new_episodes` | `0 0 */12 * * *` | Check for new TV episodes |
| `scheduler.check_new_releases` | `0 0 3 * * *` | Check for new album releases |
| `scheduler.cleanup_completed` | `0 0 * * * *` | Clean up completed downloads |

Cron format: `second minute hour day_of_month month day_of_week`

Example:
```toml
[scheduler]
search_missing = "0 0 */4 * * *"      # Every 4 hours
refresh_metadata = "0 0 3 * * *"      # Daily at 3am
check_new_episodes = "0 0 */6 * * *"  # Every 6 hours
```

## Complete Example

```toml
[server]
host = "0.0.0.0"
port = 8080
jwt_secret = "your-very-secret-key-keep-this-safe"

[database]
path = "/var/lib/lcars/lcars.db"

[tmdb]
api_key = "your-tmdb-api-key"

[musicbrainz]
rate_limit_ms = 1000

[torrent]
download_dir = "/downloads"
bind_interface = "wg0"
max_connections = 150

[torrent.seeding]
enabled = true
ratio_limit = 1.5
time_limit_hours = 48

[[storage.mounts]]
name = "media"
type = "local"
path = "/media"
enabled = true

[storage.naming]
movie_pattern = "Movies/{title} ({year})/{title} ({year}).{ext}"
tv_pattern = "TV/{title}/Season {season:02}/{title} - S{season:02}E{episode:02}.{ext}"

[[storage.rules]]
action = "move"
destination = "media"
media_types = ["movie", "tv", "music"]

[scheduler]
search_missing = "0 0 */6 * * *"
refresh_metadata = "0 0 2 * * *"
check_new_episodes = "0 0 */12 * * *"
```

## Environment Variables

For Docker deployments, all settings can be configured via environment:

```bash
# Required
LCARS_SERVER__JWT_SECRET=your-secret-key
LCARS_TMDB__API_KEY=your-tmdb-key

# Optional
LCARS_DATABASE__PATH=/data/lcars.db
LCARS_TORRENT__DOWNLOAD_DIR=/downloads
LCARS_TORRENT__BIND_INTERFACE=tun0
LCARS_ADMIN_PASSWORD=initial-admin-password
```
