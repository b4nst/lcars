//! Soulseek API endpoints for search and status.

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
use crate::services::soulseek::SoulseekStats;
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
