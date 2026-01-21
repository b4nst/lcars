//! Movies API endpoints for managing movie collection.

use axum::{
    extract::{Path, Query, State},
    Extension, Json,
};
use serde::{Deserialize, Serialize};

use crate::db::models::{MediaStatus, MediaType, Movie};
use crate::error::{AppError, Result};
use crate::services::indexer::{MediaSearchType, Release, SearchQuery as IndexerSearchQuery};
use crate::services::Claims;
use crate::AppState;

// =============================================================================
// Request/Response Types
// =============================================================================

/// Query parameters for listing movies.
#[derive(Debug, Deserialize)]
pub struct ListMoviesQuery {
    /// Filter by status (missing, searching, downloading, processing, available).
    pub status: Option<MediaStatus>,
    /// Filter by monitored state.
    pub monitored: Option<bool>,
    /// Full-text search query.
    pub search: Option<String>,
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

/// Request body for adding a movie.
#[derive(Debug, Deserialize)]
pub struct AddMovieRequest {
    /// TMDB movie ID.
    pub tmdb_id: i32,
    /// Whether to monitor this movie (default: true).
    pub monitored: Option<bool>,
    /// Quality limit for downloads (default: "1080p").
    pub quality_limit: Option<String>,
}

/// Request body for updating a movie.
#[derive(Debug, Deserialize)]
pub struct UpdateMovieRequest {
    /// Whether to monitor this movie.
    pub monitored: Option<bool>,
    /// Quality limit for downloads.
    pub quality_limit: Option<String>,
}

/// Query parameters for deleting a movie.
#[derive(Debug, Deserialize)]
pub struct DeleteMovieQuery {
    /// Whether to delete associated files (default: false).
    pub delete_files: Option<bool>,
}

/// Request body for downloading a release.
#[derive(Debug, Deserialize)]
pub struct DownloadRequest {
    /// Direct magnet link.
    pub magnet: String,
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

// =============================================================================
// Handlers
// =============================================================================

/// GET /api/movies
///
/// Lists all movies with optional filtering and pagination.
pub async fn list_movies(
    State(state): State<AppState>,
    Query(query): Query<ListMoviesQuery>,
) -> Result<Json<PaginatedResponse<Movie>>> {
    let page = query.page.unwrap_or(1).clamp(1, u32::MAX);
    let limit = query.limit.unwrap_or(20).clamp(1, 100);
    let offset = (page - 1).saturating_mul(limit);

    let db = state.db.lock().await;

    // Build the query based on whether we have a search term
    let (items, total) = if let Some(ref search) = query.search {
        // FTS search query - escape double quotes to prevent FTS syntax errors
        let sanitized = search.trim().replace('"', "\"\"");
        let search_term = format!("\"{}\"*", sanitized);

        // Count total matching items
        let total: u64 = db.query_row(
            r#"
            SELECT COUNT(*) FROM movies m
            JOIN movies_fts fts ON m.id = fts.rowid
            WHERE movies_fts MATCH ?1
              AND (?2 IS NULL OR m.status = ?2)
              AND (?3 IS NULL OR m.monitored = ?3)
            "#,
            rusqlite::params![
                search_term,
                query.status.as_ref().map(|s| s.to_string()),
                query.monitored,
            ],
            |row| row.get(0),
        )?;

        // Fetch matching items
        let mut stmt = db.prepare(
            r#"
            SELECT m.id, m.tmdb_id, m.imdb_id, m.title, m.original_title, m.year,
                   m.overview, m.poster_path, m.backdrop_path, m.runtime_minutes,
                   m.genres, m.status, m.monitored, m.quality_limit, m.file_path,
                   m.file_size, m.added_at, m.updated_at, m.added_by
            FROM movies m
            JOIN movies_fts fts ON m.id = fts.rowid
            WHERE movies_fts MATCH ?1
              AND (?2 IS NULL OR m.status = ?2)
              AND (?3 IS NULL OR m.monitored = ?3)
            ORDER BY m.added_at DESC
            LIMIT ?4 OFFSET ?5
            "#,
        )?;

        let items = stmt
            .query_map(
                rusqlite::params![
                    search_term,
                    query.status.as_ref().map(|s| s.to_string()),
                    query.monitored,
                    limit,
                    offset,
                ],
                map_movie_row,
            )?
            .collect::<std::result::Result<Vec<_>, _>>()?;

        (items, total)
    } else {
        // Standard query without FTS
        let total: u64 = db.query_row(
            r#"
            SELECT COUNT(*) FROM movies
            WHERE (?1 IS NULL OR status = ?1)
              AND (?2 IS NULL OR monitored = ?2)
            "#,
            rusqlite::params![
                query.status.as_ref().map(|s| s.to_string()),
                query.monitored,
            ],
            |row| row.get(0),
        )?;

        let mut stmt = db.prepare(
            r#"
            SELECT id, tmdb_id, imdb_id, title, original_title, year,
                   overview, poster_path, backdrop_path, runtime_minutes,
                   genres, status, monitored, quality_limit, file_path,
                   file_size, added_at, updated_at, added_by
            FROM movies
            WHERE (?1 IS NULL OR status = ?1)
              AND (?2 IS NULL OR monitored = ?2)
            ORDER BY added_at DESC
            LIMIT ?3 OFFSET ?4
            "#,
        )?;

        let items = stmt
            .query_map(
                rusqlite::params![
                    query.status.as_ref().map(|s| s.to_string()),
                    query.monitored,
                    limit,
                    offset,
                ],
                map_movie_row,
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

/// POST /api/movies
///
/// Adds a new movie by TMDB ID.
pub async fn add_movie(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Json(body): Json<AddMovieRequest>,
) -> Result<Json<Movie>> {
    // Validate TMDB ID
    if body.tmdb_id <= 0 {
        return Err(AppError::BadRequest("Invalid TMDB ID".to_string()));
    }

    // Get TMDB client
    let tmdb_client = state
        .tmdb_client()
        .ok_or_else(|| AppError::Internal("TMDB client not configured".to_string()))?;

    // Fetch movie details from TMDB
    let tmdb_movie = tmdb_client.get_movie(body.tmdb_id).await?;

    // Extract year from release_date
    let year = tmdb_movie
        .release_date
        .as_ref()
        .and_then(|d| d.split('-').next())
        .and_then(|y| y.parse::<i32>().ok())
        .unwrap_or(0);

    // Serialize genres to JSON
    let genres = serde_json::to_string(
        &tmdb_movie
            .genres
            .iter()
            .map(|g| &g.name)
            .collect::<Vec<_>>(),
    )
    .ok();

    let monitored = body.monitored.unwrap_or(true);
    let quality_limit = body.quality_limit.unwrap_or_else(|| "1080p".to_string());

    let db = state.db.lock().await;

    // Check if movie already exists
    let exists: bool = db.query_row(
        "SELECT EXISTS(SELECT 1 FROM movies WHERE tmdb_id = ?1)",
        [body.tmdb_id],
        |row| row.get(0),
    )?;

    if exists {
        return Err(AppError::BadRequest(format!(
            "Movie with TMDB ID {} already exists",
            body.tmdb_id
        )));
    }

    // Insert the movie
    db.execute(
        r#"
        INSERT INTO movies (
            tmdb_id, imdb_id, title, original_title, year, overview,
            poster_path, backdrop_path, runtime_minutes, genres,
            status, monitored, quality_limit, added_by
        ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, 'missing', ?11, ?12, ?13)
        "#,
        rusqlite::params![
            body.tmdb_id,
            tmdb_movie.imdb_id,
            tmdb_movie.title,
            tmdb_movie.original_title,
            year,
            tmdb_movie.overview,
            tmdb_movie.poster_path,
            tmdb_movie.backdrop_path,
            tmdb_movie.runtime,
            genres,
            monitored,
            quality_limit,
            claims.sub,
        ],
    )?;

    let movie_id = db.last_insert_rowid();

    // Fetch the created movie
    let movie = db.query_row(
        r#"
        SELECT id, tmdb_id, imdb_id, title, original_title, year,
               overview, poster_path, backdrop_path, runtime_minutes,
               genres, status, monitored, quality_limit, file_path,
               file_size, added_at, updated_at, added_by
        FROM movies WHERE id = ?1
        "#,
        [movie_id],
        map_movie_row,
    )?;

    tracing::info!(
        movie_id = movie.id,
        tmdb_id = movie.tmdb_id,
        title = %movie.title,
        added_by = claims.sub,
        "Movie added"
    );

    Ok(Json(movie))
}

/// GET /api/movies/:id
///
/// Gets a single movie by ID.
pub async fn get_movie(
    State(state): State<AppState>,
    Path(movie_id): Path<i64>,
) -> Result<Json<Movie>> {
    let db = state.db.lock().await;

    let movie = db
        .query_row(
            r#"
            SELECT id, tmdb_id, imdb_id, title, original_title, year,
                   overview, poster_path, backdrop_path, runtime_minutes,
                   genres, status, monitored, quality_limit, file_path,
                   file_size, added_at, updated_at, added_by
            FROM movies WHERE id = ?1
            "#,
            [movie_id],
            map_movie_row,
        )
        .map_err(|e| match e {
            rusqlite::Error::QueryReturnedNoRows => {
                AppError::NotFound("Movie not found".to_string())
            }
            _ => AppError::Sqlite(e),
        })?;

    Ok(Json(movie))
}

/// PUT /api/movies/:id
///
/// Updates a movie's settings.
pub async fn update_movie(
    State(state): State<AppState>,
    Path(movie_id): Path<i64>,
    Json(body): Json<UpdateMovieRequest>,
) -> Result<Json<Movie>> {
    let db = state.db.lock().await;

    // Check if movie exists
    let _: i64 = db
        .query_row("SELECT id FROM movies WHERE id = ?1", [movie_id], |row| {
            row.get(0)
        })
        .map_err(|e| match e {
            rusqlite::Error::QueryReturnedNoRows => {
                AppError::NotFound("Movie not found".to_string())
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
    let query = format!("UPDATE movies SET {} WHERE id = ?", updates.join(", "));
    params.push(Box::new(movie_id));

    let param_refs: Vec<&dyn rusqlite::ToSql> = params.iter().map(|p| p.as_ref()).collect();
    db.execute(&query, param_refs.as_slice())?;

    // Fetch the updated movie
    let movie = db.query_row(
        r#"
        SELECT id, tmdb_id, imdb_id, title, original_title, year,
               overview, poster_path, backdrop_path, runtime_minutes,
               genres, status, monitored, quality_limit, file_path,
               file_size, added_at, updated_at, added_by
        FROM movies WHERE id = ?1
        "#,
        [movie_id],
        map_movie_row,
    )?;

    tracing::info!(movie_id = movie.id, "Movie updated");

    Ok(Json(movie))
}

/// DELETE /api/movies/:id
///
/// Deletes a movie from the database.
pub async fn delete_movie(
    State(state): State<AppState>,
    Path(movie_id): Path<i64>,
    Query(query): Query<DeleteMovieQuery>,
) -> Result<Json<SuccessResponse>> {
    let db = state.db.lock().await;

    // Get movie to check existence and file path
    let (file_path,): (Option<String>,) = db
        .query_row(
            "SELECT file_path FROM movies WHERE id = ?1",
            [movie_id],
            |row| Ok((row.get(0)?,)),
        )
        .map_err(|e| match e {
            rusqlite::Error::QueryReturnedNoRows => {
                AppError::NotFound("Movie not found".to_string())
            }
            _ => AppError::Sqlite(e),
        })?;

    // Delete file if requested and file exists
    if query.delete_files.unwrap_or(false) {
        if let Some(ref path) = file_path {
            let path = std::path::Path::new(path);
            if path.exists() {
                if let Err(e) = std::fs::remove_file(path) {
                    tracing::warn!(
                        movie_id = movie_id,
                        path = %path.display(),
                        error = %e,
                        "Failed to delete movie file"
                    );
                } else {
                    tracing::info!(
                        movie_id = movie_id,
                        path = %path.display(),
                        "Deleted movie file"
                    );
                }
            }
        }
    }

    // Delete from database
    db.execute("DELETE FROM movies WHERE id = ?1", [movie_id])?;

    tracing::info!(
        movie_id = movie_id,
        delete_files = query.delete_files.unwrap_or(false),
        "Movie deleted"
    );

    Ok(Json(SuccessResponse {
        success: true,
        message: Some("Movie deleted successfully".to_string()),
    }))
}

/// POST /api/movies/:id/search
///
/// Searches indexers for releases of this movie.
pub async fn search_releases(
    State(state): State<AppState>,
    Path(movie_id): Path<i64>,
) -> Result<Json<Vec<Release>>> {
    let db = state.db.lock().await;

    // Get movie details
    let movie = db
        .query_row(
            r#"
            SELECT id, tmdb_id, imdb_id, title, original_title, year,
                   overview, poster_path, backdrop_path, runtime_minutes,
                   genres, status, monitored, quality_limit, file_path,
                   file_size, added_at, updated_at, added_by
            FROM movies WHERE id = ?1
            "#,
            [movie_id],
            map_movie_row,
        )
        .map_err(|e| match e {
            rusqlite::Error::QueryReturnedNoRows => {
                AppError::NotFound("Movie not found".to_string())
            }
            _ => AppError::Sqlite(e),
        })?;

    drop(db); // Release the lock before async operations

    // Build search query
    let mut query = IndexerSearchQuery::new(&movie.title)
        .media_type(MediaSearchType::Movie)
        .year(movie.year);

    if let Some(ref imdb_id) = movie.imdb_id {
        query = query.imdb_id(imdb_id);
    }

    // Search indexers
    let indexer_manager = state.indexer_manager();
    let releases = indexer_manager.search(&query).await?;

    tracing::info!(
        movie_id = movie_id,
        title = %movie.title,
        results = releases.len(),
        "Searched releases for movie"
    );

    Ok(Json(releases))
}

/// POST /api/movies/:id/download
///
/// Starts downloading a release for this movie.
pub async fn download_release(
    State(state): State<AppState>,
    Path(movie_id): Path<i64>,
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

    // Check movie exists and get title
    let (title,): (String,) = db
        .query_row(
            "SELECT title FROM movies WHERE id = ?1",
            [movie_id],
            |row| Ok((row.get(0)?,)),
        )
        .map_err(|e| match e {
            rusqlite::Error::QueryReturnedNoRows => {
                AppError::NotFound("Movie not found".to_string())
            }
            _ => AppError::Sqlite(e),
        })?;

    drop(db); // Release lock before async operation

    // Add magnet to torrent engine
    let media_ref = crate::services::torrent::MediaRef {
        media_type: MediaType::Movie,
        media_id: movie_id,
    };

    let info_hash = torrent_engine.add_magnet(&body.magnet, media_ref).await?;

    // Create download record and update movie status
    let db = state.db.lock().await;

    db.execute(
        r#"
        INSERT INTO downloads (source_type, source_id, name, media_type, media_id, source_uri, status)
        VALUES ('torrent', ?1, ?2, 'movie', ?3, ?4, 'downloading')
        "#,
        rusqlite::params![info_hash, title, movie_id, body.magnet],
    )?;

    let download_id = db.last_insert_rowid();

    // Update movie status
    db.execute(
        "UPDATE movies SET status = 'downloading', updated_at = datetime('now') WHERE id = ?1",
        [movie_id],
    )?;

    tracing::info!(
        movie_id = movie_id,
        info_hash = %info_hash,
        download_id = download_id,
        "Started movie download"
    );

    Ok(Json(DownloadInfo {
        id: download_id,
        info_hash,
        name: title,
        status: "downloading".to_string(),
    }))
}

/// POST /api/movies/:id/refresh
///
/// Refreshes movie metadata from TMDB.
pub async fn refresh_metadata(
    State(state): State<AppState>,
    Path(movie_id): Path<i64>,
) -> Result<Json<Movie>> {
    // Get TMDB client
    let tmdb_client = state
        .tmdb_client()
        .ok_or_else(|| AppError::Internal("TMDB client not configured".to_string()))?;

    let db = state.db.lock().await;

    // Get movie's TMDB ID
    let (tmdb_id,): (i64,) = db
        .query_row(
            "SELECT tmdb_id FROM movies WHERE id = ?1",
            [movie_id],
            |row| Ok((row.get(0)?,)),
        )
        .map_err(|e| match e {
            rusqlite::Error::QueryReturnedNoRows => {
                AppError::NotFound("Movie not found".to_string())
            }
            _ => AppError::Sqlite(e),
        })?;

    drop(db); // Release lock before async operation

    // Fetch fresh data from TMDB
    let tmdb_movie = tmdb_client.get_movie(tmdb_id as i32).await?;

    // Extract year from release_date
    let year = tmdb_movie
        .release_date
        .as_ref()
        .and_then(|d| d.split('-').next())
        .and_then(|y| y.parse::<i32>().ok())
        .unwrap_or(0);

    // Serialize genres to JSON
    let genres = serde_json::to_string(
        &tmdb_movie
            .genres
            .iter()
            .map(|g| &g.name)
            .collect::<Vec<_>>(),
    )
    .ok();

    let db = state.db.lock().await;

    // Update movie with fresh metadata
    db.execute(
        r#"
        UPDATE movies SET
            imdb_id = ?1,
            title = ?2,
            original_title = ?3,
            year = ?4,
            overview = ?5,
            poster_path = ?6,
            backdrop_path = ?7,
            runtime_minutes = ?8,
            genres = ?9,
            updated_at = datetime('now')
        WHERE id = ?10
        "#,
        rusqlite::params![
            tmdb_movie.imdb_id,
            tmdb_movie.title,
            tmdb_movie.original_title,
            year,
            tmdb_movie.overview,
            tmdb_movie.poster_path,
            tmdb_movie.backdrop_path,
            tmdb_movie.runtime,
            genres,
            movie_id,
        ],
    )?;

    // Fetch the updated movie
    let movie = db.query_row(
        r#"
        SELECT id, tmdb_id, imdb_id, title, original_title, year,
               overview, poster_path, backdrop_path, runtime_minutes,
               genres, status, monitored, quality_limit, file_path,
               file_size, added_at, updated_at, added_by
        FROM movies WHERE id = ?1
        "#,
        [movie_id],
        map_movie_row,
    )?;

    tracing::info!(
        movie_id = movie.id,
        title = %movie.title,
        "Movie metadata refreshed"
    );

    Ok(Json(movie))
}

// =============================================================================
// Helpers
// =============================================================================

/// Maps a database row to a Movie struct.
fn map_movie_row(row: &rusqlite::Row) -> rusqlite::Result<Movie> {
    let status_str: String = row.get(11)?;
    let status = match status_str.as_str() {
        "missing" => MediaStatus::Missing,
        "searching" => MediaStatus::Searching,
        "downloading" => MediaStatus::Downloading,
        "processing" => MediaStatus::Processing,
        "available" => MediaStatus::Available,
        _ => MediaStatus::Missing,
    };

    Ok(Movie {
        id: row.get(0)?,
        tmdb_id: row.get(1)?,
        imdb_id: row.get(2)?,
        title: row.get(3)?,
        original_title: row.get(4)?,
        year: row.get(5)?,
        overview: row.get(6)?,
        poster_path: row.get(7)?,
        backdrop_path: row.get(8)?,
        runtime_minutes: row.get(9)?,
        genres: row.get(10)?,
        status,
        monitored: row.get(12)?,
        quality_limit: row.get(13)?,
        file_path: row.get(14)?,
        file_size: row.get(15)?,
        added_at: row.get(16)?,
        updated_at: row.get(17)?,
        added_by: row.get(18)?,
    })
}
