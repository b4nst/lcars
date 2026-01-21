//! TV Shows API endpoints for managing TV show collection.

use axum::{
    extract::{Path, Query, State},
    routing::{get, post, put},
    Extension, Json, Router,
};
use futures::future::join_all;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

use crate::db::models::{Episode, MediaStatus, MediaType, ShowStatus, TvShow};
use crate::error::{AppError, Result};
use crate::middleware;
use crate::services::indexer::{MediaSearchType, Release, SearchQuery as IndexerSearchQuery};
use crate::services::tmdb::TmdbSeason;
use crate::services::Claims;
use crate::AppState;

// =============================================================================
// Request/Response Types
// =============================================================================

/// Query parameters for listing TV shows.
#[derive(Debug, Deserialize)]
pub struct ListShowsQuery {
    /// Filter by show status (continuing, ended, canceled, upcoming).
    pub status: Option<ShowStatus>,
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

/// Request body for adding a TV show.
#[derive(Debug, Deserialize)]
pub struct AddShowRequest {
    /// TMDB TV show ID.
    pub tmdb_id: i32,
    /// Whether to monitor this show (default: true).
    pub monitored: Option<bool>,
    /// Quality limit for downloads (default: "1080p").
    pub quality_limit: Option<String>,
}

/// Request body for updating a TV show.
#[derive(Debug, Deserialize)]
pub struct UpdateShowRequest {
    /// Whether to monitor this show.
    pub monitored: Option<bool>,
    /// Quality limit for downloads.
    pub quality_limit: Option<String>,
}

/// Request body for updating a season (batch update all episodes).
#[derive(Debug, Deserialize)]
pub struct UpdateSeasonRequest {
    /// Whether to monitor all episodes in this season.
    pub monitored: Option<bool>,
}

/// Request body for updating an episode.
#[derive(Debug, Deserialize)]
pub struct UpdateEpisodeRequest {
    /// Whether to monitor this episode.
    pub monitored: Option<bool>,
}

/// Query parameters for deleting a show.
#[derive(Debug, Deserialize)]
pub struct DeleteShowQuery {
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

/// A season with its episodes grouped together.
#[derive(Debug, Serialize)]
pub struct SeasonWithEpisodes {
    pub season_number: i32,
    pub episodes: Vec<Episode>,
    /// Computed: count of episodes with status = 'available'
    pub available_count: i32,
    /// Computed: total episode count
    pub total_count: i32,
}

/// A TV show with all seasons and episodes grouped.
#[derive(Debug, Serialize)]
pub struct ShowWithSeasons {
    #[serde(flatten)]
    pub show: TvShow,
    pub seasons: Vec<SeasonWithEpisodes>,
}

// =============================================================================
// Router
// =============================================================================

/// Create the TV shows router with all routes.
pub fn router(state: AppState) -> Router<AppState> {
    Router::new()
        .route("/", get(list_shows).post(add_show))
        .route("/{id}", get(get_show).put(update_show).delete(delete_show))
        .route("/{id}/refresh", post(refresh_metadata))
        .route("/{id}/season/{season}", get(get_season).put(update_season))
        .route(
            "/{id}/season/{season}/episode/{episode}",
            put(update_episode),
        )
        .route(
            "/{id}/season/{season}/episode/{episode}/search",
            post(search_episode),
        )
        .route(
            "/{id}/season/{season}/episode/{episode}/download",
            post(download_episode),
        )
        .layer(axum::middleware::from_fn_with_state(
            state,
            middleware::auth_middleware,
        ))
}

// =============================================================================
// Handlers
// =============================================================================

/// GET /api/tv
///
/// Lists all TV shows with optional filtering and pagination.
pub async fn list_shows(
    State(state): State<AppState>,
    Query(query): Query<ListShowsQuery>,
) -> Result<Json<PaginatedResponse<TvShow>>> {
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
            SELECT COUNT(*) FROM tv_shows s
            JOIN tv_shows_fts fts ON s.id = fts.rowid
            WHERE tv_shows_fts MATCH ?1
              AND (?2 IS NULL OR s.status = ?2)
              AND (?3 IS NULL OR s.monitored = ?3)
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
            SELECT s.id, s.tmdb_id, s.imdb_id, s.title, s.original_title, s.year_start,
                   s.year_end, s.overview, s.poster_path, s.backdrop_path, s.status,
                   s.monitored, s.quality_limit, s.added_at, s.updated_at, s.added_by
            FROM tv_shows s
            JOIN tv_shows_fts fts ON s.id = fts.rowid
            WHERE tv_shows_fts MATCH ?1
              AND (?2 IS NULL OR s.status = ?2)
              AND (?3 IS NULL OR s.monitored = ?3)
            ORDER BY s.added_at DESC
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
                map_show_row,
            )?
            .collect::<std::result::Result<Vec<_>, _>>()?;

        (items, total)
    } else {
        // Standard query without FTS
        let total: u64 = db.query_row(
            r#"
            SELECT COUNT(*) FROM tv_shows
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
            SELECT id, tmdb_id, imdb_id, title, original_title, year_start,
                   year_end, overview, poster_path, backdrop_path, status,
                   monitored, quality_limit, added_at, updated_at, added_by
            FROM tv_shows
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
                map_show_row,
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

/// POST /api/tv
///
/// Adds a new TV show by TMDB ID.
pub async fn add_show(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Json(body): Json<AddShowRequest>,
) -> Result<Json<ShowWithSeasons>> {
    // Validate TMDB ID
    if body.tmdb_id <= 0 {
        return Err(AppError::BadRequest("Invalid TMDB ID".to_string()));
    }

    // Get TMDB client
    let tmdb_client = state
        .tmdb_client()
        .ok_or_else(|| AppError::Internal("TMDB client not configured".to_string()))?;

    // Fetch show details from TMDB
    let tmdb_show = tmdb_client.get_tv(body.tmdb_id).await?;

    // Extract years from air dates
    let year_start = tmdb_show
        .first_air_date
        .as_ref()
        .and_then(|d| d.split('-').next())
        .and_then(|y| y.parse::<i32>().ok());

    let year_end = tmdb_show
        .last_air_date
        .as_ref()
        .and_then(|d| d.split('-').next())
        .and_then(|y| y.parse::<i32>().ok());

    // Parse show status from TMDB
    let show_status = parse_tmdb_status(tmdb_show.status.as_deref());

    // Get IMDB ID from external IDs
    let imdb_id = tmdb_show
        .external_ids
        .as_ref()
        .and_then(|e| e.imdb_id.clone());

    let monitored = body.monitored.unwrap_or(true);
    let quality_limit = body.quality_limit.unwrap_or_else(|| "1080p".to_string());

    let db = state.db.lock().await;

    // Check if show already exists
    let exists: bool = db.query_row(
        "SELECT EXISTS(SELECT 1 FROM tv_shows WHERE tmdb_id = ?1)",
        [body.tmdb_id],
        |row| row.get(0),
    )?;

    if exists {
        return Err(AppError::BadRequest(format!(
            "TV show with TMDB ID {} already exists",
            body.tmdb_id
        )));
    }

    // Insert the show
    db.execute(
        r#"
        INSERT INTO tv_shows (
            tmdb_id, imdb_id, title, original_title, year_start, year_end,
            overview, poster_path, backdrop_path, status, monitored, quality_limit, added_by
        ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)
        "#,
        rusqlite::params![
            body.tmdb_id,
            imdb_id,
            tmdb_show.name,
            tmdb_show.original_name,
            year_start,
            year_end,
            tmdb_show.overview,
            tmdb_show.poster_path,
            tmdb_show.backdrop_path,
            show_status.to_string(),
            monitored,
            quality_limit,
            claims.sub,
        ],
    )?;

    let show_id = db.last_insert_rowid();

    drop(db); // Release lock for async operations

    // Fetch all seasons concurrently for better performance
    let season_futures: Vec<_> = tmdb_show
        .seasons
        .iter()
        .filter(|s| !(s.season_number == 0 && s.episode_count == 0))
        .map(|s| {
            let tmdb_id = body.tmdb_id;
            let season_number = s.season_number;
            async move {
                let result = tmdb_client.get_season(tmdb_id, season_number).await;
                (season_number, result)
            }
        })
        .collect();

    let season_results: Vec<(i32, std::result::Result<TmdbSeason, AppError>)> =
        join_all(season_futures).await;

    // Collect all episodes from successful season fetches
    let mut episodes_to_insert = Vec::new();
    for (season_number, result) in season_results {
        match result {
            Ok(season) => {
                for ep in season.episodes {
                    episodes_to_insert.push(ep);
                }
            }
            Err(e) => {
                tracing::warn!(
                    show_id = body.tmdb_id,
                    season = season_number,
                    error = %e,
                    "Failed to fetch season details, skipping"
                );
            }
        }
    }

    // Insert all episodes in a single database lock acquisition
    let db = state.db.lock().await;
    let mut all_episodes: Vec<Episode> = Vec::new();

    for ep in &episodes_to_insert {
        db.execute(
            r#"
            INSERT INTO episodes (
                show_id, tmdb_id, season_number, episode_number, title,
                overview, air_date, runtime_minutes, still_path, status, monitored
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, 'missing', ?10)
            "#,
            rusqlite::params![
                show_id,
                ep.id,
                ep.season_number,
                ep.episode_number,
                ep.name,
                ep.overview,
                ep.air_date,
                ep.runtime,
                ep.still_path,
                monitored,
            ],
        )?;

        let episode_id = db.last_insert_rowid();
        all_episodes.push(Episode {
            id: episode_id,
            show_id,
            tmdb_id: Some(ep.id as i64),
            season_number: ep.season_number,
            episode_number: ep.episode_number,
            title: Some(ep.name.clone()),
            overview: ep.overview.clone(),
            air_date: ep.air_date.clone(),
            runtime_minutes: ep.runtime,
            still_path: ep.still_path.clone(),
            status: MediaStatus::Missing,
            monitored,
            file_path: None,
            file_size: None,
            created_at: String::new(),
            updated_at: String::new(),
        });
    }
    let show = db.query_row(
        r#"
        SELECT id, tmdb_id, imdb_id, title, original_title, year_start,
               year_end, overview, poster_path, backdrop_path, status,
               monitored, quality_limit, added_at, updated_at, added_by
        FROM tv_shows WHERE id = ?1
        "#,
        [show_id],
        map_show_row,
    )?;

    let seasons = group_episodes_by_season(all_episodes);

    tracing::info!(
        show_id = show.id,
        tmdb_id = show.tmdb_id,
        title = %show.title,
        seasons = seasons.len(),
        episodes = seasons.iter().map(|s| s.total_count).sum::<i32>(),
        added_by = claims.sub,
        "TV show added"
    );

    Ok(Json(ShowWithSeasons { show, seasons }))
}

/// GET /api/tv/:id
///
/// Gets a single TV show by ID with all episodes grouped by season.
pub async fn get_show(
    State(state): State<AppState>,
    Path(show_id): Path<i64>,
) -> Result<Json<ShowWithSeasons>> {
    let db = state.db.lock().await;

    let show = db
        .query_row(
            r#"
            SELECT id, tmdb_id, imdb_id, title, original_title, year_start,
                   year_end, overview, poster_path, backdrop_path, status,
                   monitored, quality_limit, added_at, updated_at, added_by
            FROM tv_shows WHERE id = ?1
            "#,
            [show_id],
            map_show_row,
        )
        .map_err(|e| match e {
            rusqlite::Error::QueryReturnedNoRows => {
                AppError::NotFound("TV show not found".to_string())
            }
            _ => AppError::Sqlite(e),
        })?;

    // Fetch all episodes for this show
    let mut stmt = db.prepare(
        r#"
        SELECT id, show_id, tmdb_id, season_number, episode_number, title,
               overview, air_date, runtime_minutes, still_path, status,
               monitored, file_path, file_size, created_at, updated_at
        FROM episodes
        WHERE show_id = ?1
        ORDER BY season_number, episode_number
        "#,
    )?;

    let episodes = stmt
        .query_map([show_id], map_episode_row)?
        .collect::<std::result::Result<Vec<_>, _>>()?;

    let seasons = group_episodes_by_season(episodes);

    Ok(Json(ShowWithSeasons { show, seasons }))
}

/// PUT /api/tv/:id
///
/// Updates a TV show's settings.
pub async fn update_show(
    State(state): State<AppState>,
    Path(show_id): Path<i64>,
    Json(body): Json<UpdateShowRequest>,
) -> Result<Json<TvShow>> {
    let db = state.db.lock().await;

    // Check if show exists
    let _: i64 = db
        .query_row("SELECT id FROM tv_shows WHERE id = ?1", [show_id], |row| {
            row.get(0)
        })
        .map_err(|e| match e {
            rusqlite::Error::QueryReturnedNoRows => {
                AppError::NotFound("TV show not found".to_string())
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
    let query = format!("UPDATE tv_shows SET {} WHERE id = ?", updates.join(", "));
    params.push(Box::new(show_id));

    let param_refs: Vec<&dyn rusqlite::ToSql> = params.iter().map(|p| p.as_ref()).collect();
    db.execute(&query, param_refs.as_slice())?;

    // Fetch the updated show
    let show = db.query_row(
        r#"
        SELECT id, tmdb_id, imdb_id, title, original_title, year_start,
               year_end, overview, poster_path, backdrop_path, status,
               monitored, quality_limit, added_at, updated_at, added_by
        FROM tv_shows WHERE id = ?1
        "#,
        [show_id],
        map_show_row,
    )?;

    tracing::info!(show_id = show.id, "TV show updated");

    Ok(Json(show))
}

/// DELETE /api/tv/:id
///
/// Deletes a TV show from the database.
pub async fn delete_show(
    State(state): State<AppState>,
    Path(show_id): Path<i64>,
    Query(query): Query<DeleteShowQuery>,
) -> Result<Json<SuccessResponse>> {
    let db = state.db.lock().await;

    // Check show exists
    let _: i64 = db
        .query_row("SELECT id FROM tv_shows WHERE id = ?1", [show_id], |row| {
            row.get(0)
        })
        .map_err(|e| match e {
            rusqlite::Error::QueryReturnedNoRows => {
                AppError::NotFound("TV show not found".to_string())
            }
            _ => AppError::Sqlite(e),
        })?;

    // Delete files if requested
    if query.delete_files.unwrap_or(false) {
        let mut stmt = db.prepare(
            "SELECT file_path FROM episodes WHERE show_id = ?1 AND file_path IS NOT NULL",
        )?;

        let file_paths: Vec<String> = stmt
            .query_map([show_id], |row| row.get(0))?
            .filter_map(|r| r.ok())
            .collect();

        for path in file_paths {
            let path = std::path::Path::new(&path);
            match std::fs::remove_file(path) {
                Ok(_) => {
                    tracing::info!(
                        show_id = show_id,
                        path = %path.display(),
                        "Deleted episode file"
                    );
                }
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                    // File already deleted, that's fine
                }
                Err(e) => {
                    tracing::warn!(
                        show_id = show_id,
                        path = %path.display(),
                        error = %e,
                        "Failed to delete episode file"
                    );
                }
            }
        }
    }

    // Delete from database (CASCADE handles episodes)
    db.execute("DELETE FROM tv_shows WHERE id = ?1", [show_id])?;

    tracing::info!(
        show_id = show_id,
        delete_files = query.delete_files.unwrap_or(false),
        "TV show deleted"
    );

    Ok(Json(SuccessResponse {
        success: true,
        message: Some("TV show deleted successfully".to_string()),
    }))
}

/// POST /api/tv/:id/refresh
///
/// Refreshes TV show metadata from TMDB.
pub async fn refresh_metadata(
    State(state): State<AppState>,
    Path(show_id): Path<i64>,
) -> Result<Json<ShowWithSeasons>> {
    // Get TMDB client
    let tmdb_client = state
        .tmdb_client()
        .ok_or_else(|| AppError::Internal("TMDB client not configured".to_string()))?;

    let db = state.db.lock().await;

    // Get show's TMDB ID and monitored status
    let (tmdb_id, show_monitored): (i64, bool) = db
        .query_row(
            "SELECT tmdb_id, monitored FROM tv_shows WHERE id = ?1",
            [show_id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .map_err(|e| match e {
            rusqlite::Error::QueryReturnedNoRows => {
                AppError::NotFound("TV show not found".to_string())
            }
            _ => AppError::Sqlite(e),
        })?;

    // Get existing episode IDs for this show
    let existing_episodes: std::collections::HashSet<(i32, i32)> = {
        let mut stmt =
            db.prepare("SELECT season_number, episode_number FROM episodes WHERE show_id = ?1")?;
        let result: std::collections::HashSet<(i32, i32)> = stmt
            .query_map([show_id], |row| Ok((row.get(0)?, row.get(1)?)))?
            .filter_map(|r| r.ok())
            .collect();
        result
    };

    drop(db); // Release lock before async operation

    // Fetch fresh data from TMDB
    let tmdb_show = tmdb_client.get_tv(tmdb_id as i32).await?;

    // Extract years from air dates
    let year_start = tmdb_show
        .first_air_date
        .as_ref()
        .and_then(|d| d.split('-').next())
        .and_then(|y| y.parse::<i32>().ok());

    let year_end = tmdb_show
        .last_air_date
        .as_ref()
        .and_then(|d| d.split('-').next())
        .and_then(|y| y.parse::<i32>().ok());

    let show_status = parse_tmdb_status(tmdb_show.status.as_deref());
    let imdb_id = tmdb_show
        .external_ids
        .as_ref()
        .and_then(|e| e.imdb_id.clone());

    let db = state.db.lock().await;

    // Update show with fresh metadata
    db.execute(
        r#"
        UPDATE tv_shows SET
            imdb_id = ?1,
            title = ?2,
            original_title = ?3,
            year_start = ?4,
            year_end = ?5,
            overview = ?6,
            poster_path = ?7,
            backdrop_path = ?8,
            status = ?9,
            updated_at = datetime('now')
        WHERE id = ?10
        "#,
        rusqlite::params![
            imdb_id,
            tmdb_show.name,
            tmdb_show.original_name,
            year_start,
            year_end,
            tmdb_show.overview,
            tmdb_show.poster_path,
            tmdb_show.backdrop_path,
            show_status.to_string(),
            show_id,
        ],
    )?;

    drop(db);

    // Fetch all seasons concurrently for better performance
    let season_futures: Vec<_> = tmdb_show
        .seasons
        .iter()
        .filter(|s| !(s.season_number == 0 && s.episode_count == 0))
        .map(|s| {
            let season_number = s.season_number;
            async move {
                let result = tmdb_client.get_season(tmdb_id as i32, season_number).await;
                (season_number, result)
            }
        })
        .collect();

    let season_results: Vec<(i32, std::result::Result<TmdbSeason, AppError>)> =
        join_all(season_futures).await;

    // Collect all episodes from successful season fetches
    let mut all_tmdb_episodes = Vec::new();
    for (season_number, result) in season_results {
        match result {
            Ok(season) => {
                for ep in season.episodes {
                    all_tmdb_episodes.push(ep);
                }
            }
            Err(e) => {
                tracing::warn!(
                    show_id = show_id,
                    season = season_number,
                    error = %e,
                    "Failed to fetch season for refresh"
                );
            }
        }
    }

    // Update existing and insert new episodes in a single lock acquisition
    let db = state.db.lock().await;
    let mut new_episode_count = 0;

    for ep in &all_tmdb_episodes {
        let key = (ep.season_number, ep.episode_number);

        if existing_episodes.contains(&key) {
            // Update existing episode metadata
            db.execute(
                r#"
                UPDATE episodes SET
                    title = ?1,
                    overview = ?2,
                    air_date = ?3,
                    runtime_minutes = ?4,
                    still_path = ?5,
                    updated_at = datetime('now')
                WHERE show_id = ?6 AND season_number = ?7 AND episode_number = ?8
                "#,
                rusqlite::params![
                    ep.name,
                    ep.overview,
                    ep.air_date,
                    ep.runtime,
                    ep.still_path,
                    show_id,
                    ep.season_number,
                    ep.episode_number,
                ],
            )?;
        } else {
            // Insert new episode using the show's monitored status
            db.execute(
                r#"
                INSERT INTO episodes (
                    show_id, tmdb_id, season_number, episode_number, title,
                    overview, air_date, runtime_minutes, still_path, status, monitored
                ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, 'missing', ?10)
                "#,
                rusqlite::params![
                    show_id,
                    ep.id,
                    ep.season_number,
                    ep.episode_number,
                    ep.name,
                    ep.overview,
                    ep.air_date,
                    ep.runtime,
                    ep.still_path,
                    show_monitored,
                ],
            )?;
            new_episode_count += 1;
        }
    }

    let show = db.query_row(
        r#"
        SELECT id, tmdb_id, imdb_id, title, original_title, year_start,
               year_end, overview, poster_path, backdrop_path, status,
               monitored, quality_limit, added_at, updated_at, added_by
        FROM tv_shows WHERE id = ?1
        "#,
        [show_id],
        map_show_row,
    )?;

    let mut stmt = db.prepare(
        r#"
        SELECT id, show_id, tmdb_id, season_number, episode_number, title,
               overview, air_date, runtime_minutes, still_path, status,
               monitored, file_path, file_size, created_at, updated_at
        FROM episodes
        WHERE show_id = ?1
        ORDER BY season_number, episode_number
        "#,
    )?;

    let episodes = stmt
        .query_map([show_id], map_episode_row)?
        .collect::<std::result::Result<Vec<_>, _>>()?;

    let seasons = group_episodes_by_season(episodes);

    tracing::info!(
        show_id = show.id,
        title = %show.title,
        new_episodes = new_episode_count,
        "TV show metadata refreshed"
    );

    Ok(Json(ShowWithSeasons { show, seasons }))
}

/// GET /api/tv/:id/season/:s
///
/// Gets all episodes for a specific season.
pub async fn get_season(
    State(state): State<AppState>,
    Path((show_id, season_number)): Path<(i64, i32)>,
) -> Result<Json<SeasonWithEpisodes>> {
    let db = state.db.lock().await;

    // Verify show exists
    let _: i64 = db
        .query_row("SELECT id FROM tv_shows WHERE id = ?1", [show_id], |row| {
            row.get(0)
        })
        .map_err(|e| match e {
            rusqlite::Error::QueryReturnedNoRows => {
                AppError::NotFound("TV show not found".to_string())
            }
            _ => AppError::Sqlite(e),
        })?;

    // Fetch episodes for this season
    let mut stmt = db.prepare(
        r#"
        SELECT id, show_id, tmdb_id, season_number, episode_number, title,
               overview, air_date, runtime_minutes, still_path, status,
               monitored, file_path, file_size, created_at, updated_at
        FROM episodes
        WHERE show_id = ?1 AND season_number = ?2
        ORDER BY episode_number
        "#,
    )?;

    let episodes: Vec<Episode> = stmt
        .query_map([show_id, season_number as i64], map_episode_row)?
        .collect::<std::result::Result<Vec<_>, _>>()?;

    if episodes.is_empty() {
        return Err(AppError::NotFound("Season not found".to_string()));
    }

    let available_count = episodes
        .iter()
        .filter(|e| matches!(e.status, MediaStatus::Available))
        .count() as i32;
    let total_count = episodes.len() as i32;

    Ok(Json(SeasonWithEpisodes {
        season_number,
        episodes,
        available_count,
        total_count,
    }))
}

/// PUT /api/tv/:id/season/:s
///
/// Updates all episodes in a season.
pub async fn update_season(
    State(state): State<AppState>,
    Path((show_id, season_number)): Path<(i64, i32)>,
    Json(body): Json<UpdateSeasonRequest>,
) -> Result<Json<SeasonWithEpisodes>> {
    if body.monitored.is_none() {
        return Err(AppError::BadRequest("No fields to update".to_string()));
    }

    let db = state.db.lock().await;

    // Verify show exists
    let _: i64 = db
        .query_row("SELECT id FROM tv_shows WHERE id = ?1", [show_id], |row| {
            row.get(0)
        })
        .map_err(|e| match e {
            rusqlite::Error::QueryReturnedNoRows => {
                AppError::NotFound("TV show not found".to_string())
            }
            _ => AppError::Sqlite(e),
        })?;

    // Check season exists
    let count: i64 = db.query_row(
        "SELECT COUNT(*) FROM episodes WHERE show_id = ?1 AND season_number = ?2",
        [show_id, season_number as i64],
        |row| row.get(0),
    )?;

    if count == 0 {
        return Err(AppError::NotFound("Season not found".to_string()));
    }

    // Update all episodes in the season
    if let Some(monitored) = body.monitored {
        db.execute(
            r#"
            UPDATE episodes SET monitored = ?1, updated_at = datetime('now')
            WHERE show_id = ?2 AND season_number = ?3
            "#,
            rusqlite::params![monitored, show_id, season_number],
        )?;
    }

    // Fetch updated episodes
    let mut stmt = db.prepare(
        r#"
        SELECT id, show_id, tmdb_id, season_number, episode_number, title,
               overview, air_date, runtime_minutes, still_path, status,
               monitored, file_path, file_size, created_at, updated_at
        FROM episodes
        WHERE show_id = ?1 AND season_number = ?2
        ORDER BY episode_number
        "#,
    )?;

    let episodes: Vec<Episode> = stmt
        .query_map([show_id, season_number as i64], map_episode_row)?
        .collect::<std::result::Result<Vec<_>, _>>()?;

    let available_count = episodes
        .iter()
        .filter(|e| matches!(e.status, MediaStatus::Available))
        .count() as i32;
    let total_count = episodes.len() as i32;

    tracing::info!(
        show_id = show_id,
        season = season_number,
        episodes = count,
        "Season updated"
    );

    Ok(Json(SeasonWithEpisodes {
        season_number,
        episodes,
        available_count,
        total_count,
    }))
}

/// PUT /api/tv/:id/season/:s/episode/:e
///
/// Updates a single episode.
pub async fn update_episode(
    State(state): State<AppState>,
    Path((show_id, season_number, episode_number)): Path<(i64, i32, i32)>,
    Json(body): Json<UpdateEpisodeRequest>,
) -> Result<Json<Episode>> {
    if body.monitored.is_none() {
        return Err(AppError::BadRequest("No fields to update".to_string()));
    }

    let db = state.db.lock().await;

    // Check episode exists
    let episode_id: i64 = db
        .query_row(
            r#"
            SELECT id FROM episodes
            WHERE show_id = ?1 AND season_number = ?2 AND episode_number = ?3
            "#,
            rusqlite::params![show_id, season_number, episode_number],
            |row| row.get(0),
        )
        .map_err(|e| match e {
            rusqlite::Error::QueryReturnedNoRows => {
                AppError::NotFound("Episode not found".to_string())
            }
            _ => AppError::Sqlite(e),
        })?;

    // Update episode
    if let Some(monitored) = body.monitored {
        db.execute(
            r#"
            UPDATE episodes SET monitored = ?1, updated_at = datetime('now')
            WHERE id = ?2
            "#,
            rusqlite::params![monitored, episode_id],
        )?;
    }

    // Fetch updated episode
    let episode = db.query_row(
        r#"
        SELECT id, show_id, tmdb_id, season_number, episode_number, title,
               overview, air_date, runtime_minutes, still_path, status,
               monitored, file_path, file_size, created_at, updated_at
        FROM episodes WHERE id = ?1
        "#,
        [episode_id],
        map_episode_row,
    )?;

    tracing::info!(
        episode_id = episode_id,
        show_id = show_id,
        season = season_number,
        episode = episode_number,
        "Episode updated"
    );

    Ok(Json(episode))
}

/// POST /api/tv/:id/season/:s/episode/:e/search
///
/// Searches indexers for releases of this episode.
pub async fn search_episode(
    State(state): State<AppState>,
    Path((show_id, season_number, episode_number)): Path<(i64, i32, i32)>,
) -> Result<Json<Vec<Release>>> {
    let db = state.db.lock().await;

    // Get show title and verify episode exists
    let (show_title,): (String,) = db
        .query_row(
            "SELECT title FROM tv_shows WHERE id = ?1",
            [show_id],
            |row| Ok((row.get(0)?,)),
        )
        .map_err(|e| match e {
            rusqlite::Error::QueryReturnedNoRows => {
                AppError::NotFound("TV show not found".to_string())
            }
            _ => AppError::Sqlite(e),
        })?;

    // Verify episode exists
    let _: i64 = db
        .query_row(
            r#"
            SELECT id FROM episodes
            WHERE show_id = ?1 AND season_number = ?2 AND episode_number = ?3
            "#,
            rusqlite::params![show_id, season_number, episode_number],
            |row| row.get(0),
        )
        .map_err(|e| match e {
            rusqlite::Error::QueryReturnedNoRows => {
                AppError::NotFound("Episode not found".to_string())
            }
            _ => AppError::Sqlite(e),
        })?;

    drop(db); // Release the lock before async operations

    // Build search query
    let query = IndexerSearchQuery::new(&show_title)
        .media_type(MediaSearchType::TvEpisode)
        .episode(season_number, episode_number);

    // Search indexers
    let indexer_manager = state.indexer_manager();
    let releases = indexer_manager.search(&query).await?;

    tracing::info!(
        show_id = show_id,
        title = %show_title,
        season = season_number,
        episode = episode_number,
        results = releases.len(),
        "Searched releases for episode"
    );

    Ok(Json(releases))
}

/// POST /api/tv/:id/season/:s/episode/:e/download
///
/// Starts downloading a release for this episode.
pub async fn download_episode(
    State(state): State<AppState>,
    Path((show_id, season_number, episode_number)): Path<(i64, i32, i32)>,
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

    // Get episode info
    let (episode_id, title): (i64, Option<String>) = db
        .query_row(
            r#"
            SELECT e.id, e.title FROM episodes e
            JOIN tv_shows s ON e.show_id = s.id
            WHERE e.show_id = ?1 AND e.season_number = ?2 AND e.episode_number = ?3
            "#,
            rusqlite::params![show_id, season_number, episode_number],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .map_err(|e| match e {
            rusqlite::Error::QueryReturnedNoRows => {
                AppError::NotFound("Episode not found".to_string())
            }
            _ => AppError::Sqlite(e),
        })?;

    let download_name =
        title.unwrap_or_else(|| format!("S{:02}E{:02}", season_number, episode_number));

    drop(db); // Release lock before async operation

    // Add magnet to torrent engine
    let media_ref = crate::services::torrent::MediaRef {
        media_type: MediaType::Episode,
        media_id: episode_id,
    };

    let info_hash = torrent_engine.add_magnet(&body.magnet, media_ref).await?;

    // Create download record and update episode status
    let db = state.db.lock().await;

    db.execute(
        r#"
        INSERT INTO downloads (source_type, source_id, name, media_type, media_id, source_uri, status)
        VALUES ('torrent', ?1, ?2, 'episode', ?3, ?4, 'downloading')
        "#,
        rusqlite::params![info_hash, download_name, episode_id, body.magnet],
    )?;

    let download_id = db.last_insert_rowid();

    // Update episode status
    db.execute(
        "UPDATE episodes SET status = 'downloading', updated_at = datetime('now') WHERE id = ?1",
        [episode_id],
    )?;

    tracing::info!(
        episode_id = episode_id,
        info_hash = %info_hash,
        download_id = download_id,
        "Started episode download"
    );

    Ok(Json(DownloadInfo {
        id: download_id,
        info_hash,
        name: download_name,
        status: "downloading".to_string(),
    }))
}

// =============================================================================
// Helpers
// =============================================================================

/// Maps a database row to a TvShow struct.
fn map_show_row(row: &rusqlite::Row) -> rusqlite::Result<TvShow> {
    let status_str: String = row.get(10)?;
    let status = match status_str.as_str() {
        "continuing" => ShowStatus::Continuing,
        "ended" => ShowStatus::Ended,
        "canceled" => ShowStatus::Canceled,
        "upcoming" => ShowStatus::Upcoming,
        _ => ShowStatus::Continuing,
    };

    Ok(TvShow {
        id: row.get(0)?,
        tmdb_id: row.get(1)?,
        imdb_id: row.get(2)?,
        title: row.get(3)?,
        original_title: row.get(4)?,
        year_start: row.get(5)?,
        year_end: row.get(6)?,
        overview: row.get(7)?,
        poster_path: row.get(8)?,
        backdrop_path: row.get(9)?,
        status,
        monitored: row.get(11)?,
        quality_limit: row.get(12)?,
        added_at: row.get(13)?,
        updated_at: row.get(14)?,
        added_by: row.get(15)?,
    })
}

/// Maps a database row to an Episode struct.
fn map_episode_row(row: &rusqlite::Row) -> rusqlite::Result<Episode> {
    let status_str: String = row.get(10)?;
    let status = match status_str.as_str() {
        "missing" => MediaStatus::Missing,
        "searching" => MediaStatus::Searching,
        "downloading" => MediaStatus::Downloading,
        "processing" => MediaStatus::Processing,
        "available" => MediaStatus::Available,
        _ => MediaStatus::Missing,
    };

    Ok(Episode {
        id: row.get(0)?,
        show_id: row.get(1)?,
        tmdb_id: row.get(2)?,
        season_number: row.get(3)?,
        episode_number: row.get(4)?,
        title: row.get(5)?,
        overview: row.get(6)?,
        air_date: row.get(7)?,
        runtime_minutes: row.get(8)?,
        still_path: row.get(9)?,
        status,
        monitored: row.get(11)?,
        file_path: row.get(12)?,
        file_size: row.get(13)?,
        created_at: row.get(14)?,
        updated_at: row.get(15)?,
    })
}

/// Groups episodes by season number.
fn group_episodes_by_season(episodes: Vec<Episode>) -> Vec<SeasonWithEpisodes> {
    let mut seasons: BTreeMap<i32, Vec<Episode>> = BTreeMap::new();

    for ep in episodes {
        seasons.entry(ep.season_number).or_default().push(ep);
    }

    seasons
        .into_iter()
        .map(|(season_number, episodes)| {
            let available_count = episodes
                .iter()
                .filter(|e| matches!(e.status, MediaStatus::Available))
                .count() as i32;
            let total_count = episodes.len() as i32;

            SeasonWithEpisodes {
                season_number,
                episodes,
                available_count,
                total_count,
            }
        })
        .collect()
}

/// Convert TMDB show status to our ShowStatus enum.
fn parse_tmdb_status(status: Option<&str>) -> ShowStatus {
    match status {
        Some("Returning Series") => ShowStatus::Continuing,
        Some("Ended") => ShowStatus::Ended,
        Some("Canceled") => ShowStatus::Canceled,
        Some("In Production") | Some("Planned") => ShowStatus::Upcoming,
        _ => ShowStatus::Continuing,
    }
}
