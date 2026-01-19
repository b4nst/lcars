//! Soulseek engine service.
//!
//! Main service struct that provides Soulseek network functionality
//! for searching and downloading music files.

use chrono::{DateTime, Utc};

// =============================================================================
// Constants
// =============================================================================

/// Size of the internal message channel buffer.
const MESSAGE_CHANNEL_SIZE: usize = 100;

/// Timeout for peer address resolution from server.
const PEER_ADDRESS_TIMEOUT_SECS: u64 = 10;

/// Timeout for waiting for browse response from a peer.
const BROWSE_TIMEOUT_SECS: u64 = 30;

use rand::Rng;
use soulseek_protocol::{
    message_common::ConnectionType,
    peers::p2p::{
        response::PeerResponse, search::SearchReply, shared_directories::SharedDirectories,
    },
    server::{
        login::LoginRequest, peer::RequestConnectionToPeer, request::ServerRequest,
        response::ServerResponse, search::SearchRequest, shares::SharedFolderAndFiles,
    },
};
use std::collections::HashMap;
use std::net::Ipv4Addr;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{broadcast, mpsc, oneshot, RwLock};
use tokio::task::JoinHandle;
use uuid::Uuid;

use crate::config::SoulseekConfig;
use crate::error::{AppError, Result};

use super::connection::SoulseekConnection;
use super::events::SoulseekEvent;
use super::listener::PeerListener;
use super::peer::PeerConnection;
use super::shares::ShareIndex;
use super::types::{
    BrowsedDirectory, BrowsedFile, ConnectionState, DownloadRequest, DownloadState, DownloadStatus,
    FileResult, SearchResult, SearchState, ShareStatsResponse, SoulseekStats,
};
use super::uploads::{UploadQueue, UploadState};

/// Pending peer address request - waiting for server to provide IP/port.
struct PendingPeerAddress {
    /// Channel to send the result back.
    tx: oneshot::Sender<Result<(Ipv4Addr, u32)>>,
}

/// Soulseek client engine.
///
/// Manages connection to the Soulseek server and provides search functionality.
/// Also handles P2P connections for browsing and downloading.
pub struct SoulseekEngine {
    /// Configuration.
    config: SoulseekConfig,
    /// Active server connection.
    connection: Arc<RwLock<Option<SoulseekConnection>>>,
    /// Event broadcaster.
    event_tx: broadcast::Sender<SoulseekEvent>,
    /// Active searches indexed by ticket.
    searches: Arc<RwLock<HashMap<u32, SearchState>>>,
    /// Active downloads indexed by ID.
    downloads: Arc<RwLock<HashMap<String, DownloadState>>>,
    /// Username we logged in with.
    username: Arc<RwLock<Option<String>>>,
    /// Pending peer address requests indexed by username.
    pending_peer_addresses: Arc<RwLock<HashMap<String, PendingPeerAddress>>>,
    // =========================================================================
    // Connection state management
    // =========================================================================
    /// Current connection state.
    connection_state: Arc<RwLock<ConnectionState>>,
    /// When the connection was established (if connected).
    connected_since: Arc<RwLock<Option<DateTime<Utc>>>>,
    /// Number of reconnection attempts.
    reconnect_attempts: AtomicU32,
    /// Handle to the reconnection task.
    reconnect_handle: RwLock<Option<JoinHandle<()>>>,
    /// Handle to the keepalive task.
    keepalive_handle: RwLock<Option<JoinHandle<()>>>,
    // =========================================================================
    // Sharing fields
    // =========================================================================
    /// Index of shared files.
    share_index: Arc<RwLock<ShareIndex>>,
    /// Upload queue and state.
    upload_queue: Arc<RwLock<UploadQueue>>,
    /// Handle to the peer listener task.
    listener_handle: RwLock<Option<JoinHandle<()>>>,
}

impl SoulseekEngine {
    /// Create a new Soulseek engine with the given configuration.
    ///
    /// The engine is created in a disconnected state. Call `connect()` to
    /// establish a connection to the server.
    pub async fn new(config: SoulseekConfig) -> Result<Self> {
        tracing::debug!(?config, "Creating Soulseek engine");

        // Validate configuration
        if config.username.is_none() || config.password.is_none() {
            return Err(AppError::BadRequest(
                "Soulseek username and password are required".to_string(),
            ));
        }

        let (event_tx, _) = broadcast::channel(MESSAGE_CHANNEL_SIZE);

        let upload_queue = UploadQueue::new(config.upload_slots);

        Ok(Self {
            config,
            connection: Arc::new(RwLock::new(None)),
            event_tx,
            searches: Arc::new(RwLock::new(HashMap::new())),
            downloads: Arc::new(RwLock::new(HashMap::new())),
            username: Arc::new(RwLock::new(None)),
            pending_peer_addresses: Arc::new(RwLock::new(HashMap::new())),
            connection_state: Arc::new(RwLock::new(ConnectionState::Disconnected)),
            connected_since: Arc::new(RwLock::new(None)),
            reconnect_attempts: AtomicU32::new(0),
            reconnect_handle: RwLock::new(None),
            keepalive_handle: RwLock::new(None),
            share_index: Arc::new(RwLock::new(ShareIndex::new())),
            upload_queue: Arc::new(RwLock::new(upload_queue)),
            listener_handle: RwLock::new(None),
        })
    }

    /// Create a new Soulseek engine wrapped in Arc for shared access.
    pub async fn new_shared(config: SoulseekConfig) -> Result<Arc<Self>> {
        Ok(Arc::new(Self::new(config).await?))
    }

    /// Connect to the Soulseek server and authenticate.
    pub async fn connect(self: &Arc<Self>) -> Result<()> {
        // Check if already connected
        {
            let state = self.connection_state.read().await;
            if matches!(*state, ConnectionState::Connected) {
                return Ok(());
            }
        }

        let username =
            self.config.username.as_ref().ok_or_else(|| {
                AppError::BadRequest("Soulseek username not configured".to_string())
            })?;
        let password =
            self.config.password.as_ref().ok_or_else(|| {
                AppError::BadRequest("Soulseek password not configured".to_string())
            })?;

        tracing::info!(
            host = %self.config.server_host,
            port = %self.config.server_port,
            username = %username,
            "Connecting to Soulseek server"
        );

        // Update state to connecting
        {
            let mut state = self.connection_state.write().await;
            *state = ConnectionState::Connecting;
        }

        // Establish connection with timeout
        let connect_result = tokio::time::timeout(
            Duration::from_secs(self.config.connect_timeout),
            SoulseekConnection::connect(&self.config.server_host, self.config.server_port),
        )
        .await;

        let (connection, message_rx) = match connect_result {
            Ok(Ok(conn)) => conn,
            Ok(Err(e)) => {
                self.set_connection_state(ConnectionState::Failed {
                    error: e.to_string(),
                })
                .await;
                return Err(e);
            }
            Err(_) => {
                let error = format!(
                    "Connection timeout after {} seconds",
                    self.config.connect_timeout
                );
                self.set_connection_state(ConnectionState::Failed {
                    error: error.clone(),
                })
                .await;
                return Err(AppError::Internal(error));
            }
        };

        // Update state to authenticating
        {
            let mut state = self.connection_state.write().await;
            *state = ConnectionState::Authenticating;
        }

        // Store connection
        {
            let mut conn_guard = self.connection.write().await;
            *conn_guard = Some(connection);
        }

        // Send login request
        let login = ServerRequest::Login(LoginRequest::new(username, password));
        self.send_request(login).await?;

        // Store username
        {
            let mut username_guard = self.username.write().await;
            *username_guard = Some(username.clone());
        }

        // Spawn message handler
        let engine = Arc::clone(self);
        tokio::spawn(async move {
            engine.handle_messages(message_rx).await;
        });

        Ok(())
    }

    /// Disconnect from the Soulseek server.
    pub async fn disconnect(&self) -> Result<()> {
        tracing::info!("Disconnecting from Soulseek server");

        // Cancel any ongoing reconnection attempts
        {
            let mut handle = self.reconnect_handle.write().await;
            if let Some(h) = handle.take() {
                h.abort();
            }
        }

        // Cancel keepalive task
        {
            let mut handle = self.keepalive_handle.write().await;
            if let Some(h) = handle.take() {
                h.abort();
            }
        }

        let mut conn_guard = self.connection.write().await;
        if let Some(mut connection) = conn_guard.take() {
            connection.shutdown().await;
        }

        // Update state
        self.set_connection_state(ConnectionState::Disconnected)
            .await;

        // Clear connected_since
        {
            let mut since = self.connected_since.write().await;
            *since = None;
        }

        // Reset reconnect attempts
        self.reconnect_attempts.store(0, Ordering::SeqCst);

        let _ = self.event_tx.send(SoulseekEvent::Disconnected {
            reason: "User requested disconnect".to_string(),
        });

        Ok(())
    }

    /// Subscribe to Soulseek events.
    pub fn subscribe(&self) -> broadcast::Receiver<SoulseekEvent> {
        self.event_tx.subscribe()
    }

    /// Check if connected to the server.
    pub async fn is_connected(&self) -> bool {
        let state = self.connection_state.read().await;
        matches!(*state, ConnectionState::Connected)
    }

    /// Get current connection state.
    pub async fn get_connection_state(&self) -> ConnectionState {
        self.connection_state.read().await.clone()
    }

    /// Set connection state and emit event if changed.
    async fn set_connection_state(&self, new_state: ConnectionState) {
        let mut state = self.connection_state.write().await;
        if *state != new_state {
            tracing::debug!(old = %*state, new = %new_state, "Connection state changed");
            *state = new_state;
        }
    }

    /// Start a file search.
    ///
    /// Returns the search ticket that can be used to retrieve results.
    pub async fn search(&self, query: &str) -> Result<u32> {
        if !self.is_connected().await {
            return Err(AppError::ServiceUnavailable(
                "Not connected to Soulseek server".to_string(),
            ));
        }

        // Generate random ticket
        let ticket: u32 = rand::thread_rng().gen();

        tracing::info!(ticket = ticket, query = %query, "Starting search");

        // Create search state
        let search_state = SearchState::new(query.to_string(), ticket);
        {
            let mut searches = self.searches.write().await;
            searches.insert(ticket, search_state);
        }

        // Send search request
        let request = ServerRequest::FileSearch(SearchRequest {
            ticket,
            query: query.to_string(),
        });
        self.send_request(request).await?;

        Ok(ticket)
    }

    /// Get the current state of a search by ticket.
    pub async fn get_search_results(&self, ticket: u32) -> Option<SearchState> {
        let searches = self.searches.read().await;
        // Return a clone since SearchState contains non-Clone Instant
        searches.get(&ticket).map(|s| SearchState {
            query: s.query.clone(),
            ticket: s.ticket,
            started_at: s.started_at,
            results: s.results.clone(),
            complete: s.complete,
        })
    }

    /// Cancel an active search.
    pub async fn cancel_search(&self, ticket: u32) -> Result<()> {
        let mut searches = self.searches.write().await;
        if searches.remove(&ticket).is_some() {
            tracing::debug!(ticket = ticket, "Cancelled search");
            Ok(())
        } else {
            Err(AppError::NotFound(format!(
                "Search with ticket {} not found",
                ticket
            )))
        }
    }

    /// Get statistics about the engine.
    pub async fn get_stats(&self) -> SoulseekStats {
        let connection_state = self.connection_state.read().await.clone();
        let connected = matches!(connection_state, ConnectionState::Connected);

        let username = self.username.read().await.clone();
        let connected_since = *self.connected_since.read().await;
        let reconnect_attempts = self.reconnect_attempts.load(Ordering::SeqCst);

        let server = format!("{}:{}", self.config.server_host, self.config.server_port);

        let searches = self.searches.read().await;
        let active_searches = searches.values().filter(|s| !s.complete).count();

        let downloads = self.downloads.read().await;
        let active_downloads = downloads
            .values()
            .filter(|d| {
                matches!(
                    d.status,
                    DownloadStatus::Connecting
                        | DownloadStatus::Queued
                        | DownloadStatus::Downloading
                )
            })
            .count();
        let completed_downloads = downloads
            .values()
            .filter(|d| d.status == DownloadStatus::Completed)
            .count();

        let upload_queue = self.upload_queue.read().await;
        let active_uploads = upload_queue.active_count();
        let queued_uploads = upload_queue.pending_count();
        let total_uploaded = upload_queue.total_uploaded();
        drop(upload_queue);

        let share_index = self.share_index.read().await;
        let shared_files = share_index.file_count();
        let shared_folders = share_index.folder_count();
        drop(share_index);

        SoulseekStats {
            connection_state,
            connected,
            server,
            username,
            connected_since,
            reconnect_attempts,
            active_searches,
            active_downloads,
            completed_downloads,
            active_uploads,
            queued_uploads,
            total_uploaded,
            shared_files,
            shared_folders,
        }
    }

    // =========================================================================
    // Download methods
    // =========================================================================

    /// Initiate a file download from a peer.
    ///
    /// Returns the download ID that can be used to track progress.
    pub async fn download(&self, request: DownloadRequest) -> Result<String> {
        if !self.is_connected().await {
            return Err(AppError::ServiceUnavailable(
                "Not connected to Soulseek server".to_string(),
            ));
        }

        // Generate unique download ID and ticket
        let id = Uuid::new_v4().to_string();
        let ticket: u32 = rand::thread_rng().gen();

        tracing::info!(
            id = %id,
            username = %request.username,
            filename = %request.filename,
            ticket = ticket,
            "Starting Soulseek download"
        );

        // Create download state
        let mut download_state = DownloadState::new(
            id.clone(),
            request.username.clone(),
            request.filename.clone(),
            request.size,
            ticket,
        );
        download_state.media_type = request.media_type.clone();
        download_state.media_id = request.media_id;

        // Store download state
        {
            let mut downloads = self.downloads.write().await;
            downloads.insert(id.clone(), download_state);
        }

        // Emit event
        let _ = self.event_tx.send(SoulseekEvent::DownloadQueued {
            id: id.clone(),
            username: request.username.clone(),
            filename: request.filename.clone(),
        });

        // Request peer address from server
        let connect_req =
            RequestConnectionToPeer::new(request.username.clone(), ConnectionType::PeerToPeer);
        self.send_request(ServerRequest::ConnectToPeer(connect_req))
            .await?;

        Ok(id)
    }

    /// Get the current state of a download.
    pub async fn get_download(&self, id: &str) -> Option<DownloadState> {
        let downloads = self.downloads.read().await;
        downloads.get(id).cloned()
    }

    /// Get all downloads.
    pub async fn get_downloads(&self) -> Vec<DownloadState> {
        let downloads = self.downloads.read().await;
        downloads.values().cloned().collect()
    }

    /// Cancel an active download.
    pub async fn cancel_download(&self, id: &str) -> Result<()> {
        let mut downloads = self.downloads.write().await;
        if let Some(download) = downloads.get_mut(id) {
            download.status = DownloadStatus::Cancelled;
            tracing::debug!(id = %id, "Cancelled Soulseek download");
            Ok(())
        } else {
            Err(AppError::NotFound(format!(
                "Download with ID {} not found",
                id
            )))
        }
    }

    // =========================================================================
    // Browse methods
    // =========================================================================

    /// Browse a user's shared files.
    ///
    /// Connects directly to the peer and retrieves their shared file list.
    pub async fn browse_user(&self, username: &str) -> Result<Vec<BrowsedDirectory>> {
        if !self.is_connected().await {
            return Err(AppError::ServiceUnavailable(
                "Not connected to Soulseek server".to_string(),
            ));
        }

        tracing::info!(username = %username, "Browsing user's shares");

        // Get peer address from server
        let (ip, port) = self.get_peer_address(username).await?;

        // Get our username
        let our_username = {
            let username_guard = self.username.read().await;
            username_guard
                .clone()
                .ok_or_else(|| AppError::Internal("Not logged in".to_string()))?
        };

        // Generate connection token
        let token: u32 = rand::thread_rng().gen();

        // Connect to peer
        let (mut peer_conn, mut message_rx) =
            PeerConnection::connect(username, ip, port, &our_username, token).await?;

        // Request shares
        peer_conn.request_shares().await?;

        // Wait for shares reply with timeout
        let shares = tokio::time::timeout(Duration::from_secs(BROWSE_TIMEOUT_SECS), async {
            while let Some(msg) = message_rx.recv().await {
                if let PeerResponse::SharesReply(dirs) = msg {
                    return Ok(dirs);
                }
            }
            Err(AppError::Internal(
                "Peer disconnected before sending shares".to_string(),
            ))
        })
        .await
        .map_err(|_| {
            AppError::Internal(format!("Timeout waiting for shares from {}", username))
        })??;

        // Shutdown peer connection
        peer_conn.shutdown().await;

        // Convert to our format
        Ok(convert_shared_directories(shares))
    }

    // =========================================================================
    // Share methods
    // =========================================================================

    /// Rebuild the share index by scanning configured directories.
    pub async fn rebuild_share_index(&self) -> Result<()> {
        if !self.config.sharing_enabled {
            tracing::debug!("Sharing not enabled, skipping index rebuild");
            return Ok(());
        }

        tracing::info!(
            dirs = ?self.config.share_dirs,
            "Rebuilding share index"
        );

        let new_index = ShareIndex::scan(&self.config.share_dirs, self.config.share_hidden).await?;

        let files = new_index.file_count();
        let folders = new_index.folder_count();

        {
            let mut index = self.share_index.write().await;
            *index = new_index;
        }

        // Emit event
        let _ = self
            .event_tx
            .send(SoulseekEvent::ShareIndexUpdated { files, folders });

        // If connected, update the server with our new share count
        if self.is_connected().await {
            self.send_share_count().await?;
        }

        Ok(())
    }

    /// Send our share count to the server.
    async fn send_share_count(&self) -> Result<()> {
        let index = self.share_index.read().await;
        let folders = index.folder_count() as u32;
        let files = index.file_count() as u32;
        drop(index);

        self.send_request(ServerRequest::SharedFolderAndFiles(SharedFolderAndFiles {
            dirs: folders,
            files,
        }))
        .await
    }

    /// Get share statistics.
    pub async fn get_share_stats(&self) -> ShareStatsResponse {
        let index = self.share_index.read().await;
        let stats = index.stats();

        ShareStatsResponse {
            directories: self
                .config
                .share_dirs
                .iter()
                .map(|p| p.display().to_string())
                .collect(),
            total_files: stats.total_files,
            total_folders: stats.total_folders,
            total_size: stats.total_size,
            last_indexed: stats.last_indexed.map(|_| {
                // Convert to approximate ISO 8601
                chrono::Utc::now().to_rfc3339()
            }),
            sharing_enabled: self.config.sharing_enabled,
        }
    }

    /// Start the keepalive task that periodically sends status updates.
    ///
    /// This helps maintain the connection and lets the server know we're still alive.
    pub async fn start_keepalive(self: &Arc<Self>) {
        let interval_secs = self.config.keepalive_interval;
        if interval_secs == 0 {
            tracing::debug!("Keepalive disabled (interval is 0)");
            return;
        }

        let engine = Arc::clone(self);
        let handle = tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(interval_secs));
            interval.tick().await; // Skip the first immediate tick

            loop {
                interval.tick().await;

                // Check if we're still connected
                if !engine.is_connected().await {
                    tracing::debug!("Keepalive stopping: not connected");
                    break;
                }

                // Send online status as keepalive
                // Status 2 = Online
                if let Err(e) = engine.send_request(ServerRequest::SetOnlineStatus(2)).await {
                    tracing::warn!(error = %e, "Failed to send keepalive");
                    // Connection might be dead, the message handler will detect this
                } else {
                    tracing::trace!("Keepalive sent");
                }
            }
        });

        // Store the handle
        {
            let mut guard = self.keepalive_handle.write().await;
            *guard = Some(handle);
        }

        tracing::debug!(interval_secs = interval_secs, "Keepalive task started");
    }

    /// Start the peer listener for incoming connections.
    pub async fn start_listener(self: &Arc<Self>) -> Result<()> {
        if !self.config.sharing_enabled {
            tracing::debug!("Sharing not enabled, not starting peer listener");
            return Ok(());
        }

        let username = {
            let guard = self.username.read().await;
            guard.clone().unwrap_or_else(|| "anonymous".to_string())
        };

        let listener = PeerListener::bind(
            self.config.listen_port,
            Arc::clone(&self.share_index),
            Arc::clone(&self.upload_queue),
            username,
            0, // TODO: Calculate from upload_speed_limit
        )
        .await?;

        let handle = tokio::spawn(async move {
            listener.run().await;
        });

        {
            let mut guard = self.listener_handle.write().await;
            *guard = Some(handle);
        }

        tracing::info!(port = self.config.listen_port, "Peer listener started");
        Ok(())
    }

    // =========================================================================
    // Upload methods
    // =========================================================================

    /// Get all uploads (active and recent).
    pub async fn get_uploads(&self) -> Vec<UploadState> {
        let queue = self.upload_queue.read().await;
        queue.get_active().into_iter().cloned().collect()
    }

    /// Cancel an upload.
    pub async fn cancel_upload(&self, id: &str) -> Result<()> {
        let mut queue = self.upload_queue.write().await;
        if queue.cancel(id) {
            tracing::debug!(id = %id, "Cancelled upload");
            Ok(())
        } else {
            Err(AppError::NotFound(format!("Upload {} not found", id)))
        }
    }

    /// Get the address (IP and port) of a peer from the server.
    async fn get_peer_address(&self, username: &str) -> Result<(Ipv4Addr, u32)> {
        // Create a channel to receive the address
        let (tx, rx) = oneshot::channel();

        // Register the pending request
        {
            let mut pending = self.pending_peer_addresses.write().await;
            pending.insert(username.to_string(), PendingPeerAddress { tx });
        }

        // Request the address from server
        self.send_request(ServerRequest::GetPeerAddress(username.to_string()))
            .await?;

        // Wait for response with timeout
        let result = tokio::time::timeout(Duration::from_secs(PEER_ADDRESS_TIMEOUT_SECS), rx).await;

        // Clean up pending request on timeout or error
        match &result {
            Ok(Ok(_)) => {}
            _ => {
                let mut pending = self.pending_peer_addresses.write().await;
                pending.remove(username);
            }
        }

        result
            .map_err(|_| {
                AppError::Internal(format!("Timeout waiting for peer address for {}", username))
            })?
            .map_err(|_| AppError::Internal("Peer address request cancelled".to_string()))?
    }

    /// Gracefully stop the engine.
    pub async fn stop(&self) {
        tracing::info!("Stopping Soulseek engine");
        let _ = self.disconnect().await;
    }

    // =========================================================================
    // Private helpers
    // =========================================================================

    /// Send a request to the server.
    async fn send_request(&self, request: ServerRequest) -> Result<()> {
        let conn_guard = self.connection.read().await;
        if let Some(ref connection) = *conn_guard {
            connection.send(request).await
        } else {
            Err(AppError::ServiceUnavailable(
                "Not connected to Soulseek server".to_string(),
            ))
        }
    }

    /// Background task to handle incoming messages from the server.
    async fn handle_messages(self: Arc<Self>, mut message_rx: mpsc::Receiver<ServerResponse>) {
        let mut keepalive_started = false;

        while let Some(message) = message_rx.recv().await {
            // Check for login success to start keepalive
            if !keepalive_started {
                if let ServerResponse::LoginResponse(
                    soulseek_protocol::server::login::LoginResponse::Success { .. },
                ) = &message
                {
                    self.start_keepalive().await;
                    keepalive_started = true;
                }
            }

            if let Err(e) = self.handle_message(message).await {
                tracing::error!(error = %e, "Error handling server message");
            }
        }

        // Connection closed - check if we should auto-reconnect
        let was_connected = {
            let state = self.connection_state.read().await;
            matches!(*state, ConnectionState::Connected)
        };

        if was_connected {
            // Clear connected_since
            {
                let mut since = self.connected_since.write().await;
                *since = None;
            }

            let _ = self.event_tx.send(SoulseekEvent::Disconnected {
                reason: "Connection closed".to_string(),
            });

            // Attempt auto-reconnect if enabled
            if self.config.auto_reconnect {
                self.start_reconnect().await;
            } else {
                self.set_connection_state(ConnectionState::Disconnected)
                    .await;
            }
        }
    }

    /// Start the auto-reconnect process with exponential backoff.
    async fn start_reconnect(self: &Arc<Self>) {
        // Check if we've exceeded max attempts
        let current_attempt = self.reconnect_attempts.fetch_add(1, Ordering::SeqCst) + 1;

        if let Some(max) = self.config.max_reconnect_attempts {
            if current_attempt > max {
                tracing::warn!(
                    attempts = current_attempt,
                    max = max,
                    "Max reconnection attempts reached"
                );
                self.set_connection_state(ConnectionState::Failed {
                    error: format!("Max reconnection attempts ({}) reached", max),
                })
                .await;
                let _ = self.event_tx.send(SoulseekEvent::ConnectionFailed {
                    error: format!("Max reconnection attempts ({}) reached", max),
                });
                return;
            }
        }

        // Calculate delay with exponential backoff: 1s, 2s, 4s, 8s, ... up to max
        let delay_secs = std::cmp::min(
            2u64.saturating_pow(current_attempt.saturating_sub(1)),
            self.config.reconnect_delay_max,
        );

        let next_retry = Utc::now() + chrono::Duration::seconds(delay_secs as i64);

        tracing::info!(
            attempt = current_attempt,
            delay_secs = delay_secs,
            "Scheduling reconnection attempt"
        );

        // Update state to reconnecting
        self.set_connection_state(ConnectionState::Reconnecting {
            attempt: current_attempt,
            next_retry: Some(next_retry),
        })
        .await;

        // Emit event
        let _ = self.event_tx.send(SoulseekEvent::Reconnecting {
            attempt: current_attempt,
            next_retry_secs: delay_secs,
        });

        // Schedule reconnection
        let engine = Arc::clone(self);
        let handle = tokio::spawn(async move {
            tokio::time::sleep(Duration::from_secs(delay_secs)).await;
            engine.attempt_reconnect().await;
        });

        // Store the handle
        {
            let mut guard = self.reconnect_handle.write().await;
            *guard = Some(handle);
        }
    }

    /// Attempt to reconnect to the server.
    ///
    /// This uses `Box::pin` to break the recursive async function cycle
    /// between `connect` -> `handle_messages` -> `start_reconnect` -> `attempt_reconnect` -> `connect`.
    fn attempt_reconnect(
        self: &Arc<Self>,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = ()> + Send + '_>> {
        Box::pin(async move {
            tracing::info!("Attempting to reconnect to Soulseek server");

            // Clear old connection
            {
                let mut conn_guard = self.connection.write().await;
                *conn_guard = None;
            }

            // Attempt connection
            match self.connect().await {
                Ok(()) => {
                    tracing::info!("Reconnection successful");
                    // connect() handles state updates on success
                }
                Err(e) => {
                    tracing::warn!(error = %e, "Reconnection attempt failed");
                    // Schedule next attempt
                    self.start_reconnect().await;
                }
            }
        })
    }

    /// Handle a single server message.
    async fn handle_message(&self, message: ServerResponse) -> Result<()> {
        match message {
            ServerResponse::LoginResponse(response) => {
                match response {
                    soulseek_protocol::server::login::LoginResponse::Success {
                        greeting_message,
                        user_ip,
                        ..
                    } => {
                        tracing::info!(
                            greeting = %greeting_message,
                            ip = %user_ip,
                            "Login successful"
                        );

                        // Update connection state
                        self.set_connection_state(ConnectionState::Connected).await;

                        // Set connected_since timestamp
                        {
                            let mut since = self.connected_since.write().await;
                            *since = Some(Utc::now());
                        }

                        // Reset reconnect attempts on successful connection
                        self.reconnect_attempts.store(0, Ordering::SeqCst);

                        // Get username for event
                        let username = {
                            let guard = self.username.read().await;
                            guard.clone().unwrap_or_default()
                        };

                        let _ = self.event_tx.send(SoulseekEvent::Connected { username });

                        // Set our status to online and configure distributed network
                        self.send_request(ServerRequest::SetOnlineStatus(2)).await?;
                        self.send_request(ServerRequest::SetListenPort(
                            self.config.listen_port as u32,
                        ))
                        .await?;

                        // Report share count to server
                        self.send_share_count().await?;
                    }
                    soulseek_protocol::server::login::LoginResponse::Failure { reason } => {
                        tracing::warn!(reason = %reason, "Login failed");
                        self.set_connection_state(ConnectionState::Failed {
                            error: reason.clone(),
                        })
                        .await;
                        let _ = self.event_tx.send(SoulseekEvent::LoginFailed {
                            reason: reason.clone(),
                        });
                        return Err(AppError::Unauthorized);
                    }
                }
            }

            ServerResponse::SearchReply(_search_query) => {
                // SearchReply from server contains ticket and username
                // The actual file results come from distributed/peer network
                // Fields are private in SearchQuery, so we just log that we received it
                tracing::debug!("Received search relay from server");
            }

            ServerResponse::KickedFromServer => {
                tracing::warn!("Kicked from server");
                self.set_connection_state(ConnectionState::Failed {
                    error: "Kicked from server".to_string(),
                })
                .await;
                let _ = self.event_tx.send(SoulseekEvent::Disconnected {
                    reason: "Kicked from server".to_string(),
                });
            }

            ServerResponse::RoomList(room_list) => {
                tracing::debug!(?room_list, "Received room list");
            }

            ServerResponse::PrivilegedUsers(_users) => {
                // UserList is a newtype wrapper, doesn't expose len()
                tracing::debug!("Received privileged users list");
            }

            ServerResponse::ParentMinSpeed(speed) => {
                tracing::trace!(speed = speed, "Parent min speed");
            }

            ServerResponse::ParentSpeedRatio(ratio) => {
                tracing::trace!(ratio = ratio, "Parent speed ratio");
            }

            ServerResponse::WishlistInterval(interval) => {
                tracing::trace!(interval = interval, "Wishlist interval");
            }

            ServerResponse::EmbeddedMessage(embedded) => {
                // Embedded messages contain distributed search requests/results
                self.handle_embedded_message(embedded).await?;
            }

            ServerResponse::PeerAddress(peer_address) => {
                tracing::debug!(
                    username = %peer_address.username,
                    ip = %peer_address.ip,
                    port = peer_address.port,
                    "Received peer address"
                );

                // Complete the pending request
                let mut pending = self.pending_peer_addresses.write().await;
                if let Some(request) = pending.remove(&peer_address.username) {
                    let _ = request.tx.send(Ok((peer_address.ip, peer_address.port)));
                }
            }

            ServerResponse::PeerConnectionRequest(connection_request) => {
                tracing::debug!(
                    username = %connection_request.username,
                    ip = %connection_request.ip,
                    port = connection_request.port,
                    token = connection_request.token,
                    "Received peer connection request"
                );
                // For now, we don't handle incoming connections
                // This would be needed for users trying to download from us
            }

            ServerResponse::Unknown(len, code, _data) => {
                tracing::trace!(code = code, len = len, "Received unknown message");
            }

            _ => {
                tracing::trace!(?message, "Unhandled server message");
            }
        }

        Ok(())
    }

    /// Handle embedded distributed messages (search results from the network).
    async fn handle_embedded_message(
        &self,
        _embedded: soulseek_protocol::server::distributed::EmbeddedDistributedMessage,
    ) -> Result<()> {
        // EmbeddedDistributedMessage contains code and raw message bytes
        // For now we just log it - parsing the internal message would require
        // additional work based on the code field
        tracing::trace!("Received embedded distributed message");
        // We don't share files yet, so we ignore incoming distributed messages
        Ok(())
    }

    /// Process search results from a peer.
    ///
    /// This is called when we receive search results either through the
    /// distributed network or directly from a peer connection.
    #[allow(dead_code)] // Will be used in Phase 2 (P2P connections)
    pub(crate) async fn process_search_results(&self, reply: SearchReply) {
        let ticket = reply.ticket;

        let files: Vec<FileResult> = reply
            .files
            .iter()
            .map(FileResult::from_protocol_file)
            .collect();

        // Update search state
        {
            let mut searches = self.searches.write().await;
            if let Some(search_state) = searches.get_mut(&ticket) {
                let result = SearchResult {
                    username: reply.username.clone(),
                    files: files.clone(),
                    has_free_slot: reply.slot_free,
                    average_speed: reply.average_speed,
                    queue_length: reply.queue_length,
                };
                search_state.add_results(result);
            }
        }

        // Emit event
        let _ = self.event_tx.send(SoulseekEvent::SearchResult {
            ticket,
            username: reply.username,
            files,
            has_free_slot: reply.slot_free,
            average_speed: reply.average_speed,
            queue_length: reply.queue_length,
        });
    }
}

/// Convert protocol SharedDirectories to our BrowsedDirectory format.
fn convert_shared_directories(dirs: SharedDirectories) -> Vec<BrowsedDirectory> {
    dirs.dirs
        .into_iter()
        .map(|dir| {
            let files: Vec<BrowsedFile> = dir
                .files
                .into_iter()
                .map(|file| {
                    // Extract attributes
                    let mut bitrate = None;
                    let mut duration = None;

                    for attr in &file.attributes {
                        match attr.place {
                            0 => bitrate = Some(attr.attribute),
                            1 => duration = Some(attr.attribute),
                            _ => {}
                        }
                    }

                    // Get filename from full path
                    let name = file
                        .name
                        .rsplit(['/', '\\'])
                        .next()
                        .unwrap_or(&file.name)
                        .to_string();

                    let extension = if !file.extension.is_empty() {
                        file.extension.to_lowercase()
                    } else {
                        name.rsplit('.').next().unwrap_or("").to_lowercase()
                    };

                    BrowsedFile {
                        name,
                        full_path: file.name,
                        size: file.size,
                        extension,
                        bitrate,
                        duration,
                    }
                })
                .collect();

            BrowsedDirectory {
                path: dir.name,
                file_count: files.len(),
                files,
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_config() -> SoulseekConfig {
        SoulseekConfig {
            username: Some("test_user".to_string()),
            password: Some("test_pass".to_string()),
            ..Default::default()
        }
    }

    #[tokio::test]
    async fn test_engine_creation() {
        let engine = SoulseekEngine::new(test_config()).await;
        assert!(engine.is_ok());

        let engine = engine.unwrap();
        assert!(!engine.is_connected().await);
    }

    #[tokio::test]
    async fn test_engine_without_credentials() {
        let config = SoulseekConfig::default();
        let result = SoulseekEngine::new(config).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_search_without_connection() {
        let engine = SoulseekEngine::new(test_config()).await.unwrap();
        let result = engine.search("test query").await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_get_stats() {
        let engine = SoulseekEngine::new(test_config()).await.unwrap();
        let stats = engine.get_stats().await;
        assert!(!stats.connected);
        assert_eq!(stats.active_searches, 0);
    }

    #[tokio::test]
    async fn test_get_connection_state() {
        let engine = SoulseekEngine::new(test_config()).await.unwrap();
        let state = engine.get_connection_state().await;
        assert_eq!(state, ConnectionState::Disconnected);
    }

    #[tokio::test]
    async fn test_get_search_results_nonexistent() {
        let engine = SoulseekEngine::new(test_config()).await.unwrap();
        let result = engine.get_search_results(12345).await;
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn test_cancel_search_nonexistent() {
        let engine = SoulseekEngine::new(test_config()).await.unwrap();
        let result = engine.cancel_search(12345).await;
        // Should return error for nonexistent search
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_get_download_nonexistent() {
        let engine = SoulseekEngine::new(test_config()).await.unwrap();
        let result = engine.get_download("nonexistent-id").await;
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn test_get_downloads_empty() {
        let engine = SoulseekEngine::new(test_config()).await.unwrap();
        let downloads = engine.get_downloads().await;
        assert!(downloads.is_empty());
    }

    #[tokio::test]
    async fn test_cancel_download_nonexistent() {
        let engine = SoulseekEngine::new(test_config()).await.unwrap();
        let result = engine.cancel_download("nonexistent-id").await;
        // Should return error for nonexistent download
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_download_without_connection() {
        let engine = SoulseekEngine::new(test_config()).await.unwrap();
        let result = engine
            .download(DownloadRequest {
                username: "testuser".to_string(),
                filename: "/Music/test.flac".to_string(),
                size: 1000,
                media_type: None,
                media_id: None,
            })
            .await;
        // Should fail without connection
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_browse_user_without_connection() {
        let engine = SoulseekEngine::new(test_config()).await.unwrap();
        let result = engine.browse_user("someuser").await;
        // Should fail without connection
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_get_uploads_empty() {
        let engine = SoulseekEngine::new(test_config()).await.unwrap();
        let uploads = engine.get_uploads().await;
        assert!(uploads.is_empty());
    }

    #[tokio::test]
    async fn test_cancel_upload_nonexistent() {
        let engine = SoulseekEngine::new(test_config()).await.unwrap();
        let result = engine.cancel_upload("nonexistent-id").await;
        // Should return error for nonexistent upload
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_get_share_stats() {
        let engine = SoulseekEngine::new(test_config()).await.unwrap();
        let stats = engine.get_share_stats().await;
        // With default config (no share dirs), should be empty
        assert!(stats.directories.is_empty());
        assert_eq!(stats.total_files, 0);
        assert_eq!(stats.total_folders, 0);
    }

    #[tokio::test]
    async fn test_subscribe() {
        let engine = SoulseekEngine::new(test_config()).await.unwrap();
        let _rx = engine.subscribe();
        // Should be able to subscribe multiple times
        let _rx2 = engine.subscribe();
    }
}
