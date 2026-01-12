# LCARS API Documentation

## Overview

The LCARS backend provides a RESTful API for managing media libraries. All API endpoints are prefixed with `/api`.

## Authentication

Most endpoints require authentication via JWT tokens.

### Login
```http
POST /api/auth/login
Content-Type: application/json

{
  "username": "admin",
  "password": "your-password"
}
```

Response:
```json
{
  "token": "eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9...",
  "user": {
    "id": 1,
    "username": "admin",
    "role": "admin"
  }
}
```

### Using the Token

Include the token in subsequent requests:
```http
Authorization: Bearer <token>
```

### Get Current User
```http
GET /api/auth/me
Authorization: Bearer <token>
```

### Logout
```http
POST /api/auth/logout
Authorization: Bearer <token>
```

## Movies

### List Movies
```http
GET /api/movies
Authorization: Bearer <token>
```

Query Parameters:
- `status` - Filter by status: `missing`, `searching`, `downloading`, `processing`, `available`
- `monitored` - Filter by monitored: `true`, `false`
- `search` - Full-text search query
- `page` - Page number (default: 1)
- `limit` - Items per page (default: 20, max: 100)

Response:
```json
{
  "items": [
    {
      "id": 1,
      "tmdb_id": 550,
      "title": "Fight Club",
      "year": 1999,
      "overview": "...",
      "poster_path": "/path.jpg",
      "status": "available",
      "monitored": true,
      "quality_limit": "1080p",
      "file_path": "/media/movies/Fight Club (1999)/...",
      "added_at": "2024-01-15T10:30:00Z",
      "updated_at": "2024-01-15T10:30:00Z"
    }
  ],
  "total": 42,
  "page": 1,
  "pages": 3
}
```

### Add Movie
```http
POST /api/movies
Authorization: Bearer <token>
Content-Type: application/json

{
  "tmdb_id": 550,
  "monitored": true,
  "quality_limit": "1080p"
}
```

### Get Movie
```http
GET /api/movies/{id}
Authorization: Bearer <token>
```

### Update Movie
```http
PUT /api/movies/{id}
Authorization: Bearer <token>
Content-Type: application/json

{
  "monitored": false,
  "quality_limit": "4k"
}
```

### Delete Movie
```http
DELETE /api/movies/{id}?delete_files=false
Authorization: Bearer <token>
```

### Search Releases
```http
POST /api/movies/{id}/search
Authorization: Bearer <token>
```

Returns available torrent releases from configured indexers.

### Download Release
```http
POST /api/movies/{id}/download
Authorization: Bearer <token>
Content-Type: application/json

{
  "release_id": "abc123"
}
```

Or with magnet link:
```json
{
  "magnet": "magnet:?xt=urn:btih:..."
}
```

## TV Shows

### List Shows
```http
GET /api/tv
Authorization: Bearer <token>
```

Query parameters same as movies.

### Add Show
```http
POST /api/tv
Authorization: Bearer <token>
Content-Type: application/json

{
  "tmdb_id": 1399,
  "monitored": true,
  "quality_limit": "1080p"
}
```

### Get Show
```http
GET /api/tv/{id}
Authorization: Bearer <token>
```

### Update Show
```http
PUT /api/tv/{id}
Authorization: Bearer <token>
```

### Delete Show
```http
DELETE /api/tv/{id}?delete_files=false
Authorization: Bearer <token>
```

### Get Season Episodes
```http
GET /api/tv/{id}/seasons/{season_number}
Authorization: Bearer <token>
```

### Update Episode
```http
PUT /api/tv/{id}/episodes/{episode_id}
Authorization: Bearer <token>
Content-Type: application/json

{
  "monitored": true
}
```

## Music

### Artists

```http
GET /api/music/artists
POST /api/music/artists
GET /api/music/artists/{id}
PUT /api/music/artists/{id}
DELETE /api/music/artists/{id}
```

### Albums

```http
GET /api/music/albums
GET /api/music/albums/{id}
PUT /api/music/albums/{id}
DELETE /api/music/albums/{id}
POST /api/music/albums/{id}/search
POST /api/music/albums/{id}/download
```

### Tracks

```http
GET /api/music/tracks
PUT /api/music/tracks/{id}
```

## Downloads

### List Downloads
```http
GET /api/downloads
Authorization: Bearer <token>
```

Query Parameters:
- `status` - Filter by status: `queued`, `downloading`, `seeding`, `processing`, `completed`, `failed`, `paused`

### Get Download
```http
GET /api/downloads/{id}
Authorization: Bearer <token>
```

### Pause Download
```http
POST /api/downloads/{id}/pause
Authorization: Bearer <token>
```

### Resume Download
```http
POST /api/downloads/{id}/resume
Authorization: Bearer <token>
```

### Delete Download
```http
DELETE /api/downloads/{id}?delete_files=false
Authorization: Bearer <token>
```

## System

### Get Status
```http
GET /api/system/status
Authorization: Bearer <token>
```

Response:
```json
{
  "version": "0.1.0",
  "uptime_seconds": 3600,
  "active_downloads": 2,
  "disk_space": {
    "total": 1000000000000,
    "free": 500000000000
  }
}
```

### Get Activity Log
```http
GET /api/system/activity
Authorization: Bearer <token>
```

Query Parameters:
- `type` - Filter by event type
- `limit` - Number of events (default: 50)
- `before` - Pagination cursor (ISO timestamp)

### Admin Endpoints

These require admin role.

#### List Jobs
```http
GET /api/system/jobs
Authorization: Bearer <token>
```

#### Trigger Job
```http
POST /api/system/jobs/{name}/run
Authorization: Bearer <token>
```

#### Indexers
```http
GET /api/system/indexers
POST /api/system/indexers
PUT /api/system/indexers/{id}
DELETE /api/system/indexers/{id}
POST /api/system/indexers/{id}/test
```

#### Storage Mounts
```http
GET /api/system/storage/mounts
POST /api/system/storage/mounts/{name}/test
```

## WebSocket

Connect to `/api/ws?token=<jwt>` for real-time download updates.

Message types:
- `DownloadAdded` - New download started
- `DownloadProgress` - Progress update (percentage, speeds, peers)
- `DownloadCompleted` - Download finished
- `DownloadStatus` - Status change
- `DownloadRemoved` - Download removed
- `SystemStatus` - Active downloads count

## Error Responses

All errors follow this format:
```json
{
  "error": "error_code",
  "message": "Human-readable description"
}
```

Common error codes:
- `unauthorized` (401) - Missing or invalid token
- `forbidden` (403) - Insufficient permissions
- `not_found` (404) - Resource not found
- `bad_request` (400) - Invalid request data
- `conflict` (409) - Resource already exists
- `rate_limited` (429) - Too many requests
- `service_unavailable` (503) - External service down
- `timeout` (504) - Request timed out
