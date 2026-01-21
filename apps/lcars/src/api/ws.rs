//! WebSocket API endpoint for real-time download progress updates.

use axum::{
    extract::{
        ws::{Message, WebSocket, WebSocketUpgrade},
        Query, State,
    },
    response::Response,
};
use futures::{SinkExt, StreamExt};
use serde::{Deserialize, Serialize};
use tokio::sync::broadcast;

use crate::error::Result;
use crate::services::torrent::TorrentEvent;
use crate::AppState;

// =============================================================================
// Request/Response Types
// =============================================================================

/// Query parameters for WebSocket connection.
///
/// # Security Note
/// The token is passed as a query parameter due to WebSocket limitations
/// (browsers don't support custom headers in WebSocket handshake).
/// Tokens may be exposed in server logs or browser history.
/// Consider using short-lived tokens for WebSocket connections.
#[derive(Debug, Deserialize)]
pub struct WsQuery {
    /// JWT token for authentication.
    pub token: String,
}

/// WebSocket message sent to clients.
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type", content = "data", rename_all = "snake_case")]
pub enum WsMessage {
    /// A new download was added.
    DownloadAdded { info_hash: String, name: String },

    /// Download progress update.
    DownloadProgress {
        info_hash: String,
        progress: f64,
        download_speed: u64,
        upload_speed: u64,
        peers: usize,
    },

    /// Download completed.
    DownloadCompleted { info_hash: String },

    /// Download status changed.
    DownloadStatus {
        info_hash: String,
        status: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        error_message: Option<String>,
    },

    /// Download was removed.
    DownloadRemoved { info_hash: String },

    /// System status update (reserved for future use).
    #[allow(dead_code)]
    SystemStatus { active_downloads: usize },

    /// Error message.
    Error { message: String },
}

// =============================================================================
// Handler
// =============================================================================

/// GET /api/ws
///
/// WebSocket endpoint for real-time updates.
/// Requires authentication via `?token=<jwt>` query parameter.
pub async fn ws_handler(
    ws: WebSocketUpgrade,
    State(state): State<AppState>,
    Query(query): Query<WsQuery>,
) -> Result<Response> {
    // Validate JWT token
    let _claims = state.auth_service().verify_token(&query.token)?;

    tracing::debug!("WebSocket connection authenticated");

    Ok(ws.on_upgrade(move |socket| handle_socket(socket, state)))
}

/// Handles an individual WebSocket connection.
async fn handle_socket(socket: WebSocket, state: AppState) {
    let (mut sender, mut receiver) = socket.split();

    // Get torrent engine for event subscription
    let torrent_engine = match state.torrent_engine() {
        Some(engine) => engine,
        None => {
            // Send error and close if no torrent engine
            let error_msg = WsMessage::Error {
                message: "Torrent engine not available".to_string(),
            };
            if let Ok(json) = serde_json::to_string(&error_msg) {
                let _ = sender.send(Message::Text(json)).await;
            }
            let _ = sender.close().await;
            tracing::warn!("WebSocket closed: torrent engine not available");
            return;
        }
    };

    // Subscribe to torrent events
    let mut event_rx = torrent_engine.subscribe();

    tracing::info!("WebSocket client connected");

    loop {
        tokio::select! {
            // Forward torrent events to WebSocket client
            event = event_rx.recv() => {
                match event {
                    Ok(torrent_event) => {
                        let ws_msg = convert_torrent_event(torrent_event);
                        match serde_json::to_string(&ws_msg) {
                            Ok(json) => {
                                if sender.send(Message::Text(json)).await.is_err() {
                                    tracing::debug!("WebSocket send failed, closing connection");
                                    break;
                                }
                            }
                            Err(e) => {
                                tracing::error!("Failed to serialize WebSocket message: {}", e);
                            }
                        }
                    }
                    Err(broadcast::error::RecvError::Lagged(count)) => {
                        tracing::warn!("WebSocket client lagged, missed {} events", count);
                        // Continue receiving - don't disconnect for lag
                    }
                    Err(broadcast::error::RecvError::Closed) => {
                        tracing::debug!("Event channel closed, closing WebSocket");
                        break;
                    }
                }
            }

            // Handle incoming messages from client
            msg = receiver.next() => {
                match msg {
                    Some(Ok(Message::Ping(data))) => {
                        if sender.send(Message::Pong(data)).await.is_err() {
                            tracing::debug!("WebSocket pong failed, closing connection");
                            break;
                        }
                    }
                    Some(Ok(Message::Pong(_))) => {
                        // Pong received, connection is alive
                    }
                    Some(Ok(Message::Close(_))) => {
                        tracing::debug!("WebSocket client sent close");
                        break;
                    }
                    Some(Ok(Message::Text(_))) => {
                        // Client messages are currently ignored
                        // Could be extended to support client commands
                    }
                    Some(Ok(Message::Binary(_))) => {
                        // Binary messages not supported
                    }
                    Some(Err(e)) => {
                        tracing::debug!("WebSocket error: {}", e);
                        break;
                    }
                    None => {
                        tracing::debug!("WebSocket stream ended");
                        break;
                    }
                }
            }
        }
    }

    tracing::info!("WebSocket client disconnected");
}

/// Converts a TorrentEvent to a WebSocket message.
fn convert_torrent_event(event: TorrentEvent) -> WsMessage {
    match event {
        TorrentEvent::Added { info_hash, name } => WsMessage::DownloadAdded { info_hash, name },

        TorrentEvent::Progress {
            info_hash,
            progress,
            download_speed,
            upload_speed,
            peers,
        } => WsMessage::DownloadProgress {
            info_hash,
            progress,
            download_speed,
            upload_speed,
            peers,
        },

        TorrentEvent::Completed { info_hash } => WsMessage::DownloadCompleted { info_hash },

        TorrentEvent::Error { info_hash, message } => WsMessage::DownloadStatus {
            info_hash,
            status: "failed".to_string(),
            error_message: Some(message),
        },

        TorrentEvent::Removed { info_hash } => WsMessage::DownloadRemoved { info_hash },

        TorrentEvent::Paused { info_hash } => WsMessage::DownloadStatus {
            info_hash,
            status: "paused".to_string(),
            error_message: None,
        },

        TorrentEvent::Resumed { info_hash } => WsMessage::DownloadStatus {
            info_hash,
            status: "downloading".to_string(),
            error_message: None,
        },
    }
}
