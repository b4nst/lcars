//! Downloads API endpoints for managing active downloads.

use axum::{
    extract::{Path, Query, State},
    middleware as axum_mw,
    routing::{get, post},
    Json, Router,
};
use serde::{Deserialize, Serialize};

use crate::db::models::{Download, DownloadSource, DownloadStatus, MediaType};
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
    /// Filter by source type (torrent, soulseek).
    pub source: Option<String>,
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
/// Lists all downloads with optional status and source filtering.
/// Merges database records with real-time stats from the appropriate engine.
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

    // Validate source filter if provided
    if let Some(ref source) = query.source {
        let valid_sources = ["torrent", "soulseek"];
        if !valid_sources.contains(&source.as_str()) {
            return Err(AppError::BadRequest(format!("Invalid source: {}", source)));
        }
    }

    let mut downloads: Vec<Download> = {
        let db = state.db.lock().await;

        // Query downloads from database
        let mut stmt = db.prepare(
            r#"
            SELECT id, source_type, source_id, name, media_type, media_id, source_uri, status,
                   progress, download_speed, upload_speed, size_bytes,
                   downloaded_bytes, uploaded_bytes, ratio, peers, error_message,
                   added_at, started_at, completed_at,
                   soulseek_username, soulseek_filename, queue_position
            FROM downloads
            WHERE (?1 IS NULL OR status = ?1)
              AND (?2 IS NULL OR source_type = ?2)
            ORDER BY added_at DESC
            "#,
        )?;

        let downloads = stmt
            .query_map(rusqlite::params![query.status, query.source], map_download_row)?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        downloads
    };

    // Merge with real-time stats from torrent engine if available
    if let Some(torrent_engine) = state.torrent_engine() {
        let live_stats = torrent_engine.list_all().await;

        for download in &mut downloads {
            // Only merge torrent stats for torrent downloads
            if download.source_type != DownloadSource::Torrent {
                continue;
            }
            if let Some(stats) = live_stats
                .iter()
                .find(|s| s.info_hash == download.source_id)
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

    // TODO: Merge with Soulseek stats when SoulseekEngine has download tracking

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
            SELECT id, source_type, source_id, name, media_type, media_id, source_uri, status,
                   progress, download_speed, upload_speed, size_bytes,
                   downloaded_bytes, uploaded_bytes, ratio, peers, error_message,
                   added_at, started_at, completed_at,
                   soulseek_username, soulseek_filename, queue_position
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

    drop(db); // Release lock before accessing engines

    // Merge with real-time stats from the appropriate engine
    match download.source_type {
        DownloadSource::Torrent => {
            if let Some(torrent_engine) = state.torrent_engine() {
                if let Ok(stats) = torrent_engine.get_status(&download.source_id).await {
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
        DownloadSource::Soulseek => {
            // TODO: Merge with Soulseek stats when SoulseekEngine has download tracking
        }
    }

    Ok(Json(download))
}

/// DELETE /api/downloads/:id
///
/// Removes a download from the appropriate engine and database.
pub async fn delete_download(
    State(state): State<AppState>,
    Path(download_id): Path<i64>,
    Query(query): Query<DeleteDownloadQuery>,
) -> Result<Json<SuccessResponse>> {
    let db = state.db.lock().await;

    // Get download info
    let (source_type_str, source_id, media_type_str, media_id): (String, String, String, i64) = db
        .query_row(
            "SELECT source_type, source_id, media_type, media_id FROM downloads WHERE id = ?1",
            [download_id],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
        )
        .map_err(|e| match e {
            rusqlite::Error::QueryReturnedNoRows => {
                AppError::NotFound("Download not found".to_string())
            }
            _ => AppError::Sqlite(e),
        })?;

    // Parse source type
    let source_type: DownloadSource = source_type_str
        .parse()
        .map_err(|e: String| AppError::Internal(e))?;

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

    // Remove from appropriate engine
    let delete_files = query.delete_files.unwrap_or(false);
    match source_type {
        DownloadSource::Torrent => {
            if let Some(torrent_engine) = state.torrent_engine() {
                if let Err(e) = torrent_engine.remove(&source_id, delete_files).await {
                    tracing::debug!(
                        source_id = %source_id,
                        error = %e,
                        "Failed to remove torrent from engine (may have already been removed)"
                    );
                }
            }
        }
        DownloadSource::Soulseek => {
            // TODO: Cancel Soulseek download when SoulseekEngine has download tracking
            tracing::debug!(
                source_id = %source_id,
                "Soulseek download removal not yet implemented"
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
        source_type = %source_type,
        source_id = %source_id,
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
/// Pauses a download. Only supported for torrent downloads.
pub async fn pause_download(
    State(state): State<AppState>,
    Path(download_id): Path<i64>,
) -> Result<Json<Download>> {
    let db = state.db.lock().await;

    // Get download info
    let (source_type_str, source_id): (String, String) = db
        .query_row(
            "SELECT source_type, source_id FROM downloads WHERE id = ?1",
            [download_id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .map_err(|e| match e {
            rusqlite::Error::QueryReturnedNoRows => {
                AppError::NotFound("Download not found".to_string())
            }
            _ => AppError::Sqlite(e),
        })?;

    // Parse source type
    let source_type: DownloadSource = source_type_str
        .parse()
        .map_err(|e: String| AppError::Internal(e))?;

    drop(db); // Release lock before async operations

    // Pause in appropriate engine
    match source_type {
        DownloadSource::Torrent => {
            let torrent_engine = state
                .torrent_engine()
                .ok_or_else(|| AppError::Internal("Torrent engine not available".to_string()))?;

            torrent_engine.pause(&source_id).await?;
        }
        DownloadSource::Soulseek => {
            return Err(AppError::BadRequest(
                "Pause is not supported for Soulseek downloads".to_string(),
            ));
        }
    }

    // Update database status
    let db = state.db.lock().await;
    db.execute(
        "UPDATE downloads SET status = 'paused' WHERE id = ?1",
        [download_id],
    )?;

    // Fetch updated download
    let download = db.query_row(
        r#"
        SELECT id, source_type, source_id, name, media_type, media_id, source_uri, status,
               progress, download_speed, upload_speed, size_bytes,
               downloaded_bytes, uploaded_bytes, ratio, peers, error_message,
               added_at, started_at, completed_at,
               soulseek_username, soulseek_filename, queue_position
        FROM downloads WHERE id = ?1
        "#,
        [download_id],
        map_download_row,
    )?;

    tracing::info!(
        download_id = download_id,
        source_id = %source_id,
        "Download paused"
    );

    Ok(Json(download))
}

/// POST /api/downloads/:id/resume
///
/// Resumes a paused download. Only supported for torrent downloads.
pub async fn resume_download(
    State(state): State<AppState>,
    Path(download_id): Path<i64>,
) -> Result<Json<Download>> {
    let db = state.db.lock().await;

    // Get download info
    let (source_type_str, source_id): (String, String) = db
        .query_row(
            "SELECT source_type, source_id FROM downloads WHERE id = ?1",
            [download_id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .map_err(|e| match e {
            rusqlite::Error::QueryReturnedNoRows => {
                AppError::NotFound("Download not found".to_string())
            }
            _ => AppError::Sqlite(e),
        })?;

    // Parse source type
    let source_type: DownloadSource = source_type_str
        .parse()
        .map_err(|e: String| AppError::Internal(e))?;

    drop(db); // Release lock before async operations

    // Resume in appropriate engine
    match source_type {
        DownloadSource::Torrent => {
            let torrent_engine = state
                .torrent_engine()
                .ok_or_else(|| AppError::Internal("Torrent engine not available".to_string()))?;

            torrent_engine.resume(&source_id).await?;
        }
        DownloadSource::Soulseek => {
            return Err(AppError::BadRequest(
                "Resume is not supported for Soulseek downloads".to_string(),
            ));
        }
    }

    // Update database status
    let db = state.db.lock().await;
    db.execute(
        "UPDATE downloads SET status = 'downloading' WHERE id = ?1",
        [download_id],
    )?;

    // Fetch updated download
    let download = db.query_row(
        r#"
        SELECT id, source_type, source_id, name, media_type, media_id, source_uri, status,
               progress, download_speed, upload_speed, size_bytes,
               downloaded_bytes, uploaded_bytes, ratio, peers, error_message,
               added_at, started_at, completed_at,
               soulseek_username, soulseek_filename, queue_position
        FROM downloads WHERE id = ?1
        "#,
        [download_id],
        map_download_row,
    )?;

    tracing::info!(
        download_id = download_id,
        source_id = %source_id,
        "Download resumed"
    );

    Ok(Json(download))
}

/// POST /api/downloads/:id/retry
///
/// Retries a failed download by re-adding it to the appropriate engine.
pub async fn retry_download(
    State(state): State<AppState>,
    Path(download_id): Path<i64>,
) -> Result<Json<Download>> {
    let db = state.db.lock().await;

    // Get download info
    let (source_type_str, source_id, source_uri, media_type_str, media_id): (String, String, String, String, i64) = db
        .query_row(
            "SELECT source_type, source_id, source_uri, media_type, media_id FROM downloads WHERE id = ?1",
            [download_id],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?, row.get(4)?)),
        )
        .map_err(|e| match e {
            rusqlite::Error::QueryReturnedNoRows => {
                AppError::NotFound("Download not found".to_string())
            }
            _ => AppError::Sqlite(e),
        })?;

    // Parse source type
    let source_type: DownloadSource = source_type_str
        .parse()
        .map_err(|e: String| AppError::Internal(e))?;

    drop(db); // Release lock before async operations

    // Parse media type
    let media_type = match media_type_str.as_str() {
        "movie" => MediaType::Movie,
        "episode" => MediaType::Episode,
        "album" => MediaType::Album,
        "track" => MediaType::Track,
        _ => return Err(AppError::Internal("Invalid media type".to_string())),
    };

    // Retry in appropriate engine
    let new_source_id = match source_type {
        DownloadSource::Torrent => {
            let torrent_engine = state
                .torrent_engine()
                .ok_or_else(|| AppError::Internal("Torrent engine not available".to_string()))?;

            // Remove the failed torrent if it exists
            let _ = torrent_engine.remove(&source_id, false).await;

            // Re-add the magnet
            let media_ref = MediaRef {
                media_type,
                media_id,
            };

            torrent_engine.add_magnet(&source_uri, media_ref).await?
        }
        DownloadSource::Soulseek => {
            return Err(AppError::BadRequest(
                "Retry is not yet supported for Soulseek downloads".to_string(),
            ));
        }
    };

    // Update database
    let db = state.db.lock().await;

    db.execute(
        r#"
        UPDATE downloads
        SET source_id = ?1, status = 'downloading', error_message = NULL,
            progress = 0, download_speed = 0, upload_speed = 0,
            downloaded_bytes = 0, uploaded_bytes = 0, ratio = 0, peers = 0,
            started_at = datetime('now')
        WHERE id = ?2
        "#,
        rusqlite::params![new_source_id, download_id],
    )?;

    // Fetch updated download
    let download = db.query_row(
        r#"
        SELECT id, source_type, source_id, name, media_type, media_id, source_uri, status,
               progress, download_speed, upload_speed, size_bytes,
               downloaded_bytes, uploaded_bytes, ratio, peers, error_message,
               added_at, started_at, completed_at,
               soulseek_username, soulseek_filename, queue_position
        FROM downloads WHERE id = ?1
        "#,
        [download_id],
        map_download_row,
    )?;

    tracing::info!(
        download_id = download_id,
        old_source_id = %source_id,
        new_source_id = %new_source_id,
        "Download retried"
    );

    Ok(Json(download))
}

// =============================================================================
// Helpers
// =============================================================================

/// Maps a database row to a Download struct.
/// Column order:
///   0: id, 1: source_type, 2: source_id, 3: name, 4: media_type, 5: media_id,
///   6: source_uri, 7: status, 8: progress, 9: download_speed, 10: upload_speed,
///   11: size_bytes, 12: downloaded_bytes, 13: uploaded_bytes, 14: ratio, 15: peers,
///   16: error_message, 17: added_at, 18: started_at, 19: completed_at,
///   20: soulseek_username, 21: soulseek_filename, 22: queue_position
fn map_download_row(row: &rusqlite::Row) -> rusqlite::Result<Download> {
    let source_type_str: String = row.get(1)?;
    let source_type = match source_type_str.as_str() {
        "soulseek" => DownloadSource::Soulseek,
        _ => DownloadSource::Torrent,
    };

    let status_str: String = row.get(7)?;
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

    let media_type_str: String = row.get(4)?;
    let media_type = match media_type_str.as_str() {
        "movie" => MediaType::Movie,
        "episode" => MediaType::Episode,
        "album" => MediaType::Album,
        "track" => MediaType::Track,
        _ => MediaType::Movie,
    };

    Ok(Download {
        id: row.get(0)?,
        source_type,
        source_id: row.get(2)?,
        source_uri: row.get(6)?,
        name: row.get(3)?,
        media_type,
        media_id: row.get(5)?,
        status,
        progress: row.get(8)?,
        download_speed: row.get(9)?,
        upload_speed: row.get(10)?,
        size_bytes: row.get(11)?,
        downloaded_bytes: row.get(12)?,
        uploaded_bytes: row.get(13)?,
        ratio: row.get(14)?,
        peers: row.get(15)?,
        error_message: row.get(16)?,
        added_at: row.get(17)?,
        started_at: row.get(18)?,
        completed_at: row.get(19)?,
        soulseek_username: row.get(20)?,
        soulseek_filename: row.get(21)?,
        queue_position: row.get(22)?,
    })
}
