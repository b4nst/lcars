//! Music API endpoints for managing artists, albums, and tracks.

use std::collections::HashMap;

use axum::{
    extract::{Path, Query, State},
    routing::{get, post, put},
    Extension, Json, Router,
};
use serde::{Deserialize, Serialize};

use crate::config::MusicQualityConfig;
use crate::db::models::{Album, AlbumStatus, Artist, MediaStatus, MediaType, Track};
use crate::error::{AppError, Result};
use crate::middleware;
use crate::services::indexer::{MediaSearchType, Release, SearchQuery as IndexerSearchQuery};
use crate::services::soulseek::{
    FileResult as SoulseekFileResultType, SearchResult as SoulseekSearchResult,
};
use crate::services::Claims;
use crate::AppState;

// =============================================================================
// Request/Response Types
// =============================================================================

/// Query parameters for listing artists.
#[derive(Debug, Deserialize)]
pub struct ListArtistsQuery {
    /// Filter by monitored state.
    pub monitored: Option<bool>,
    /// Full-text search query.
    pub search: Option<String>,
    /// Page number (1-indexed, default: 1).
    pub page: Option<u32>,
    /// Items per page (default: 20, max: 100).
    pub limit: Option<u32>,
}

/// Query parameters for listing albums.
#[derive(Debug, Deserialize)]
pub struct ListAlbumsQuery {
    /// Filter by artist ID.
    pub artist_id: Option<i64>,
    /// Filter by status (missing, searching, downloading, processing, partial, available).
    pub status: Option<AlbumStatus>,
    /// Filter by monitored state.
    pub monitored: Option<bool>,
    /// Full-text search query.
    pub search: Option<String>,
    /// Page number (1-indexed, default: 1).
    pub page: Option<u32>,
    /// Items per page (default: 20, max: 100).
    pub limit: Option<u32>,
}

/// Query parameters for listing tracks.
#[derive(Debug, Deserialize)]
pub struct ListTracksQuery {
    /// Filter by album ID.
    pub album_id: Option<i64>,
    /// Filter by status.
    pub status: Option<MediaStatus>,
    /// Page number (1-indexed, default: 1).
    pub page: Option<u32>,
    /// Items per page (default: 20, max: 100).
    pub limit: Option<u32>,
}

/// Paginated response wrapper.
#[derive(Debug, Serialize)]
pub struct PaginatedResponse<T> {
    /// Items in the current page.
    pub items: Vec<T>,
    /// Total number of items across all pages.
    pub total: u64,
    /// Current page number (1-indexed).
    pub page: u32,
    /// Total number of pages.
    pub pages: u32,
}

/// Request body for adding an artist.
#[derive(Debug, Deserialize)]
pub struct AddArtistRequest {
    /// MusicBrainz artist ID (UUID).
    pub mbid: String,
    /// Whether to monitor this artist (default: true).
    pub monitored: Option<bool>,
    /// Quality limit for downloads (default: "flac").
    pub quality_limit: Option<String>,
}

/// Request body for updating an artist.
#[derive(Debug, Deserialize)]
pub struct UpdateArtistRequest {
    /// Whether to monitor this artist.
    pub monitored: Option<bool>,
    /// Quality limit for downloads.
    pub quality_limit: Option<String>,
}

/// Request body for updating an album.
#[derive(Debug, Deserialize)]
pub struct UpdateAlbumRequest {
    /// Whether to monitor this album.
    pub monitored: Option<bool>,
    /// Quality limit for downloads.
    pub quality_limit: Option<String>,
}

/// Request body for updating a track.
#[derive(Debug, Deserialize)]
pub struct UpdateTrackRequest {
    /// Whether to monitor this track.
    pub monitored: Option<bool>,
}

/// Query parameters for deleting an artist.
#[derive(Debug, Deserialize)]
pub struct DeleteArtistQuery {
    /// Whether to delete associated files (default: false).
    pub delete_files: Option<bool>,
}

/// Query parameters for deleting an album.
#[derive(Debug, Deserialize)]
pub struct DeleteAlbumQuery {
    /// Whether to delete associated files (default: false).
    pub delete_files: Option<bool>,
}

/// Request body for downloading a release (torrent).
#[derive(Debug, Deserialize)]
pub struct DownloadRequest {
    /// Direct magnet link.
    pub magnet: String,
}

/// Request body for searching releases with multiple sources.
#[derive(Debug, Deserialize)]
pub struct UnifiedSearchRequest {
    /// Sources to search (options: "indexers", "soulseek", or "all").
    /// If not specified, uses the configured default sources.
    pub sources: Option<Vec<String>>,
    /// Optional custom search query (defaults to "Artist Album").
    pub query: Option<String>,
}

/// Request body for downloading from multiple sources.
#[derive(Debug, Deserialize)]
pub struct UnifiedDownloadRequest {
    /// Download source: "torrent" or "soulseek".
    #[serde(default = "default_source")]
    pub source: String,
    /// Magnet link (required for torrent downloads).
    pub magnet: Option<String>,
    /// Soulseek username (required for soulseek downloads).
    pub username: Option<String>,
    /// Soulseek files to download (required for soulseek downloads).
    /// Each item should contain: filename, size.
    pub files: Option<Vec<SoulseekFileDownload>>,
}

fn default_source() -> String {
    "torrent".to_string()
}

/// File to download from Soulseek.
#[derive(Debug, Deserialize)]
pub struct SoulseekFileDownload {
    /// Full path of the file on the peer's system.
    pub filename: String,
    /// File size in bytes.
    pub size: u64,
}

/// A Soulseek search result for an album.
#[derive(Debug, Clone, Serialize)]
pub struct SoulseekAlbumResult {
    /// Source identifier.
    pub source: String,
    /// Username of the peer sharing the files.
    pub username: String,
    /// Common folder path for the album.
    pub folder: String,
    /// Files in the album.
    pub files: Vec<SoulseekFileResult>,
    /// Quality information.
    pub quality: SoulseekQuality,
    /// Whether the user has free upload slots.
    pub user_slots_free: bool,
    /// User's average upload speed in bytes/second.
    pub user_speed: u32,
    /// User's queue length.
    pub queue_length: u32,
    /// Quality score (higher is better).
    pub score: u32,
}

/// A file in a Soulseek album result.
#[derive(Debug, Clone, Serialize)]
pub struct SoulseekFileResult {
    /// Full path of the file.
    pub filename: String,
    /// File size in bytes.
    pub size: u64,
    /// File extension.
    pub extension: String,
    /// Bitrate in kbps (if available).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bitrate: Option<u32>,
    /// Duration in seconds (if available).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub duration: Option<u32>,
}

/// Quality information for a Soulseek result.
#[derive(Debug, Clone, Serialize)]
pub struct SoulseekQuality {
    /// Primary audio format (e.g., "flac", "mp3").
    pub format: String,
    /// Average bitrate in kbps.
    pub bitrate: u32,
}

/// Response for unified search.
#[derive(Debug, Serialize)]
pub struct UnifiedSearchResponse {
    /// Album ID that was searched.
    pub album_id: i64,
    /// Artist name.
    pub artist: String,
    /// Album title.
    pub album: String,
    /// Indexer/torrent results.
    pub indexer_results: Vec<Release>,
    /// Soulseek results (grouped by user/folder).
    pub soulseek_results: Vec<SoulseekAlbumResult>,
}

/// Success response for operations without specific data.
#[derive(Debug, Serialize)]
pub struct SuccessResponse {
    pub success: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
}

/// Download information returned after starting a download.
#[derive(Debug, Serialize)]
pub struct DownloadInfo {
    pub id: i64,
    pub info_hash: String,
    pub name: String,
    pub status: String,
}

/// An artist with album count.
#[derive(Debug, Serialize)]
pub struct ArtistWithStats {
    #[serde(flatten)]
    pub artist: Artist,
    /// Total number of albums for this artist.
    pub album_count: i32,
}

/// An album with all its tracks.
#[derive(Debug, Serialize)]
pub struct AlbumWithTracks {
    #[serde(flatten)]
    pub album: Album,
    pub tracks: Vec<Track>,
}

/// An artist with all albums.
#[derive(Debug, Serialize)]
pub struct ArtistWithAlbums {
    #[serde(flatten)]
    pub artist: Artist,
    pub albums: Vec<Album>,
}

// =============================================================================
// Router
// =============================================================================

/// Create the music router with all routes.
///
/// Note: Path parameters use `:id` syntax instead of `{id}` for compatibility
/// with axum-test. Both syntaxes are valid in Axum 0.7, but axum-test requires
/// the colon syntax for proper route matching in test environments.
pub fn router(state: AppState) -> Router<AppState> {
    Router::new()
        // Artists
        .route("/artists", get(list_artists).post(add_artist))
        .route(
            "/artists/:id",
            get(get_artist).put(update_artist).delete(delete_artist),
        )
        .route("/artists/:id/refresh", post(refresh_artist))
        // Albums
        .route("/albums", get(list_albums))
        .route(
            "/albums/:id",
            get(get_album).put(update_album).delete(delete_album),
        )
        .route("/albums/:id/search", post(search_album_releases))
        .route("/albums/:id/download", post(download_album))
        .route("/albums/:id/refresh", post(refresh_album))
        // Unified search/download endpoints (supports both indexers and Soulseek)
        .route("/albums/:id/unified-search", post(unified_search_album))
        .route("/albums/:id/unified-download", post(unified_download_album))
        // Tracks
        .route("/tracks", get(list_tracks))
        .route("/tracks/:id", put(update_track))
        .route("/tracks/:id/search", post(search_track_releases))
        .route("/tracks/:id/download", post(download_track))
        .layer(axum::middleware::from_fn_with_state(
            state,
            middleware::auth_middleware,
        ))
}

// =============================================================================
// Artist Handlers
// =============================================================================

/// GET /api/music/artists
///
/// Lists all artists with optional filtering and pagination.
pub async fn list_artists(
    State(state): State<AppState>,
    Query(query): Query<ListArtistsQuery>,
) -> Result<Json<PaginatedResponse<ArtistWithStats>>> {
    let page = query.page.unwrap_or(1).clamp(1, u32::MAX);
    let limit = query.limit.unwrap_or(20).clamp(1, 100);
    let offset = (page - 1).saturating_mul(limit);

    let db = state.db.lock().await;

    let (items, total) = if let Some(ref search) = query.search {
        // FTS search query - escape double quotes to prevent FTS syntax errors
        let sanitized = search.trim().replace('"', "\"\"");
        let search_term = format!("\"{}\"*", sanitized);

        // Count total matching items
        let total: u64 = db.query_row(
            r#"
            SELECT COUNT(*) FROM artists a
            JOIN artists_fts fts ON a.id = fts.rowid
            WHERE artists_fts MATCH ?1
              AND (?2 IS NULL OR a.monitored = ?2)
            "#,
            rusqlite::params![search_term, query.monitored,],
            |row| row.get(0),
        )?;

        // Fetch matching items with album count
        let mut stmt = db.prepare(
            r#"
            SELECT a.id, a.mbid, a.name, a.sort_name, a.disambiguation, a.artist_type,
                   a.country, a.begin_date, a.end_date, a.overview, a.image_path,
                   a.monitored, a.quality_limit, a.added_at, a.updated_at, a.added_by,
                   (SELECT COUNT(*) FROM albums WHERE artist_id = a.id) as album_count
            FROM artists a
            JOIN artists_fts fts ON a.id = fts.rowid
            WHERE artists_fts MATCH ?1
              AND (?2 IS NULL OR a.monitored = ?2)
            ORDER BY a.added_at DESC
            LIMIT ?3 OFFSET ?4
            "#,
        )?;

        let items = stmt
            .query_map(
                rusqlite::params![search_term, query.monitored, limit, offset,],
                map_artist_with_stats_row,
            )?
            .collect::<std::result::Result<Vec<_>, _>>()?;

        (items, total)
    } else {
        // Standard query without FTS
        let total: u64 = db.query_row(
            r#"
            SELECT COUNT(*) FROM artists
            WHERE (?1 IS NULL OR monitored = ?1)
            "#,
            rusqlite::params![query.monitored,],
            |row| row.get(0),
        )?;

        let mut stmt = db.prepare(
            r#"
            SELECT a.id, a.mbid, a.name, a.sort_name, a.disambiguation, a.artist_type,
                   a.country, a.begin_date, a.end_date, a.overview, a.image_path,
                   a.monitored, a.quality_limit, a.added_at, a.updated_at, a.added_by,
                   (SELECT COUNT(*) FROM albums WHERE artist_id = a.id) as album_count
            FROM artists a
            WHERE (?1 IS NULL OR a.monitored = ?1)
            ORDER BY a.added_at DESC
            LIMIT ?2 OFFSET ?3
            "#,
        )?;

        let items = stmt
            .query_map(
                rusqlite::params![query.monitored, limit, offset,],
                map_artist_with_stats_row,
            )?
            .collect::<std::result::Result<Vec<_>, _>>()?;

        (items, total)
    };

    let pages = ((total as f64) / (limit as f64)).ceil() as u32;

    Ok(Json(PaginatedResponse {
        items,
        total,
        page,
        pages,
    }))
}

/// POST /api/music/artists
///
/// Adds a new artist by MusicBrainz ID.
pub async fn add_artist(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Json(body): Json<AddArtistRequest>,
) -> Result<Json<ArtistWithAlbums>> {
    // Validate MBID format (must be a valid UUID)
    let mbid = body.mbid.trim();
    if mbid.is_empty() {
        return Err(AppError::BadRequest(
            "MusicBrainz ID is required".to_string(),
        ));
    }
    // Validate UUID format (MusicBrainz uses UUIDs as identifiers)
    // UUID format: xxxxxxxx-xxxx-xxxx-xxxx-xxxxxxxxxxxx (36 chars with 4 dashes)
    if !is_valid_uuid(mbid) {
        return Err(AppError::BadRequest(
            "Invalid MusicBrainz ID format (must be UUID)".to_string(),
        ));
    }

    // Get MusicBrainz client
    let mb_client = state
        .musicbrainz_client()
        .ok_or_else(|| AppError::Internal("MusicBrainz client not configured".to_string()))?;

    // Fetch artist details from MusicBrainz
    let mb_artist = mb_client.get_artist(mbid).await?;

    let monitored = body.monitored.unwrap_or(true);
    let quality_limit = body.quality_limit.unwrap_or_else(|| "flac".to_string());

    let db = state.db.lock().await;

    // Check if artist already exists
    let exists: bool = db.query_row(
        "SELECT EXISTS(SELECT 1 FROM artists WHERE mbid = ?1)",
        [mbid],
        |row| row.get(0),
    )?;

    if exists {
        return Err(AppError::BadRequest(format!(
            "Artist with MusicBrainz ID {} already exists",
            mbid
        )));
    }

    // Extract life span dates
    let begin_date = mb_artist.life_span.as_ref().and_then(|ls| ls.begin.clone());
    let end_date = mb_artist.life_span.as_ref().and_then(|ls| ls.end.clone());

    // Insert the artist
    db.execute(
        r#"
        INSERT INTO artists (
            mbid, name, sort_name, disambiguation, artist_type, country,
            begin_date, end_date, monitored, quality_limit, added_by
        ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)
        "#,
        rusqlite::params![
            mbid,
            mb_artist.name,
            mb_artist.sort_name,
            mb_artist.disambiguation,
            mb_artist.artist_type,
            mb_artist.country,
            begin_date,
            end_date,
            monitored,
            quality_limit,
            claims.sub,
        ],
    )?;

    let artist_id = db.last_insert_rowid();

    // Insert albums from release groups
    let mut albums = Vec::new();
    for rg in &mb_artist.release_groups {
        // Skip compilations and other secondary types for initial import
        if !rg.secondary_types.is_empty() {
            continue;
        }

        db.execute(
            r#"
            INSERT INTO albums (
                mbid, artist_id, title, album_type, release_date,
                status, monitored, quality_limit
            ) VALUES (?1, ?2, ?3, ?4, ?5, 'missing', ?6, ?7)
            "#,
            rusqlite::params![
                rg.id,
                artist_id,
                rg.title,
                rg.primary_type,
                rg.first_release_date,
                monitored,
                quality_limit,
            ],
        )?;

        let album_id = db.last_insert_rowid();
        albums.push(Album {
            id: album_id,
            mbid: rg.id.clone(),
            artist_id,
            title: rg.title.clone(),
            album_type: rg.primary_type.clone(),
            release_date: rg.first_release_date.clone(),
            overview: None,
            cover_path: None,
            total_tracks: None,
            status: AlbumStatus::Missing,
            monitored,
            quality_limit: quality_limit.clone(),
            added_at: String::new(),
            updated_at: String::new(),
        });
    }

    // Fetch the created artist
    let artist = db.query_row(
        r#"
        SELECT id, mbid, name, sort_name, disambiguation, artist_type, country,
               begin_date, end_date, overview, image_path, monitored, quality_limit,
               added_at, updated_at, added_by
        FROM artists WHERE id = ?1
        "#,
        [artist_id],
        map_artist_row,
    )?;

    tracing::info!(
        artist_id = artist.id,
        mbid = %artist.mbid,
        name = %artist.name,
        albums = albums.len(),
        added_by = claims.sub,
        "Artist added"
    );

    Ok(Json(ArtistWithAlbums { artist, albums }))
}

/// GET /api/music/artists/:id
///
/// Gets a single artist by ID with all albums.
pub async fn get_artist(
    State(state): State<AppState>,
    Path(artist_id): Path<i64>,
) -> Result<Json<ArtistWithAlbums>> {
    let db = state.db.lock().await;

    let artist = db
        .query_row(
            r#"
            SELECT id, mbid, name, sort_name, disambiguation, artist_type, country,
                   begin_date, end_date, overview, image_path, monitored, quality_limit,
                   added_at, updated_at, added_by
            FROM artists WHERE id = ?1
            "#,
            [artist_id],
            map_artist_row,
        )
        .map_err(|e| match e {
            rusqlite::Error::QueryReturnedNoRows => {
                AppError::NotFound("Artist not found".to_string())
            }
            _ => AppError::Sqlite(e),
        })?;

    // Fetch all albums for this artist
    let mut stmt = db.prepare(
        r#"
        SELECT id, mbid, artist_id, title, album_type, release_date, overview,
               cover_path, total_tracks, status, monitored, quality_limit,
               added_at, updated_at
        FROM albums
        WHERE artist_id = ?1
        ORDER BY release_date DESC
        "#,
    )?;

    let albums = stmt
        .query_map([artist_id], map_album_row)?
        .collect::<std::result::Result<Vec<_>, _>>()?;

    Ok(Json(ArtistWithAlbums { artist, albums }))
}

/// PUT /api/music/artists/:id
///
/// Updates an artist's settings.
pub async fn update_artist(
    State(state): State<AppState>,
    Path(artist_id): Path<i64>,
    Json(body): Json<UpdateArtistRequest>,
) -> Result<Json<Artist>> {
    let db = state.db.lock().await;

    // Check if artist exists
    let _: i64 = db
        .query_row("SELECT id FROM artists WHERE id = ?1", [artist_id], |row| {
            row.get(0)
        })
        .map_err(|e| match e {
            rusqlite::Error::QueryReturnedNoRows => {
                AppError::NotFound("Artist not found".to_string())
            }
            _ => AppError::Sqlite(e),
        })?;

    // Build dynamic UPDATE query
    let mut updates = Vec::new();
    let mut params: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();

    if let Some(monitored) = body.monitored {
        updates.push("monitored = ?");
        params.push(Box::new(monitored));
    }

    if let Some(ref quality_limit) = body.quality_limit {
        updates.push("quality_limit = ?");
        params.push(Box::new(quality_limit.clone()));
    }

    if updates.is_empty() {
        return Err(AppError::BadRequest("No fields to update".to_string()));
    }

    updates.push("updated_at = datetime('now')");
    let query = format!("UPDATE artists SET {} WHERE id = ?", updates.join(", "));
    params.push(Box::new(artist_id));

    let param_refs: Vec<&dyn rusqlite::ToSql> = params.iter().map(|p| p.as_ref()).collect();
    db.execute(&query, param_refs.as_slice())?;

    // Fetch the updated artist
    let artist = db.query_row(
        r#"
        SELECT id, mbid, name, sort_name, disambiguation, artist_type, country,
               begin_date, end_date, overview, image_path, monitored, quality_limit,
               added_at, updated_at, added_by
        FROM artists WHERE id = ?1
        "#,
        [artist_id],
        map_artist_row,
    )?;

    tracing::info!(artist_id = artist.id, "Artist updated");

    Ok(Json(artist))
}

/// DELETE /api/music/artists/:id
///
/// Deletes an artist from the database.
pub async fn delete_artist(
    State(state): State<AppState>,
    Path(artist_id): Path<i64>,
    Query(query): Query<DeleteArtistQuery>,
) -> Result<Json<SuccessResponse>> {
    let db = state.db.lock().await;

    // Check artist exists
    let _: i64 = db
        .query_row("SELECT id FROM artists WHERE id = ?1", [artist_id], |row| {
            row.get(0)
        })
        .map_err(|e| match e {
            rusqlite::Error::QueryReturnedNoRows => {
                AppError::NotFound("Artist not found".to_string())
            }
            _ => AppError::Sqlite(e),
        })?;

    // Delete files if requested
    if query.delete_files.unwrap_or(false) {
        let mut stmt = db.prepare(
            r#"
            SELECT t.file_path FROM tracks t
            JOIN albums a ON t.album_id = a.id
            WHERE a.artist_id = ?1 AND t.file_path IS NOT NULL
            "#,
        )?;

        let file_paths: Vec<String> = stmt
            .query_map([artist_id], |row| row.get(0))?
            .filter_map(|r| r.ok())
            .collect();

        for path in file_paths {
            let path = std::path::Path::new(&path);
            match std::fs::remove_file(path) {
                Ok(_) => {
                    tracing::info!(
                        artist_id = artist_id,
                        path = %path.display(),
                        "Deleted track file"
                    );
                }
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                    // File already deleted, that's fine
                }
                Err(e) => {
                    tracing::warn!(
                        artist_id = artist_id,
                        path = %path.display(),
                        error = %e,
                        "Failed to delete track file"
                    );
                }
            }
        }
    }

    // Delete from database (CASCADE handles albums and tracks)
    db.execute("DELETE FROM artists WHERE id = ?1", [artist_id])?;

    tracing::info!(
        artist_id = artist_id,
        delete_files = query.delete_files.unwrap_or(false),
        "Artist deleted"
    );

    Ok(Json(SuccessResponse {
        success: true,
        message: Some("Artist deleted successfully".to_string()),
    }))
}

/// POST /api/music/artists/:id/refresh
///
/// Refreshes artist metadata from MusicBrainz.
pub async fn refresh_artist(
    State(state): State<AppState>,
    Path(artist_id): Path<i64>,
) -> Result<Json<ArtistWithAlbums>> {
    // Get MusicBrainz client
    let mb_client = state
        .musicbrainz_client()
        .ok_or_else(|| AppError::Internal("MusicBrainz client not configured".to_string()))?;

    let db = state.db.lock().await;

    // Get artist's MBID and monitored status
    let (mbid, artist_monitored, artist_quality): (String, bool, String) = db
        .query_row(
            "SELECT mbid, monitored, quality_limit FROM artists WHERE id = ?1",
            [artist_id],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .map_err(|e| match e {
            rusqlite::Error::QueryReturnedNoRows => {
                AppError::NotFound("Artist not found".to_string())
            }
            _ => AppError::Sqlite(e),
        })?;

    // Get existing album MBIDs
    let existing_albums: std::collections::HashSet<String> = {
        let mut stmt = db.prepare("SELECT mbid FROM albums WHERE artist_id = ?1")?;
        let result: std::collections::HashSet<String> = stmt
            .query_map([artist_id], |row| row.get(0))?
            .filter_map(|r| r.ok())
            .collect();
        result
    };

    drop(db); // Release lock before async operation

    // Fetch fresh data from MusicBrainz
    let mb_artist = mb_client.get_artist(&mbid).await?;

    // Extract life span dates
    let begin_date = mb_artist.life_span.as_ref().and_then(|ls| ls.begin.clone());
    let end_date = mb_artist.life_span.as_ref().and_then(|ls| ls.end.clone());

    let db = state.db.lock().await;

    // Update artist with fresh metadata
    db.execute(
        r#"
        UPDATE artists SET
            name = ?1,
            sort_name = ?2,
            disambiguation = ?3,
            artist_type = ?4,
            country = ?5,
            begin_date = ?6,
            end_date = ?7,
            updated_at = datetime('now')
        WHERE id = ?8
        "#,
        rusqlite::params![
            mb_artist.name,
            mb_artist.sort_name,
            mb_artist.disambiguation,
            mb_artist.artist_type,
            mb_artist.country,
            begin_date,
            end_date,
            artist_id,
        ],
    )?;

    // Add new albums
    let mut new_album_count = 0;
    for rg in &mb_artist.release_groups {
        // Skip compilations and other secondary types
        if !rg.secondary_types.is_empty() {
            continue;
        }

        if !existing_albums.contains(&rg.id) {
            db.execute(
                r#"
                INSERT INTO albums (
                    mbid, artist_id, title, album_type, release_date,
                    status, monitored, quality_limit
                ) VALUES (?1, ?2, ?3, ?4, ?5, 'missing', ?6, ?7)
                "#,
                rusqlite::params![
                    rg.id,
                    artist_id,
                    rg.title,
                    rg.primary_type,
                    rg.first_release_date,
                    artist_monitored,
                    artist_quality,
                ],
            )?;
            new_album_count += 1;
        }
    }

    // Fetch the updated artist
    let artist = db.query_row(
        r#"
        SELECT id, mbid, name, sort_name, disambiguation, artist_type, country,
               begin_date, end_date, overview, image_path, monitored, quality_limit,
               added_at, updated_at, added_by
        FROM artists WHERE id = ?1
        "#,
        [artist_id],
        map_artist_row,
    )?;

    // Fetch all albums
    let mut stmt = db.prepare(
        r#"
        SELECT id, mbid, artist_id, title, album_type, release_date, overview,
               cover_path, total_tracks, status, monitored, quality_limit,
               added_at, updated_at
        FROM albums
        WHERE artist_id = ?1
        ORDER BY release_date DESC
        "#,
    )?;

    let albums = stmt
        .query_map([artist_id], map_album_row)?
        .collect::<std::result::Result<Vec<_>, _>>()?;

    tracing::info!(
        artist_id = artist.id,
        name = %artist.name,
        new_albums = new_album_count,
        "Artist metadata refreshed"
    );

    Ok(Json(ArtistWithAlbums { artist, albums }))
}

// =============================================================================
// Album Handlers
// =============================================================================

/// GET /api/music/albums
///
/// Lists all albums with optional filtering and pagination.
pub async fn list_albums(
    State(state): State<AppState>,
    Query(query): Query<ListAlbumsQuery>,
) -> Result<Json<PaginatedResponse<Album>>> {
    let page = query.page.unwrap_or(1).clamp(1, u32::MAX);
    let limit = query.limit.unwrap_or(20).clamp(1, 100);
    let offset = (page - 1).saturating_mul(limit);

    let db = state.db.lock().await;

    let (items, total) = if let Some(ref search) = query.search {
        // FTS search query
        let sanitized = search.trim().replace('"', "\"\"");
        let search_term = format!("\"{}\"*", sanitized);

        let total: u64 = db.query_row(
            r#"
            SELECT COUNT(*) FROM albums a
            JOIN albums_fts fts ON a.id = fts.rowid
            WHERE albums_fts MATCH ?1
              AND (?2 IS NULL OR a.artist_id = ?2)
              AND (?3 IS NULL OR a.status = ?3)
              AND (?4 IS NULL OR a.monitored = ?4)
            "#,
            rusqlite::params![
                search_term,
                query.artist_id,
                query.status.as_ref().map(|s| s.to_string()),
                query.monitored,
            ],
            |row| row.get(0),
        )?;

        let mut stmt = db.prepare(
            r#"
            SELECT a.id, a.mbid, a.artist_id, a.title, a.album_type, a.release_date,
                   a.overview, a.cover_path, a.total_tracks, a.status, a.monitored,
                   a.quality_limit, a.added_at, a.updated_at
            FROM albums a
            JOIN albums_fts fts ON a.id = fts.rowid
            WHERE albums_fts MATCH ?1
              AND (?2 IS NULL OR a.artist_id = ?2)
              AND (?3 IS NULL OR a.status = ?3)
              AND (?4 IS NULL OR a.monitored = ?4)
            ORDER BY a.release_date DESC
            LIMIT ?5 OFFSET ?6
            "#,
        )?;

        let items = stmt
            .query_map(
                rusqlite::params![
                    search_term,
                    query.artist_id,
                    query.status.as_ref().map(|s| s.to_string()),
                    query.monitored,
                    limit,
                    offset,
                ],
                map_album_row,
            )?
            .collect::<std::result::Result<Vec<_>, _>>()?;

        (items, total)
    } else {
        let total: u64 = db.query_row(
            r#"
            SELECT COUNT(*) FROM albums
            WHERE (?1 IS NULL OR artist_id = ?1)
              AND (?2 IS NULL OR status = ?2)
              AND (?3 IS NULL OR monitored = ?3)
            "#,
            rusqlite::params![
                query.artist_id,
                query.status.as_ref().map(|s| s.to_string()),
                query.monitored,
            ],
            |row| row.get(0),
        )?;

        let mut stmt = db.prepare(
            r#"
            SELECT id, mbid, artist_id, title, album_type, release_date, overview,
                   cover_path, total_tracks, status, monitored, quality_limit,
                   added_at, updated_at
            FROM albums
            WHERE (?1 IS NULL OR artist_id = ?1)
              AND (?2 IS NULL OR status = ?2)
              AND (?3 IS NULL OR monitored = ?3)
            ORDER BY release_date DESC
            LIMIT ?4 OFFSET ?5
            "#,
        )?;

        let items = stmt
            .query_map(
                rusqlite::params![
                    query.artist_id,
                    query.status.as_ref().map(|s| s.to_string()),
                    query.monitored,
                    limit,
                    offset,
                ],
                map_album_row,
            )?
            .collect::<std::result::Result<Vec<_>, _>>()?;

        (items, total)
    };

    let pages = ((total as f64) / (limit as f64)).ceil() as u32;

    Ok(Json(PaginatedResponse {
        items,
        total,
        page,
        pages,
    }))
}

/// GET /api/music/albums/:id
///
/// Gets a single album by ID with all tracks.
pub async fn get_album(
    State(state): State<AppState>,
    Path(album_id): Path<i64>,
) -> Result<Json<AlbumWithTracks>> {
    let db = state.db.lock().await;

    let album = db
        .query_row(
            r#"
            SELECT id, mbid, artist_id, title, album_type, release_date, overview,
                   cover_path, total_tracks, status, monitored, quality_limit,
                   added_at, updated_at
            FROM albums WHERE id = ?1
            "#,
            [album_id],
            map_album_row,
        )
        .map_err(|e| match e {
            rusqlite::Error::QueryReturnedNoRows => {
                AppError::NotFound("Album not found".to_string())
            }
            _ => AppError::Sqlite(e),
        })?;

    // Fetch all tracks for this album
    let mut stmt = db.prepare(
        r#"
        SELECT id, mbid, album_id, artist_id, title, track_number, disc_number,
               duration_ms, status, monitored, file_path, file_size, audio_format,
               bitrate, sample_rate, bit_depth, created_at, updated_at
        FROM tracks
        WHERE album_id = ?1
        ORDER BY disc_number, track_number
        "#,
    )?;

    let tracks = stmt
        .query_map([album_id], map_track_row)?
        .collect::<std::result::Result<Vec<_>, _>>()?;

    Ok(Json(AlbumWithTracks { album, tracks }))
}

/// PUT /api/music/albums/:id
///
/// Updates an album's settings.
pub async fn update_album(
    State(state): State<AppState>,
    Path(album_id): Path<i64>,
    Json(body): Json<UpdateAlbumRequest>,
) -> Result<Json<Album>> {
    let db = state.db.lock().await;

    // Check if album exists
    let _: i64 = db
        .query_row("SELECT id FROM albums WHERE id = ?1", [album_id], |row| {
            row.get(0)
        })
        .map_err(|e| match e {
            rusqlite::Error::QueryReturnedNoRows => {
                AppError::NotFound("Album not found".to_string())
            }
            _ => AppError::Sqlite(e),
        })?;

    // Build dynamic UPDATE query
    let mut updates = Vec::new();
    let mut params: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();

    if let Some(monitored) = body.monitored {
        updates.push("monitored = ?");
        params.push(Box::new(monitored));
    }

    if let Some(ref quality_limit) = body.quality_limit {
        updates.push("quality_limit = ?");
        params.push(Box::new(quality_limit.clone()));
    }

    if updates.is_empty() {
        return Err(AppError::BadRequest("No fields to update".to_string()));
    }

    updates.push("updated_at = datetime('now')");
    let query = format!("UPDATE albums SET {} WHERE id = ?", updates.join(", "));
    params.push(Box::new(album_id));

    let param_refs: Vec<&dyn rusqlite::ToSql> = params.iter().map(|p| p.as_ref()).collect();
    db.execute(&query, param_refs.as_slice())?;

    // Fetch the updated album
    let album = db.query_row(
        r#"
        SELECT id, mbid, artist_id, title, album_type, release_date, overview,
               cover_path, total_tracks, status, monitored, quality_limit,
               added_at, updated_at
        FROM albums WHERE id = ?1
        "#,
        [album_id],
        map_album_row,
    )?;

    tracing::info!(album_id = album.id, "Album updated");

    Ok(Json(album))
}

/// DELETE /api/music/albums/:id
///
/// Deletes an album from the database.
pub async fn delete_album(
    State(state): State<AppState>,
    Path(album_id): Path<i64>,
    Query(query): Query<DeleteAlbumQuery>,
) -> Result<Json<SuccessResponse>> {
    let db = state.db.lock().await;

    // Check album exists
    let _: i64 = db
        .query_row("SELECT id FROM albums WHERE id = ?1", [album_id], |row| {
            row.get(0)
        })
        .map_err(|e| match e {
            rusqlite::Error::QueryReturnedNoRows => {
                AppError::NotFound("Album not found".to_string())
            }
            _ => AppError::Sqlite(e),
        })?;

    // Delete files if requested
    if query.delete_files.unwrap_or(false) {
        let mut stmt = db.prepare(
            "SELECT file_path FROM tracks WHERE album_id = ?1 AND file_path IS NOT NULL",
        )?;

        let file_paths: Vec<String> = stmt
            .query_map([album_id], |row| row.get(0))?
            .filter_map(|r| r.ok())
            .collect();

        for path in file_paths {
            let path = std::path::Path::new(&path);
            match std::fs::remove_file(path) {
                Ok(_) => {
                    tracing::info!(
                        album_id = album_id,
                        path = %path.display(),
                        "Deleted track file"
                    );
                }
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
                Err(e) => {
                    tracing::warn!(
                        album_id = album_id,
                        path = %path.display(),
                        error = %e,
                        "Failed to delete track file"
                    );
                }
            }
        }
    }

    // Delete from database (CASCADE handles tracks)
    db.execute("DELETE FROM albums WHERE id = ?1", [album_id])?;

    tracing::info!(
        album_id = album_id,
        delete_files = query.delete_files.unwrap_or(false),
        "Album deleted"
    );

    Ok(Json(SuccessResponse {
        success: true,
        message: Some("Album deleted successfully".to_string()),
    }))
}

/// POST /api/music/albums/:id/search
///
/// Searches indexers for releases of this album.
pub async fn search_album_releases(
    State(state): State<AppState>,
    Path(album_id): Path<i64>,
) -> Result<Json<Vec<Release>>> {
    let db = state.db.lock().await;

    // Get album and artist details
    let (album_title, artist_name): (String, String) = db
        .query_row(
            r#"
            SELECT a.title, ar.name
            FROM albums a
            JOIN artists ar ON a.artist_id = ar.id
            WHERE a.id = ?1
            "#,
            [album_id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .map_err(|e| match e {
            rusqlite::Error::QueryReturnedNoRows => {
                AppError::NotFound("Album not found".to_string())
            }
            _ => AppError::Sqlite(e),
        })?;

    drop(db); // Release the lock before async operations

    // Build search query with artist and album
    let search_term = format!("{} {}", artist_name, album_title);
    let query = IndexerSearchQuery::new(&search_term).media_type(MediaSearchType::MusicAlbum);

    // Search indexers
    let indexer_manager = state.indexer_manager();
    let releases = indexer_manager.search(&query).await?;

    tracing::info!(
        album_id = album_id,
        artist = %artist_name,
        album = %album_title,
        results = releases.len(),
        "Searched releases for album"
    );

    Ok(Json(releases))
}

/// POST /api/music/albums/:id/unified-search
///
/// Searches both indexers and Soulseek for releases of this album.
pub async fn unified_search_album(
    State(state): State<AppState>,
    Path(album_id): Path<i64>,
    Json(body): Json<UnifiedSearchRequest>,
) -> Result<Json<UnifiedSearchResponse>> {
    let db = state.db.lock().await;

    // Get album and artist details
    let (album_title, artist_name, total_tracks): (String, String, Option<i32>) = db
        .query_row(
            r#"
            SELECT a.title, ar.name, a.total_tracks
            FROM albums a
            JOIN artists ar ON a.artist_id = ar.id
            WHERE a.id = ?1
            "#,
            [album_id],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .map_err(|e| match e {
            rusqlite::Error::QueryReturnedNoRows => {
                AppError::NotFound("Album not found".to_string())
            }
            _ => AppError::Sqlite(e),
        })?;

    drop(db); // Release the lock before async operations

    // Determine which sources to search
    let sources = body
        .sources
        .unwrap_or_else(|| state.config.music.search_sources.clone());

    // Build search query
    let search_term = body
        .query
        .unwrap_or_else(|| format!("{} {}", artist_name, album_title));

    let mut indexer_results = Vec::new();
    let mut soulseek_results = Vec::new();

    // Search indexers if requested
    if sources.iter().any(|s| s == "indexers" || s == "all") {
        let query = IndexerSearchQuery::new(&search_term).media_type(MediaSearchType::MusicAlbum);
        let indexer_manager = state.indexer_manager();
        match indexer_manager.search(&query).await {
            Ok(releases) => {
                indexer_results = releases;
            }
            Err(e) => {
                tracing::warn!(error = %e, "Failed to search indexers");
            }
        }
    }

    // Search Soulseek if requested and available
    if sources.iter().any(|s| s == "soulseek" || s == "all") {
        if let Some(engine) = state.soulseek_engine() {
            if engine.is_connected().await {
                match engine.search(&search_term).await {
                    Ok(ticket) => {
                        // Wait briefly for results to come in
                        tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;

                        if let Some(search_state) = engine.get_search_results(ticket).await {
                            // Group results by user/folder and score them
                            soulseek_results = group_and_score_soulseek_results(
                                &search_state.results,
                                total_tracks.unwrap_or(0) as usize,
                                &state.config.music.quality,
                            );
                        }

                        // Cancel the search after we have results
                        let _ = engine.cancel_search(ticket).await;
                    }
                    Err(e) => {
                        tracing::warn!(error = %e, "Failed to search Soulseek");
                    }
                }
            } else {
                tracing::debug!("Soulseek not connected, skipping search");
            }
        }
    }

    tracing::info!(
        album_id = album_id,
        artist = %artist_name,
        album = %album_title,
        indexer_results = indexer_results.len(),
        soulseek_results = soulseek_results.len(),
        "Unified search completed"
    );

    Ok(Json(UnifiedSearchResponse {
        album_id,
        artist: artist_name,
        album: album_title,
        indexer_results,
        soulseek_results,
    }))
}

/// POST /api/music/albums/:id/unified-download
///
/// Starts downloading a release for this album from either torrent or Soulseek.
pub async fn unified_download_album(
    State(state): State<AppState>,
    Path(album_id): Path<i64>,
    Json(body): Json<UnifiedDownloadRequest>,
) -> Result<Json<DownloadInfo>> {
    match body.source.as_str() {
        "torrent" => {
            // Validate magnet link
            let magnet = body.magnet.ok_or_else(|| {
                AppError::BadRequest("Magnet link required for torrent downloads".to_string())
            })?;

            if !magnet.starts_with("magnet:?") {
                return Err(AppError::BadRequest(
                    "Invalid magnet link format".to_string(),
                ));
            }

            // Get torrent engine
            let torrent_engine = state
                .torrent_engine()
                .ok_or_else(|| AppError::Internal("Torrent engine not available".to_string()))?;

            let db = state.db.lock().await;

            // Get album info
            let (title,): (String,) = db
                .query_row(
                    "SELECT title FROM albums WHERE id = ?1",
                    [album_id],
                    |row| Ok((row.get(0)?,)),
                )
                .map_err(|e| match e {
                    rusqlite::Error::QueryReturnedNoRows => {
                        AppError::NotFound("Album not found".to_string())
                    }
                    _ => AppError::Sqlite(e),
                })?;

            drop(db);

            // Add magnet to torrent engine
            let media_ref = crate::services::torrent::MediaRef {
                media_type: MediaType::Album,
                media_id: album_id,
            };

            let info_hash = torrent_engine.add_magnet(&magnet, media_ref).await?;

            // Create download record and update album status
            let db = state.db.lock().await;

            db.execute(
                r#"
                INSERT INTO downloads (source_type, source_id, name, media_type, media_id, source_uri, status)
                VALUES ('torrent', ?1, ?2, 'album', ?3, ?4, 'downloading')
                "#,
                rusqlite::params![info_hash, title, album_id, magnet],
            )?;

            let download_id = db.last_insert_rowid();

            db.execute(
                "UPDATE albums SET status = 'downloading', updated_at = datetime('now') WHERE id = ?1",
                [album_id],
            )?;

            tracing::info!(
                album_id = album_id,
                info_hash = %info_hash,
                download_id = download_id,
                source = "torrent",
                "Started album download via unified endpoint"
            );

            Ok(Json(DownloadInfo {
                id: download_id,
                info_hash,
                name: title,
                status: "downloading".to_string(),
            }))
        }
        "soulseek" => {
            // Validate Soulseek parameters
            let username = body.username.ok_or_else(|| {
                AppError::BadRequest("Username required for Soulseek downloads".to_string())
            })?;

            let files = body.files.ok_or_else(|| {
                AppError::BadRequest("Files required for Soulseek downloads".to_string())
            })?;

            if files.is_empty() {
                return Err(AppError::BadRequest(
                    "At least one file required for download".to_string(),
                ));
            }

            // Get Soulseek engine
            let engine = state.soulseek_engine().ok_or_else(|| {
                AppError::ServiceUnavailable("Soulseek not configured".to_string())
            })?;

            if !engine.is_connected().await {
                return Err(AppError::ServiceUnavailable(
                    "Soulseek is not connected".to_string(),
                ));
            }

            let db = state.db.lock().await;

            // Get album info
            let (title,): (String,) = db
                .query_row(
                    "SELECT title FROM albums WHERE id = ?1",
                    [album_id],
                    |row| Ok((row.get(0)?,)),
                )
                .map_err(|e| match e {
                    rusqlite::Error::QueryReturnedNoRows => {
                        AppError::NotFound("Album not found".to_string())
                    }
                    _ => AppError::Sqlite(e),
                })?;

            drop(db);

            // Start downloads for each file
            let mut first_download_id = String::new();
            for (i, file) in files.iter().enumerate() {
                let request = crate::services::soulseek::DownloadRequest {
                    username: username.clone(),
                    filename: file.filename.clone(),
                    size: file.size,
                    media_type: Some("album".to_string()),
                    media_id: Some(album_id),
                };

                match engine.download(request).await {
                    Ok(id) => {
                        if i == 0 {
                            first_download_id = id.clone();
                        }

                        // Create download record in database
                        let db = state.db.lock().await;
                        let source_uri = format!("soulseek://{}/{}", username, file.filename);
                        let file_name = file
                            .filename
                            .rsplit(['/', '\\'])
                            .next()
                            .unwrap_or(&file.filename);

                        db.execute(
                            r#"
                            INSERT INTO downloads (source_type, source_id, name, media_type, media_id, source_uri, status, size_bytes, soulseek_username, soulseek_filename)
                            VALUES ('soulseek', ?1, ?2, 'album', ?3, ?4, 'downloading', ?5, ?6, ?7)
                            "#,
                            rusqlite::params![id, file_name, album_id, source_uri, file.size, username, file.filename],
                        )?;
                    }
                    Err(e) => {
                        tracing::warn!(
                            username = %username,
                            filename = %file.filename,
                            error = %e,
                            "Failed to start Soulseek download for file"
                        );
                    }
                }
            }

            // Update album status
            let db = state.db.lock().await;
            db.execute(
                "UPDATE albums SET status = 'downloading', updated_at = datetime('now') WHERE id = ?1",
                [album_id],
            )?;

            tracing::info!(
                album_id = album_id,
                username = %username,
                files = files.len(),
                source = "soulseek",
                "Started album download via unified endpoint"
            );

            Ok(Json(DownloadInfo {
                id: 0, // Soulseek downloads don't have a single DB ID
                info_hash: first_download_id,
                name: title,
                status: "downloading".to_string(),
            }))
        }
        _ => Err(AppError::BadRequest(format!(
            "Invalid source: {}. Must be 'torrent' or 'soulseek'",
            body.source
        ))),
    }
}

/// POST /api/music/albums/:id/download
///
/// Starts downloading a release for this album.
pub async fn download_album(
    State(state): State<AppState>,
    Path(album_id): Path<i64>,
    Json(body): Json<DownloadRequest>,
) -> Result<Json<DownloadInfo>> {
    // Validate magnet link format
    if !body.magnet.starts_with("magnet:?") {
        return Err(AppError::BadRequest(
            "Invalid magnet link format".to_string(),
        ));
    }

    // Get torrent engine
    let torrent_engine = state
        .torrent_engine()
        .ok_or_else(|| AppError::Internal("Torrent engine not available".to_string()))?;

    let db = state.db.lock().await;

    // Get album info
    let (title,): (String,) = db
        .query_row(
            "SELECT title FROM albums WHERE id = ?1",
            [album_id],
            |row| Ok((row.get(0)?,)),
        )
        .map_err(|e| match e {
            rusqlite::Error::QueryReturnedNoRows => {
                AppError::NotFound("Album not found".to_string())
            }
            _ => AppError::Sqlite(e),
        })?;

    drop(db); // Release lock before async operation

    // Add magnet to torrent engine
    let media_ref = crate::services::torrent::MediaRef {
        media_type: MediaType::Album,
        media_id: album_id,
    };

    let info_hash = torrent_engine.add_magnet(&body.magnet, media_ref).await?;

    // Create download record and update album status
    let db = state.db.lock().await;

    db.execute(
        r#"
        INSERT INTO downloads (source_type, source_id, name, media_type, media_id, source_uri, status)
        VALUES ('torrent', ?1, ?2, 'album', ?3, ?4, 'downloading')
        "#,
        rusqlite::params![info_hash, title, album_id, body.magnet],
    )?;

    let download_id = db.last_insert_rowid();

    // Update album status
    db.execute(
        "UPDATE albums SET status = 'downloading', updated_at = datetime('now') WHERE id = ?1",
        [album_id],
    )?;

    tracing::info!(
        album_id = album_id,
        info_hash = %info_hash,
        download_id = download_id,
        "Started album download"
    );

    Ok(Json(DownloadInfo {
        id: download_id,
        info_hash,
        name: title,
        status: "downloading".to_string(),
    }))
}

/// POST /api/music/albums/:id/refresh
///
/// Refreshes album metadata from MusicBrainz.
pub async fn refresh_album(
    State(state): State<AppState>,
    Path(album_id): Path<i64>,
) -> Result<Json<AlbumWithTracks>> {
    // Get MusicBrainz client
    let mb_client = state
        .musicbrainz_client()
        .ok_or_else(|| AppError::Internal("MusicBrainz client not configured".to_string()))?;

    let db = state.db.lock().await;

    // Get album's MBID and settings
    let (mbid, album_monitored): (String, bool) = db
        .query_row(
            "SELECT mbid, monitored FROM albums WHERE id = ?1",
            [album_id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .map_err(|e| match e {
            rusqlite::Error::QueryReturnedNoRows => {
                AppError::NotFound("Album not found".to_string())
            }
            _ => AppError::Sqlite(e),
        })?;

    // Get existing track MBIDs
    let existing_tracks: std::collections::HashSet<(i32, i32)> = {
        let mut stmt =
            db.prepare("SELECT disc_number, track_number FROM tracks WHERE album_id = ?1")?;
        let result: std::collections::HashSet<(i32, i32)> = stmt
            .query_map([album_id], |row| Ok((row.get(0)?, row.get(1)?)))?
            .filter_map(|r| r.ok())
            .collect();
        result
    };

    drop(db); // Release lock before async operation

    // Fetch release group details
    let rg_details = mb_client.get_release_group(&mbid).await?;

    // Get the first official release to fetch tracks
    let release_mbid = rg_details
        .releases
        .iter()
        .find(|r| r.status.as_deref() == Some("Official"))
        .or(rg_details.releases.first())
        .map(|r| r.id.clone());

    let mut new_track_count = 0;
    let mut total_tracks = 0;

    if let Some(release_mbid) = release_mbid {
        let release = mb_client.get_release(&release_mbid).await?;

        let db = state.db.lock().await;

        // Update album with release info
        db.execute(
            r#"
            UPDATE albums SET
                title = ?1,
                total_tracks = ?2,
                updated_at = datetime('now')
            WHERE id = ?3
            "#,
            rusqlite::params![
                release.title,
                release.media.iter().map(|m| m.track_count).sum::<u32>() as i32,
                album_id,
            ],
        )?;

        // Add tracks
        for medium in &release.media {
            for track in &medium.tracks {
                let key = (medium.position as i32, track.position as i32);
                total_tracks += 1;

                if !existing_tracks.contains(&key) {
                    db.execute(
                        r#"
                        INSERT INTO tracks (
                            mbid, album_id, title, track_number, disc_number,
                            duration_ms, status, monitored
                        ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, 'missing', ?7)
                        "#,
                        rusqlite::params![
                            track.recording.id,
                            album_id,
                            track.title,
                            track.position as i32,
                            medium.position as i32,
                            track.length.or(track.recording.length).map(|l| l as i32),
                            album_monitored,
                        ],
                    )?;
                    new_track_count += 1;
                }
            }
        }
    }

    let db = state.db.lock().await;

    // Fetch the updated album
    let album = db.query_row(
        r#"
        SELECT id, mbid, artist_id, title, album_type, release_date, overview,
               cover_path, total_tracks, status, monitored, quality_limit,
               added_at, updated_at
        FROM albums WHERE id = ?1
        "#,
        [album_id],
        map_album_row,
    )?;

    // Fetch all tracks
    let mut stmt = db.prepare(
        r#"
        SELECT id, mbid, album_id, artist_id, title, track_number, disc_number,
               duration_ms, status, monitored, file_path, file_size, audio_format,
               bitrate, sample_rate, bit_depth, created_at, updated_at
        FROM tracks
        WHERE album_id = ?1
        ORDER BY disc_number, track_number
        "#,
    )?;

    let tracks = stmt
        .query_map([album_id], map_track_row)?
        .collect::<std::result::Result<Vec<_>, _>>()?;

    tracing::info!(
        album_id = album.id,
        title = %album.title,
        new_tracks = new_track_count,
        total_tracks = total_tracks,
        "Album metadata refreshed"
    );

    Ok(Json(AlbumWithTracks { album, tracks }))
}

// =============================================================================
// Track Handlers
// =============================================================================

/// GET /api/music/tracks
///
/// Lists all tracks with optional filtering and pagination.
pub async fn list_tracks(
    State(state): State<AppState>,
    Query(query): Query<ListTracksQuery>,
) -> Result<Json<PaginatedResponse<Track>>> {
    let page = query.page.unwrap_or(1).clamp(1, u32::MAX);
    let limit = query.limit.unwrap_or(20).clamp(1, 100);
    let offset = (page - 1).saturating_mul(limit);

    let db = state.db.lock().await;

    let total: u64 = db.query_row(
        r#"
        SELECT COUNT(*) FROM tracks
        WHERE (?1 IS NULL OR album_id = ?1)
          AND (?2 IS NULL OR status = ?2)
        "#,
        rusqlite::params![query.album_id, query.status.as_ref().map(|s| s.to_string()),],
        |row| row.get(0),
    )?;

    let mut stmt = db.prepare(
        r#"
        SELECT id, mbid, album_id, artist_id, title, track_number, disc_number,
               duration_ms, status, monitored, file_path, file_size, audio_format,
               bitrate, sample_rate, bit_depth, created_at, updated_at
        FROM tracks
        WHERE (?1 IS NULL OR album_id = ?1)
          AND (?2 IS NULL OR status = ?2)
        ORDER BY album_id, disc_number, track_number
        LIMIT ?3 OFFSET ?4
        "#,
    )?;

    let items = stmt
        .query_map(
            rusqlite::params![
                query.album_id,
                query.status.as_ref().map(|s| s.to_string()),
                limit,
                offset,
            ],
            map_track_row,
        )?
        .collect::<std::result::Result<Vec<_>, _>>()?;

    let pages = ((total as f64) / (limit as f64)).ceil() as u32;

    Ok(Json(PaginatedResponse {
        items,
        total,
        page,
        pages,
    }))
}

/// PUT /api/music/tracks/:id
///
/// Updates a track's settings.
pub async fn update_track(
    State(state): State<AppState>,
    Path(track_id): Path<i64>,
    Json(body): Json<UpdateTrackRequest>,
) -> Result<Json<Track>> {
    if body.monitored.is_none() {
        return Err(AppError::BadRequest("No fields to update".to_string()));
    }

    let db = state.db.lock().await;

    // Check if track exists
    let _: i64 = db
        .query_row("SELECT id FROM tracks WHERE id = ?1", [track_id], |row| {
            row.get(0)
        })
        .map_err(|e| match e {
            rusqlite::Error::QueryReturnedNoRows => {
                AppError::NotFound("Track not found".to_string())
            }
            _ => AppError::Sqlite(e),
        })?;

    // Update track
    if let Some(monitored) = body.monitored {
        db.execute(
            r#"
            UPDATE tracks SET monitored = ?1, updated_at = datetime('now')
            WHERE id = ?2
            "#,
            rusqlite::params![monitored, track_id],
        )?;
    }

    // Fetch the updated track
    let track = db.query_row(
        r#"
        SELECT id, mbid, album_id, artist_id, title, track_number, disc_number,
               duration_ms, status, monitored, file_path, file_size, audio_format,
               bitrate, sample_rate, bit_depth, created_at, updated_at
        FROM tracks WHERE id = ?1
        "#,
        [track_id],
        map_track_row,
    )?;

    tracing::info!(track_id = track.id, "Track updated");

    Ok(Json(track))
}

/// POST /api/music/tracks/:id/search
///
/// Searches indexers for releases of this track.
pub async fn search_track_releases(
    State(state): State<AppState>,
    Path(track_id): Path<i64>,
) -> Result<Json<Vec<Release>>> {
    let db = state.db.lock().await;

    // Get track and artist details
    let (track_title, artist_name): (String, String) = db
        .query_row(
            r#"
            SELECT t.title, ar.name
            FROM tracks t
            JOIN albums a ON t.album_id = a.id
            JOIN artists ar ON a.artist_id = ar.id
            WHERE t.id = ?1
            "#,
            [track_id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .map_err(|e| match e {
            rusqlite::Error::QueryReturnedNoRows => {
                AppError::NotFound("Track not found".to_string())
            }
            _ => AppError::Sqlite(e),
        })?;

    drop(db); // Release the lock before async operations

    // Build search query
    let search_term = format!("{} {}", artist_name, track_title);
    let query = IndexerSearchQuery::new(&search_term).media_type(MediaSearchType::MusicAlbum);

    // Search indexers
    let indexer_manager = state.indexer_manager();
    let releases = indexer_manager.search(&query).await?;

    tracing::info!(
        track_id = track_id,
        artist = %artist_name,
        track = %track_title,
        results = releases.len(),
        "Searched releases for track"
    );

    Ok(Json(releases))
}

/// POST /api/music/tracks/:id/download
///
/// Starts downloading a release for this track.
pub async fn download_track(
    State(state): State<AppState>,
    Path(track_id): Path<i64>,
    Json(body): Json<DownloadRequest>,
) -> Result<Json<DownloadInfo>> {
    // Validate magnet link format
    if !body.magnet.starts_with("magnet:?") {
        return Err(AppError::BadRequest(
            "Invalid magnet link format".to_string(),
        ));
    }

    // Get torrent engine
    let torrent_engine = state
        .torrent_engine()
        .ok_or_else(|| AppError::Internal("Torrent engine not available".to_string()))?;

    let db = state.db.lock().await;

    // Get track info
    let (title,): (String,) = db
        .query_row(
            "SELECT title FROM tracks WHERE id = ?1",
            [track_id],
            |row| Ok((row.get(0)?,)),
        )
        .map_err(|e| match e {
            rusqlite::Error::QueryReturnedNoRows => {
                AppError::NotFound("Track not found".to_string())
            }
            _ => AppError::Sqlite(e),
        })?;

    drop(db); // Release lock before async operation

    // Add magnet to torrent engine
    let media_ref = crate::services::torrent::MediaRef {
        media_type: MediaType::Track,
        media_id: track_id,
    };

    let info_hash = torrent_engine.add_magnet(&body.magnet, media_ref).await?;

    // Create download record and update track status
    let db = state.db.lock().await;

    db.execute(
        r#"
        INSERT INTO downloads (source_type, source_id, name, media_type, media_id, source_uri, status)
        VALUES ('torrent', ?1, ?2, 'track', ?3, ?4, 'downloading')
        "#,
        rusqlite::params![info_hash, title, track_id, body.magnet],
    )?;

    let download_id = db.last_insert_rowid();

    // Update track status
    db.execute(
        "UPDATE tracks SET status = 'downloading', updated_at = datetime('now') WHERE id = ?1",
        [track_id],
    )?;

    tracing::info!(
        track_id = track_id,
        info_hash = %info_hash,
        download_id = download_id,
        "Started track download"
    );

    Ok(Json(DownloadInfo {
        id: download_id,
        info_hash,
        name: title,
        status: "downloading".to_string(),
    }))
}

// =============================================================================
// Helpers
// =============================================================================

/// Groups Soulseek search results by user/folder and scores them for quality.
fn group_and_score_soulseek_results(
    results: &[SoulseekSearchResult],
    expected_track_count: usize,
    quality_prefs: &MusicQualityConfig,
) -> Vec<SoulseekAlbumResult> {
    // Group files by username and folder
    let mut grouped: HashMap<
        (String, String),
        Vec<(&SoulseekSearchResult, &SoulseekFileResultType)>,
    > = HashMap::new();

    for result in results {
        for file in &result.files {
            // Extract folder from filename
            let folder = extract_folder(&file.filename);
            let key = (result.username.clone(), folder);
            grouped.entry(key).or_default().push((result, file));
        }
    }

    // Convert to album results and score them
    let mut album_results: Vec<SoulseekAlbumResult> = grouped
        .into_iter()
        .filter_map(|((username, folder), files)| {
            // Only include groups with audio files
            let audio_files: Vec<_> = files
                .iter()
                .filter(|(_, f)| is_audio_extension(&f.extension))
                .collect();

            if audio_files.is_empty() {
                return None;
            }

            // Get user stats from first result
            let (first_result, _) = audio_files[0];

            // Determine primary format and average bitrate
            let format_counts: HashMap<String, usize> =
                audio_files.iter().fold(HashMap::new(), |mut acc, (_, f)| {
                    *acc.entry(f.extension.to_lowercase()).or_default() += 1;
                    acc
                });

            let primary_format = format_counts
                .into_iter()
                .max_by_key(|(_, count)| *count)
                .map(|(fmt, _)| fmt)
                .unwrap_or_else(|| "unknown".to_string());

            let avg_bitrate = {
                let bitrates: Vec<u32> =
                    audio_files.iter().filter_map(|(_, f)| f.bitrate).collect();
                if bitrates.is_empty() {
                    // Estimate based on format
                    match primary_format.as_str() {
                        "flac" => 1000,
                        "mp3" => 320,
                        _ => 256,
                    }
                } else {
                    bitrates.iter().sum::<u32>() / bitrates.len() as u32
                }
            };

            // Build file list
            let file_results: Vec<SoulseekFileResult> = audio_files
                .iter()
                .map(|(_, f)| SoulseekFileResult {
                    filename: f.filename.clone(),
                    size: f.size,
                    extension: f.extension.clone(),
                    bitrate: f.bitrate,
                    duration: f.duration,
                })
                .collect();

            // Calculate quality score
            let score = score_soulseek_result(
                &primary_format,
                avg_bitrate,
                file_results.len(),
                expected_track_count,
                first_result.has_free_slot,
                first_result.average_speed,
                quality_prefs,
            );

            Some(SoulseekAlbumResult {
                source: "soulseek".to_string(),
                username,
                folder,
                files: file_results,
                quality: SoulseekQuality {
                    format: primary_format,
                    bitrate: avg_bitrate,
                },
                user_slots_free: first_result.has_free_slot,
                user_speed: first_result.average_speed,
                queue_length: first_result.queue_length,
                score,
            })
        })
        .collect();

    // Sort by score (highest first)
    album_results.sort_by(|a, b| b.score.cmp(&a.score));

    album_results
}

/// Extracts the folder path from a full file path.
fn extract_folder(path: &str) -> String {
    // Handle both forward and backslashes
    let normalized = path.replace('\\', "/");
    if let Some(last_slash) = normalized.rfind('/') {
        normalized[..last_slash].to_string()
    } else {
        String::new()
    }
}

/// Checks if a file extension is a common audio format.
fn is_audio_extension(ext: &str) -> bool {
    matches!(
        ext.to_lowercase().as_str(),
        "flac" | "mp3" | "m4a" | "aac" | "ogg" | "opus" | "wav" | "wma" | "ape" | "alac"
    )
}

/// Scores a Soulseek result based on quality preferences.
fn score_soulseek_result(
    format: &str,
    bitrate: u32,
    file_count: usize,
    expected_track_count: usize,
    slots_free: bool,
    upload_speed: u32,
    prefs: &MusicQualityConfig,
) -> u32 {
    let mut score: u32 = 0;

    // Format match bonus (max 100 points)
    let format_lower = format.to_lowercase();
    for (i, pref_format) in prefs.preferred_formats.iter().enumerate() {
        if format_lower == pref_format.to_lowercase()
            || (pref_format.to_lowercase().starts_with(&format_lower))
        {
            // Higher score for formats earlier in the preference list
            score += 100 - (i as u32 * 20).min(80);
            break;
        }
    }

    // Lossless format bonus
    if matches!(format_lower.as_str(), "flac" | "wav" | "ape" | "alac") {
        score += 50;
    }

    // Bitrate bonus (max 50 points)
    if bitrate >= prefs.min_bitrate {
        score += 25;
        // Extra points for higher bitrates
        score += ((bitrate as f64 / 1000.0) * 10.0).min(25.0) as u32;
    }

    // Complete album bonus (max 50 points)
    if prefs.prefer_complete_albums && expected_track_count > 0 {
        let completeness = (file_count as f64 / expected_track_count as f64).min(1.0);
        score += (completeness * 50.0) as u32;
    }

    // Availability bonus (max 30 points)
    if slots_free {
        score += 20;
    }

    // Speed bonus (max 10 points)
    // 1MB/s = 1,000,000 bytes/s gets full points
    score += ((upload_speed as f64 / 100000.0) as u32).min(10);

    score
}

/// Maps a database row to an Artist struct.
fn map_artist_row(row: &rusqlite::Row) -> rusqlite::Result<Artist> {
    Ok(Artist {
        id: row.get(0)?,
        mbid: row.get(1)?,
        name: row.get(2)?,
        sort_name: row.get(3)?,
        disambiguation: row.get(4)?,
        artist_type: row.get(5)?,
        country: row.get(6)?,
        begin_date: row.get(7)?,
        end_date: row.get(8)?,
        overview: row.get(9)?,
        image_path: row.get(10)?,
        monitored: row.get(11)?,
        quality_limit: row.get(12)?,
        added_at: row.get(13)?,
        updated_at: row.get(14)?,
        added_by: row.get(15)?,
    })
}

/// Maps a database row to an ArtistWithStats struct.
fn map_artist_with_stats_row(row: &rusqlite::Row) -> rusqlite::Result<ArtistWithStats> {
    Ok(ArtistWithStats {
        artist: Artist {
            id: row.get(0)?,
            mbid: row.get(1)?,
            name: row.get(2)?,
            sort_name: row.get(3)?,
            disambiguation: row.get(4)?,
            artist_type: row.get(5)?,
            country: row.get(6)?,
            begin_date: row.get(7)?,
            end_date: row.get(8)?,
            overview: row.get(9)?,
            image_path: row.get(10)?,
            monitored: row.get(11)?,
            quality_limit: row.get(12)?,
            added_at: row.get(13)?,
            updated_at: row.get(14)?,
            added_by: row.get(15)?,
        },
        album_count: row.get(16)?,
    })
}

/// Maps a database row to an Album struct.
fn map_album_row(row: &rusqlite::Row) -> rusqlite::Result<Album> {
    let status_str: String = row.get(9)?;
    let status = match status_str.as_str() {
        "missing" => AlbumStatus::Missing,
        "searching" => AlbumStatus::Searching,
        "downloading" => AlbumStatus::Downloading,
        "processing" => AlbumStatus::Processing,
        "partial" => AlbumStatus::Partial,
        "available" => AlbumStatus::Available,
        _ => AlbumStatus::Missing,
    };

    Ok(Album {
        id: row.get(0)?,
        mbid: row.get(1)?,
        artist_id: row.get(2)?,
        title: row.get(3)?,
        album_type: row.get(4)?,
        release_date: row.get(5)?,
        overview: row.get(6)?,
        cover_path: row.get(7)?,
        total_tracks: row.get(8)?,
        status,
        monitored: row.get(10)?,
        quality_limit: row.get(11)?,
        added_at: row.get(12)?,
        updated_at: row.get(13)?,
    })
}

/// Maps a database row to a Track struct.
fn map_track_row(row: &rusqlite::Row) -> rusqlite::Result<Track> {
    let status_str: String = row.get(8)?;
    let status = match status_str.as_str() {
        "missing" => MediaStatus::Missing,
        "searching" => MediaStatus::Searching,
        "downloading" => MediaStatus::Downloading,
        "processing" => MediaStatus::Processing,
        "available" => MediaStatus::Available,
        _ => MediaStatus::Missing,
    };

    Ok(Track {
        id: row.get(0)?,
        mbid: row.get(1)?,
        album_id: row.get(2)?,
        artist_id: row.get(3)?,
        title: row.get(4)?,
        track_number: row.get(5)?,
        disc_number: row.get(6)?,
        duration_ms: row.get(7)?,
        status,
        monitored: row.get(9)?,
        file_path: row.get(10)?,
        file_size: row.get(11)?,
        audio_format: row.get(12)?,
        bitrate: row.get(13)?,
        sample_rate: row.get(14)?,
        bit_depth: row.get(15)?,
        created_at: row.get(16)?,
        updated_at: row.get(17)?,
    })
}

/// Calculate album status based on track statuses.
///
/// TODO: Use this function to automatically update album status when:
/// - A track download completes (status changes to Available)
/// - Track status is updated via API
/// - Album refresh discovers new tracks
#[allow(dead_code)]
fn calculate_album_status(tracks: &[Track]) -> AlbumStatus {
    if tracks.is_empty() {
        return AlbumStatus::Missing;
    }

    let available = tracks
        .iter()
        .filter(|t| matches!(t.status, MediaStatus::Available))
        .count();

    if available == 0 {
        AlbumStatus::Missing
    } else if available == tracks.len() {
        AlbumStatus::Available
    } else {
        AlbumStatus::Partial
    }
}

/// Validate that a string is a valid UUID format.
///
/// UUID format: xxxxxxxx-xxxx-xxxx-xxxx-xxxxxxxxxxxx (36 characters with 4 dashes)
/// where x is a hexadecimal digit (0-9, a-f, A-F).
fn is_valid_uuid(s: &str) -> bool {
    if s.len() != 36 {
        return false;
    }

    let parts: Vec<&str> = s.split('-').collect();
    if parts.len() != 5 {
        return false;
    }

    // Expected lengths: 8-4-4-4-12
    let expected_lengths = [8, 4, 4, 4, 12];
    for (part, expected_len) in parts.iter().zip(expected_lengths.iter()) {
        if part.len() != *expected_len {
            return false;
        }
        if !part.chars().all(|c| c.is_ascii_hexdigit()) {
            return false;
        }
    }

    true
}
