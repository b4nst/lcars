//! Soulseek API endpoints for search, download, and status.

use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    middleware as axum_mw,
    routing::{get, post},
    Json, Router,
};
use serde::{Deserialize, Serialize};

use crate::error::{AppError, Result};
use crate::middleware;
use crate::services::soulseek::{
    DownloadRequest as EngineDownloadRequest, DownloadState, SoulseekStats,
};
use crate::AppState;

// =============================================================================
// Request/Response Types
// =============================================================================

/// Request body for starting a search.
#[derive(Debug, Deserialize)]
pub struct SearchRequest {
    /// The search query string.
    pub query: String,
}

/// Response for starting a search.
#[derive(Debug, Serialize)]
pub struct SearchStartResponse {
    /// The search ticket ID.
    pub ticket: u32,
    /// The search query.
    pub query: String,
    /// Current status of the search.
    pub status: String,
}

/// Query parameters for getting search results.
#[derive(Debug, Deserialize)]
pub struct SearchResultsQuery {
    /// Minimum bitrate filter in kbps.
    pub min_bitrate: Option<u32>,
    /// Comma-separated list of file extensions to include (e.g., "flac,mp3").
    pub extensions: Option<String>,
    /// Only show results from users with free upload slots.
    pub free_slots_only: Option<bool>,
    /// Sort by: "bitrate", "size", "speed", "queue".
    pub sort_by: Option<String>,
    /// Maximum number of results to return.
    pub limit: Option<usize>,
    /// Number of results to skip.
    pub offset: Option<usize>,
}

/// A single file result for API responses.
#[derive(Debug, Serialize)]
pub struct FileResultResponse {
    /// Username of the peer sharing the file.
    pub username: String,
    /// Full path of the file on the peer's system.
    pub filename: String,
    /// File size in bytes.
    pub size: u64,
    /// Bitrate in kbps (if available).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bitrate: Option<u32>,
    /// Duration in seconds (if available).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub duration: Option<u32>,
    /// File extension.
    pub extension: String,
    /// Whether the peer has free download slots.
    pub slots_free: bool,
    /// Average upload speed in bytes/second.
    pub upload_speed: u32,
    /// Number of files in the peer's queue.
    pub queue_length: u32,
}

/// Response for search results.
#[derive(Debug, Serialize)]
pub struct SearchResultsResponse {
    /// The search ticket ID.
    pub ticket: u32,
    /// The search query.
    pub query: String,
    /// Status: "searching", "complete", or "cancelled".
    pub status: String,
    /// Total number of results (before filtering/pagination).
    pub result_count: usize,
    /// The search results.
    pub results: Vec<FileResultResponse>,
}

/// Status response for the Soulseek engine.
#[derive(Debug, Serialize)]
pub struct StatusResponse {
    /// Whether connected to the Soulseek server.
    pub connected: bool,
    /// Number of active searches.
    pub active_searches: usize,
    /// Number of active downloads.
    pub active_downloads: usize,
    /// Number of completed downloads.
    pub completed_downloads: usize,
}

// =============================================================================
// Download Request/Response Types
// =============================================================================

/// Request body for starting a download.
#[derive(Debug, Deserialize)]
pub struct DownloadRequest {
    /// Username of the peer sharing the file.
    pub username: String,
    /// Full path of the file on the peer's system.
    pub filename: String,
    /// File size in bytes.
    pub size: u64,
    /// Optional media type (track, album, episode).
    pub media_type: Option<String>,
    /// Optional media ID to link to our library.
    pub media_id: Option<i64>,
}

/// Response for a download operation.
#[derive(Debug, Serialize)]
pub struct DownloadResponse {
    /// Unique download ID.
    pub id: String,
    /// Source type (always "soulseek").
    pub source_type: String,
    /// Username of the peer.
    pub username: String,
    /// Filename being downloaded.
    pub filename: String,
    /// Download status.
    pub status: String,
    /// File size in bytes.
    pub size: u64,
    /// Bytes downloaded.
    pub downloaded: u64,
    /// Download speed in bytes/second.
    pub speed: u64,
    /// Position in the remote user's queue (if queued).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub queue_position: Option<u32>,
    /// Progress percentage (0-100).
    pub progress_percent: u8,
}

impl From<&DownloadState> for DownloadResponse {
    fn from(state: &DownloadState) -> Self {
        Self {
            id: state.id.clone(),
            source_type: "soulseek".to_string(),
            username: state.username.clone(),
            filename: state.filename.clone(),
            status: state.status.to_string(),
            size: state.size,
            downloaded: state.downloaded,
            speed: state.speed,
            queue_position: state.queue_position,
            progress_percent: state.progress_percent(),
        }
    }
}

/// Response for browse operation.
#[derive(Debug, Serialize)]
pub struct BrowseResponse {
    /// Username of the peer.
    pub username: String,
    /// List of directories with files.
    pub directories: Vec<BrowseDirectoryResponse>,
    /// Total number of directories.
    pub total_directories: usize,
    /// Total number of files.
    pub total_files: usize,
}

/// A directory from browse results.
#[derive(Debug, Serialize)]
pub struct BrowseDirectoryResponse {
    /// Directory path.
    pub path: String,
    /// Number of files in this directory.
    pub file_count: usize,
    /// Files in this directory.
    pub files: Vec<BrowseFileResponse>,
}

/// A file from browse results.
#[derive(Debug, Serialize)]
pub struct BrowseFileResponse {
    /// Filename (without full path).
    pub name: String,
    /// Full path on the remote system.
    pub full_path: String,
    /// File size in bytes.
    pub size: u64,
    /// File extension.
    pub extension: String,
    /// Bitrate (if available).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bitrate: Option<u32>,
    /// Duration in seconds (if available).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub duration: Option<u32>,
}

// =============================================================================
// Router
// =============================================================================

/// Creates the Soulseek router with all endpoints.
pub fn router(state: AppState) -> Router<AppState> {
    Router::new()
        .route("/search", post(start_search))
        .route(
            "/search/{ticket}",
            get(get_search_results).delete(cancel_search),
        )
        .route("/download", post(start_download))
        .route("/downloads", get(list_downloads))
        .route("/downloads/{id}", get(get_download).delete(cancel_download))
        .route("/browse/{username}", get(browse_user))
        .route("/status", get(get_status))
        .layer(axum_mw::from_fn_with_state(
            state,
            middleware::auth_middleware,
        ))
}

// =============================================================================
// Handlers
// =============================================================================

/// POST /api/soulseek/search
///
/// Start a new Soulseek search.
/// Returns 202 Accepted with the search ticket.
pub async fn start_search(
    State(state): State<AppState>,
    Json(request): Json<SearchRequest>,
) -> Result<(StatusCode, Json<SearchStartResponse>)> {
    // Validate query
    let query = request.query.trim();
    if query.is_empty() {
        return Err(AppError::BadRequest(
            "Search query cannot be empty".to_string(),
        ));
    }

    // Get Soulseek engine
    let engine = state
        .soulseek_engine()
        .ok_or_else(|| AppError::ServiceUnavailable("Soulseek is not configured".to_string()))?;

    // Check if connected
    if !engine.is_connected() {
        return Err(AppError::ServiceUnavailable(
            "Soulseek is not connected".to_string(),
        ));
    }

    // Start the search
    let ticket = engine.search(query).await?;

    tracing::info!(
        ticket = ticket,
        query = %query,
        "Started Soulseek search"
    );

    Ok((
        StatusCode::ACCEPTED,
        Json(SearchStartResponse {
            ticket,
            query: query.to_string(),
            status: "searching".to_string(),
        }),
    ))
}

/// GET /api/soulseek/search/{ticket}
///
/// Get results for an active or completed search.
pub async fn get_search_results(
    State(state): State<AppState>,
    Path(ticket): Path<u32>,
    Query(query): Query<SearchResultsQuery>,
) -> Result<Json<SearchResultsResponse>> {
    // Get Soulseek engine
    let engine = state
        .soulseek_engine()
        .ok_or_else(|| AppError::ServiceUnavailable("Soulseek is not configured".to_string()))?;

    // Get search state
    let search_state = engine
        .get_search_results(ticket)
        .await
        .ok_or_else(|| AppError::NotFound(format!("Search with ticket {} not found", ticket)))?;

    // Flatten results from all peers into a single list
    let mut all_files: Vec<FileResultResponse> = search_state
        .results
        .iter()
        .flat_map(|result| {
            result.files.iter().map(|file| FileResultResponse {
                username: result.username.clone(),
                filename: file.filename.clone(),
                size: file.size,
                bitrate: file.bitrate,
                duration: file.duration,
                extension: file.extension.clone(),
                slots_free: result.has_free_slot,
                upload_speed: result.average_speed,
                queue_length: result.queue_length,
            })
        })
        .collect();

    let total_count = all_files.len();

    // Apply filters
    if let Some(min_bitrate) = query.min_bitrate {
        all_files.retain(|f| f.bitrate.is_some_and(|b| b >= min_bitrate));
    }

    if let Some(ref extensions) = query.extensions {
        let allowed_exts: Vec<String> = extensions
            .split(',')
            .map(|s| s.trim().to_lowercase())
            .collect();
        all_files.retain(|f| {
            allowed_exts
                .iter()
                .any(|ext| f.extension.to_lowercase() == *ext)
        });
    }

    if query.free_slots_only.unwrap_or(false) {
        all_files.retain(|f| f.slots_free);
    }

    // Apply sorting
    if let Some(ref sort_by) = query.sort_by {
        match sort_by.as_str() {
            "bitrate" => {
                all_files.sort_by(|a, b| b.bitrate.cmp(&a.bitrate));
            }
            "size" => {
                all_files.sort_by(|a, b| b.size.cmp(&a.size));
            }
            "speed" => {
                all_files.sort_by(|a, b| b.upload_speed.cmp(&a.upload_speed));
            }
            "queue" => {
                all_files.sort_by(|a, b| a.queue_length.cmp(&b.queue_length));
            }
            _ => {
                // Invalid sort parameter, ignore
            }
        }
    }

    // Apply pagination
    let offset = query.offset.unwrap_or(0);
    let limit = query.limit.unwrap_or(100).min(1000); // Max 1000 results per request

    let results: Vec<FileResultResponse> = all_files.into_iter().skip(offset).take(limit).collect();

    let status = if search_state.complete {
        "complete"
    } else {
        "searching"
    };

    Ok(Json(SearchResultsResponse {
        ticket,
        query: search_state.query,
        status: status.to_string(),
        result_count: total_count,
        results,
    }))
}

/// DELETE /api/soulseek/search/{ticket}
///
/// Cancel an active search.
pub async fn cancel_search(
    State(state): State<AppState>,
    Path(ticket): Path<u32>,
) -> Result<StatusCode> {
    // Get Soulseek engine
    let engine = state
        .soulseek_engine()
        .ok_or_else(|| AppError::ServiceUnavailable("Soulseek is not configured".to_string()))?;

    // Cancel the search
    engine.cancel_search(ticket).await?;

    tracing::info!(ticket = ticket, "Cancelled Soulseek search");

    Ok(StatusCode::NO_CONTENT)
}

/// GET /api/soulseek/status
///
/// Get Soulseek engine status.
pub async fn get_status(State(state): State<AppState>) -> Result<Json<StatusResponse>> {
    // Get Soulseek engine (return disconnected status if not configured)
    let stats = match state.soulseek_engine() {
        Some(engine) => engine.get_stats().await,
        None => SoulseekStats {
            connected: false,
            active_searches: 0,
            active_downloads: 0,
            completed_downloads: 0,
        },
    };

    Ok(Json(StatusResponse {
        connected: stats.connected,
        active_searches: stats.active_searches,
        active_downloads: stats.active_downloads,
        completed_downloads: stats.completed_downloads,
    }))
}

// =============================================================================
// Download Handlers
// =============================================================================

/// POST /api/soulseek/download
///
/// Start a new Soulseek download.
/// Returns 201 Created with the download details.
pub async fn start_download(
    State(state): State<AppState>,
    Json(request): Json<DownloadRequest>,
) -> Result<(StatusCode, Json<DownloadResponse>)> {
    // Validate request
    if request.username.trim().is_empty() {
        return Err(AppError::BadRequest("Username cannot be empty".to_string()));
    }
    if request.filename.trim().is_empty() {
        return Err(AppError::BadRequest("Filename cannot be empty".to_string()));
    }

    // Get Soulseek engine
    let engine = state
        .soulseek_engine()
        .ok_or_else(|| AppError::ServiceUnavailable("Soulseek is not configured".to_string()))?;

    // Check if connected
    if !engine.is_connected() {
        return Err(AppError::ServiceUnavailable(
            "Soulseek is not connected".to_string(),
        ));
    }

    // Convert to engine request
    let engine_request = EngineDownloadRequest {
        username: request.username.clone(),
        filename: request.filename.clone(),
        size: request.size,
        media_type: request.media_type,
        media_id: request.media_id,
    };

    // Start the download
    let id = engine.download(engine_request).await?;

    tracing::info!(
        id = %id,
        username = %request.username,
        filename = %request.filename,
        "Started Soulseek download"
    );

    // Get the download state
    let download_state = engine
        .get_download(&id)
        .await
        .ok_or_else(|| AppError::Internal("Download state not found after creation".to_string()))?;

    Ok((
        StatusCode::CREATED,
        Json(DownloadResponse::from(&download_state)),
    ))
}

/// GET /api/soulseek/downloads
///
/// List all Soulseek downloads.
pub async fn list_downloads(State(state): State<AppState>) -> Result<Json<Vec<DownloadResponse>>> {
    // Get Soulseek engine
    let engine = state
        .soulseek_engine()
        .ok_or_else(|| AppError::ServiceUnavailable("Soulseek is not configured".to_string()))?;

    let downloads = engine.get_downloads().await;
    let responses: Vec<DownloadResponse> = downloads.iter().map(DownloadResponse::from).collect();

    Ok(Json(responses))
}

/// GET /api/soulseek/downloads/{id}
///
/// Get a specific download by ID.
pub async fn get_download(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<DownloadResponse>> {
    // Get Soulseek engine
    let engine = state
        .soulseek_engine()
        .ok_or_else(|| AppError::ServiceUnavailable("Soulseek is not configured".to_string()))?;

    let download = engine
        .get_download(&id)
        .await
        .ok_or_else(|| AppError::NotFound(format!("Download with ID {} not found", id)))?;

    Ok(Json(DownloadResponse::from(&download)))
}

/// DELETE /api/soulseek/downloads/{id}
///
/// Cancel a download.
pub async fn cancel_download(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<StatusCode> {
    // Get Soulseek engine
    let engine = state
        .soulseek_engine()
        .ok_or_else(|| AppError::ServiceUnavailable("Soulseek is not configured".to_string()))?;

    engine.cancel_download(&id).await?;

    tracing::info!(id = %id, "Cancelled Soulseek download");

    Ok(StatusCode::NO_CONTENT)
}

// =============================================================================
// Browse Handlers
// =============================================================================

/// GET /api/soulseek/browse/{username}
///
/// Browse a user's shared files.
pub async fn browse_user(
    State(state): State<AppState>,
    Path(username): Path<String>,
) -> Result<Json<BrowseResponse>> {
    // Validate username
    let username = username.trim();
    if username.is_empty() {
        return Err(AppError::BadRequest("Username cannot be empty".to_string()));
    }

    // Get Soulseek engine
    let engine = state
        .soulseek_engine()
        .ok_or_else(|| AppError::ServiceUnavailable("Soulseek is not configured".to_string()))?;

    // Check if connected
    if !engine.is_connected() {
        return Err(AppError::ServiceUnavailable(
            "Soulseek is not connected".to_string(),
        ));
    }

    tracing::info!(username = %username, "Browsing user's shares");

    // Browse the user
    let directories = engine.browse_user(username).await?;

    // Convert to response format
    let total_files: usize = directories.iter().map(|d| d.file_count).sum();
    let total_directories = directories.len();

    let dir_responses: Vec<BrowseDirectoryResponse> = directories
        .into_iter()
        .map(|dir| BrowseDirectoryResponse {
            path: dir.path,
            file_count: dir.file_count,
            files: dir
                .files
                .into_iter()
                .map(|f| BrowseFileResponse {
                    name: f.name,
                    full_path: f.full_path,
                    size: f.size,
                    extension: f.extension,
                    bitrate: f.bitrate,
                    duration: f.duration,
                })
                .collect(),
        })
        .collect();

    Ok(Json(BrowseResponse {
        username: username.to_string(),
        directories: dir_responses,
        total_directories,
        total_files,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_search_request_deserialize() {
        let json = r#"{"query": "Artist - Album"}"#;
        let request: SearchRequest = serde_json::from_str(json).unwrap();
        assert_eq!(request.query, "Artist - Album");
    }

    #[test]
    fn test_search_results_query_defaults() {
        let query = SearchResultsQuery {
            min_bitrate: None,
            extensions: None,
            free_slots_only: None,
            sort_by: None,
            limit: None,
            offset: None,
        };
        assert!(query.min_bitrate.is_none());
        assert!(query.free_slots_only.unwrap_or(false) == false);
    }

    #[test]
    fn test_status_response_serialize() {
        let response = StatusResponse {
            connected: true,
            active_searches: 2,
            active_downloads: 5,
            completed_downloads: 10,
        };
        let json = serde_json::to_string(&response).unwrap();
        assert!(json.contains("\"connected\":true"));
        assert!(json.contains("\"active_searches\":2"));
    }
}
