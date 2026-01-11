//! Downloads API endpoints for managing active downloads.

use axum::{
    extract::{Path, Query, State},
    middleware as axum_mw,
    routing::{get, post},
    Json, Router,
};
use serde::{Deserialize, Serialize};

use crate::db::models::{Download, DownloadStatus, MediaType};
use crate::error::{AppError, Result};
use crate::middleware;
use crate::services::torrent::MediaRef;
use crate::AppState;

// =============================================================================
// Request/Response Types
// =============================================================================

/// Query parameters for listing downloads.
#[derive(Debug, Deserialize)]
pub struct ListDownloadsQuery {
    /// Filter by status (queued, downloading, seeding, processing, completed, failed, paused).
    pub status: Option<String>,
}

/// Query parameters for deleting a download.
#[derive(Debug, Deserialize)]
pub struct DeleteDownloadQuery {
    /// Whether to delete downloaded files (default: false).
    pub delete_files: Option<bool>,
}

/// Success response for operations without specific data.
#[derive(Debug, Serialize)]
pub struct SuccessResponse {
    pub success: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
}

// =============================================================================
// Router
// =============================================================================

/// Creates the downloads router with all endpoints.
pub fn router(state: AppState) -> Router<AppState> {
    Router::new()
        .route("/", get(list_downloads))
        .route("/{id}", get(get_download).delete(delete_download))
        .route("/{id}/pause", post(pause_download))
        .route("/{id}/resume", post(resume_download))
        .route("/{id}/retry", post(retry_download))
        .layer(axum_mw::from_fn_with_state(
            state,
            middleware::auth_middleware,
        ))
}

// =============================================================================
// Handlers
// =============================================================================

/// GET /api/downloads
///
/// Lists all downloads with optional status filtering.
/// Merges database records with real-time stats from the torrent engine.
pub async fn list_downloads(
    State(state): State<AppState>,
    Query(query): Query<ListDownloadsQuery>,
) -> Result<Json<Vec<Download>>> {
    // Validate status filter if provided
    if let Some(ref status) = query.status {
        let valid_statuses = [
            "queued",
            "downloading",
            "seeding",
            "processing",
            "completed",
            "failed",
            "paused",
        ];
        if !valid_statuses.contains(&status.as_str()) {
            return Err(AppError::BadRequest(format!("Invalid status: {}", status)));
        }
    }

    let mut downloads: Vec<Download> = {
        let db = state.db.lock().await;

        // Query downloads from database
        let mut stmt = db.prepare(
            r#"
            SELECT id, info_hash, name, media_type, media_id, magnet, status,
                   progress, download_speed, upload_speed, size_bytes,
                   downloaded_bytes, uploaded_bytes, ratio, peers, error_message,
                   added_at, started_at, completed_at
            FROM downloads
            WHERE (?1 IS NULL OR status = ?1)
            ORDER BY added_at DESC
            "#,
        )?;

        let downloads = stmt
            .query_map(rusqlite::params![query.status], map_download_row)?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        downloads
    };

    // Merge with real-time stats from torrent engine if available
    if let Some(torrent_engine) = state.torrent_engine() {
        let live_stats = torrent_engine.list_all().await;

        for download in &mut downloads {
            if let Some(stats) = live_stats
                .iter()
                .find(|s| s.info_hash == download.info_hash)
            {
                download.progress = stats.progress;
                download.download_speed = stats.download_speed as i64;
                download.upload_speed = stats.upload_speed as i64;
                download.downloaded_bytes = stats.downloaded as i64;
                download.uploaded_bytes = stats.uploaded as i64;
                download.ratio = stats.ratio;
                download.peers = stats.peers as i32;
                download.size_bytes = Some(stats.size as i64);
            }
        }
    }

    Ok(Json(downloads))
}

/// GET /api/downloads/:id
///
/// Gets a single download by ID with real-time stats.
pub async fn get_download(
    State(state): State<AppState>,
    Path(download_id): Path<i64>,
) -> Result<Json<Download>> {
    let db = state.db.lock().await;

    let mut download = db
        .query_row(
            r#"
            SELECT id, info_hash, name, media_type, media_id, magnet, status,
                   progress, download_speed, upload_speed, size_bytes,
                   downloaded_bytes, uploaded_bytes, ratio, peers, error_message,
                   added_at, started_at, completed_at
            FROM downloads WHERE id = ?1
            "#,
            [download_id],
            map_download_row,
        )
        .map_err(|e| match e {
            rusqlite::Error::QueryReturnedNoRows => {
                AppError::NotFound("Download not found".to_string())
            }
            _ => AppError::Sqlite(e),
        })?;

    drop(db); // Release lock before accessing torrent engine

    // Merge with real-time stats from torrent engine if available
    if let Some(torrent_engine) = state.torrent_engine() {
        if let Ok(stats) = torrent_engine.get_status(&download.info_hash).await {
            download.progress = stats.progress;
            download.download_speed = stats.download_speed as i64;
            download.upload_speed = stats.upload_speed as i64;
            download.downloaded_bytes = stats.downloaded as i64;
            download.uploaded_bytes = stats.uploaded as i64;
            download.ratio = stats.ratio;
            download.peers = stats.peers as i32;
            download.size_bytes = Some(stats.size as i64);
        }
    }

    Ok(Json(download))
}

/// DELETE /api/downloads/:id
///
/// Removes a download from the torrent engine and database.
pub async fn delete_download(
    State(state): State<AppState>,
    Path(download_id): Path<i64>,
    Query(query): Query<DeleteDownloadQuery>,
) -> Result<Json<SuccessResponse>> {
    let db = state.db.lock().await;

    // Get download info
    let (info_hash, media_type_str, media_id): (String, String, i64) = db
        .query_row(
            "SELECT info_hash, media_type, media_id FROM downloads WHERE id = ?1",
            [download_id],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .map_err(|e| match e {
            rusqlite::Error::QueryReturnedNoRows => {
                AppError::NotFound("Download not found".to_string())
            }
            _ => AppError::Sqlite(e),
        })?;

    // Parse and validate media type
    let media_type = match media_type_str.as_str() {
        "movie" => MediaType::Movie,
        "episode" => MediaType::Episode,
        "album" => MediaType::Album,
        "track" => MediaType::Track,
        _ => {
            return Err(AppError::Internal(format!(
                "Invalid media type in database: {}",
                media_type_str
            )))
        }
    };

    drop(db); // Release lock before async operations

    // Remove from torrent engine
    let delete_files = query.delete_files.unwrap_or(false);
    if let Some(torrent_engine) = state.torrent_engine() {
        if let Err(e) = torrent_engine.remove(&info_hash, delete_files).await {
            tracing::debug!(
                info_hash = %info_hash,
                error = %e,
                "Failed to remove torrent from engine (may have already been removed)"
            );
        }
    }

    // Delete from database and revert media status
    let db = state.db.lock().await;

    db.execute("DELETE FROM downloads WHERE id = ?1", [download_id])?;

    // Revert media status to 'missing'
    let update_query = match media_type {
        MediaType::Movie => {
            "UPDATE movies SET status = 'missing', updated_at = datetime('now') WHERE id = ?1"
        }
        MediaType::Episode => {
            "UPDATE episodes SET status = 'missing', updated_at = datetime('now') WHERE id = ?1"
        }
        MediaType::Album => {
            "UPDATE albums SET status = 'missing', updated_at = datetime('now') WHERE id = ?1"
        }
        MediaType::Track => {
            "UPDATE tracks SET status = 'missing', updated_at = datetime('now') WHERE id = ?1"
        }
    };

    db.execute(update_query, [media_id])?;

    tracing::info!(
        download_id = download_id,
        info_hash = %info_hash,
        delete_files = delete_files,
        "Download deleted"
    );

    Ok(Json(SuccessResponse {
        success: true,
        message: Some("Download deleted successfully".to_string()),
    }))
}

/// POST /api/downloads/:id/pause
///
/// Pauses a download.
pub async fn pause_download(
    State(state): State<AppState>,
    Path(download_id): Path<i64>,
) -> Result<Json<Download>> {
    let db = state.db.lock().await;

    // Get download info
    let (info_hash,): (String,) = db
        .query_row(
            "SELECT info_hash FROM downloads WHERE id = ?1",
            [download_id],
            |row| Ok((row.get(0)?,)),
        )
        .map_err(|e| match e {
            rusqlite::Error::QueryReturnedNoRows => {
                AppError::NotFound("Download not found".to_string())
            }
            _ => AppError::Sqlite(e),
        })?;

    drop(db); // Release lock before async operations

    // Pause in torrent engine
    let torrent_engine = state
        .torrent_engine()
        .ok_or_else(|| AppError::Internal("Torrent engine not available".to_string()))?;

    torrent_engine.pause(&info_hash).await?;

    // Update database status
    let db = state.db.lock().await;
    db.execute(
        "UPDATE downloads SET status = 'paused' WHERE id = ?1",
        [download_id],
    )?;

    // Fetch updated download
    let download = db.query_row(
        r#"
        SELECT id, info_hash, name, media_type, media_id, magnet, status,
               progress, download_speed, upload_speed, size_bytes,
               downloaded_bytes, uploaded_bytes, ratio, peers, error_message,
               added_at, started_at, completed_at
        FROM downloads WHERE id = ?1
        "#,
        [download_id],
        map_download_row,
    )?;

    tracing::info!(
        download_id = download_id,
        info_hash = %info_hash,
        "Download paused"
    );

    Ok(Json(download))
}

/// POST /api/downloads/:id/resume
///
/// Resumes a paused download.
pub async fn resume_download(
    State(state): State<AppState>,
    Path(download_id): Path<i64>,
) -> Result<Json<Download>> {
    let db = state.db.lock().await;

    // Get download info
    let (info_hash,): (String,) = db
        .query_row(
            "SELECT info_hash FROM downloads WHERE id = ?1",
            [download_id],
            |row| Ok((row.get(0)?,)),
        )
        .map_err(|e| match e {
            rusqlite::Error::QueryReturnedNoRows => {
                AppError::NotFound("Download not found".to_string())
            }
            _ => AppError::Sqlite(e),
        })?;

    drop(db); // Release lock before async operations

    // Resume in torrent engine
    let torrent_engine = state
        .torrent_engine()
        .ok_or_else(|| AppError::Internal("Torrent engine not available".to_string()))?;

    torrent_engine.resume(&info_hash).await?;

    // Update database status
    let db = state.db.lock().await;
    db.execute(
        "UPDATE downloads SET status = 'downloading' WHERE id = ?1",
        [download_id],
    )?;

    // Fetch updated download
    let download = db.query_row(
        r#"
        SELECT id, info_hash, name, media_type, media_id, magnet, status,
               progress, download_speed, upload_speed, size_bytes,
               downloaded_bytes, uploaded_bytes, ratio, peers, error_message,
               added_at, started_at, completed_at
        FROM downloads WHERE id = ?1
        "#,
        [download_id],
        map_download_row,
    )?;

    tracing::info!(
        download_id = download_id,
        info_hash = %info_hash,
        "Download resumed"
    );

    Ok(Json(download))
}

/// POST /api/downloads/:id/retry
///
/// Retries a failed download by re-adding the magnet link.
pub async fn retry_download(
    State(state): State<AppState>,
    Path(download_id): Path<i64>,
) -> Result<Json<Download>> {
    let db = state.db.lock().await;

    // Get download info
    let (info_hash, magnet, media_type_str, media_id): (String, String, String, i64) = db
        .query_row(
            "SELECT info_hash, magnet, media_type, media_id FROM downloads WHERE id = ?1",
            [download_id],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
        )
        .map_err(|e| match e {
            rusqlite::Error::QueryReturnedNoRows => {
                AppError::NotFound("Download not found".to_string())
            }
            _ => AppError::Sqlite(e),
        })?;

    drop(db); // Release lock before async operations

    // Get torrent engine
    let torrent_engine = state
        .torrent_engine()
        .ok_or_else(|| AppError::Internal("Torrent engine not available".to_string()))?;

    // Remove the failed torrent if it exists
    let _ = torrent_engine.remove(&info_hash, false).await;

    // Parse media type
    let media_type = match media_type_str.as_str() {
        "movie" => MediaType::Movie,
        "episode" => MediaType::Episode,
        "album" => MediaType::Album,
        "track" => MediaType::Track,
        _ => return Err(AppError::Internal("Invalid media type".to_string())),
    };

    // Re-add the magnet
    let media_ref = MediaRef {
        media_type,
        media_id,
    };

    let new_info_hash = torrent_engine.add_magnet(&magnet, media_ref).await?;

    // Update database
    let db = state.db.lock().await;

    db.execute(
        r#"
        UPDATE downloads
        SET info_hash = ?1, status = 'downloading', error_message = NULL,
            progress = 0, download_speed = 0, upload_speed = 0,
            downloaded_bytes = 0, uploaded_bytes = 0, ratio = 0, peers = 0,
            started_at = datetime('now')
        WHERE id = ?2
        "#,
        rusqlite::params![new_info_hash, download_id],
    )?;

    // Fetch updated download
    let download = db.query_row(
        r#"
        SELECT id, info_hash, name, media_type, media_id, magnet, status,
               progress, download_speed, upload_speed, size_bytes,
               downloaded_bytes, uploaded_bytes, ratio, peers, error_message,
               added_at, started_at, completed_at
        FROM downloads WHERE id = ?1
        "#,
        [download_id],
        map_download_row,
    )?;

    tracing::info!(
        download_id = download_id,
        old_info_hash = %info_hash,
        new_info_hash = %new_info_hash,
        "Download retried"
    );

    Ok(Json(download))
}

// =============================================================================
// Helpers
// =============================================================================

/// Maps a database row to a Download struct.
fn map_download_row(row: &rusqlite::Row) -> rusqlite::Result<Download> {
    let status_str: String = row.get(6)?;
    let status = match status_str.as_str() {
        "queued" => DownloadStatus::Queued,
        "downloading" => DownloadStatus::Downloading,
        "seeding" => DownloadStatus::Seeding,
        "processing" => DownloadStatus::Processing,
        "completed" => DownloadStatus::Completed,
        "failed" => DownloadStatus::Failed,
        "paused" => DownloadStatus::Paused,
        _ => DownloadStatus::Queued,
    };

    let media_type_str: String = row.get(3)?;
    let media_type = match media_type_str.as_str() {
        "movie" => MediaType::Movie,
        "episode" => MediaType::Episode,
        "album" => MediaType::Album,
        "track" => MediaType::Track,
        _ => MediaType::Movie,
    };

    Ok(Download {
        id: row.get(0)?,
        info_hash: row.get(1)?,
        name: row.get(2)?,
        media_type,
        media_id: row.get(4)?,
        magnet: row.get(5)?,
        status,
        progress: row.get(7)?,
        download_speed: row.get(8)?,
        upload_speed: row.get(9)?,
        size_bytes: row.get(10)?,
        downloaded_bytes: row.get(11)?,
        uploaded_bytes: row.get(12)?,
        ratio: row.get(13)?,
        peers: row.get(14)?,
        error_message: row.get(15)?,
        added_at: row.get(16)?,
        started_at: row.get(17)?,
        completed_at: row.get(18)?,
    })
}
