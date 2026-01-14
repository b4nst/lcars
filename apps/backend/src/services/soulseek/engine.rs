//! Soulseek engine service.
//!
//! Main service struct that provides Soulseek network functionality
//! for searching and downloading music files.

use rand::Rng;
use soulseek_protocol::{
    message_common::ConnectionType,
    peers::p2p::{
        response::PeerResponse, search::SearchReply, shared_directories::SharedDirectories,
    },
    server::{
        login::LoginRequest, peer::RequestConnectionToPeer, request::ServerRequest,
        response::ServerResponse, search::SearchRequest,
    },
};
use std::collections::HashMap;
use std::net::Ipv4Addr;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tokio::sync::{broadcast, mpsc, oneshot, RwLock};
use uuid::Uuid;

use crate::config::SoulseekConfig;
use crate::error::{AppError, Result};

use super::connection::SoulseekConnection;
use super::events::SoulseekEvent;
use super::peer::PeerConnection;
use super::types::{
    BrowsedDirectory, BrowsedFile, DownloadRequest, DownloadState, DownloadStatus, FileResult,
    SearchResult, SearchState, SoulseekStats,
};

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
    /// Connection status flag.
    connected: AtomicBool,
    /// Username we logged in with.
    username: Arc<RwLock<Option<String>>>,
    /// Pending peer address requests indexed by username.
    pending_peer_addresses: Arc<RwLock<HashMap<String, PendingPeerAddress>>>,
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

        let (event_tx, _) = broadcast::channel(100);

        Ok(Self {
            config,
            connection: Arc::new(RwLock::new(None)),
            event_tx,
            searches: Arc::new(RwLock::new(HashMap::new())),
            downloads: Arc::new(RwLock::new(HashMap::new())),
            connected: AtomicBool::new(false),
            username: Arc::new(RwLock::new(None)),
            pending_peer_addresses: Arc::new(RwLock::new(HashMap::new())),
        })
    }

    /// Create a new Soulseek engine wrapped in Arc for shared access.
    pub async fn new_shared(config: SoulseekConfig) -> Result<Arc<Self>> {
        Ok(Arc::new(Self::new(config).await?))
    }

    /// Connect to the Soulseek server and authenticate.
    pub async fn connect(self: &Arc<Self>) -> Result<()> {
        if self.connected.load(Ordering::SeqCst) {
            return Ok(());
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

        // Establish connection
        let (connection, message_rx) =
            SoulseekConnection::connect(&self.config.server_host, self.config.server_port).await?;

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

        let mut conn_guard = self.connection.write().await;
        if let Some(mut connection) = conn_guard.take() {
            connection.shutdown().await;
        }

        self.connected.store(false, Ordering::SeqCst);

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
    pub fn is_connected(&self) -> bool {
        self.connected.load(Ordering::SeqCst)
    }

    /// Start a file search.
    ///
    /// Returns the search ticket that can be used to retrieve results.
    pub async fn search(&self, query: &str) -> Result<u32> {
        if !self.is_connected() {
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

        SoulseekStats {
            connected: self.is_connected(),
            active_searches,
            active_downloads,
            completed_downloads,
        }
    }

    // =========================================================================
    // Download methods
    // =========================================================================

    /// Initiate a file download from a peer.
    ///
    /// Returns the download ID that can be used to track progress.
    pub async fn download(&self, request: DownloadRequest) -> Result<String> {
        if !self.is_connected() {
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
        if !self.is_connected() {
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
        let shares = tokio::time::timeout(std::time::Duration::from_secs(30), async {
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
        tokio::time::timeout(std::time::Duration::from_secs(10), rx)
            .await
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
        while let Some(message) = message_rx.recv().await {
            if let Err(e) = self.handle_message(message).await {
                tracing::error!(error = %e, "Error handling server message");
            }
        }

        // Connection closed
        self.connected.store(false, Ordering::SeqCst);
        let _ = self.event_tx.send(SoulseekEvent::Disconnected {
            reason: "Connection closed".to_string(),
        });
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
                        self.connected.store(true, Ordering::SeqCst);
                        let _ = self.event_tx.send(SoulseekEvent::Connected);

                        // Set our status to online and configure distributed network
                        self.send_request(ServerRequest::SetOnlineStatus(2)).await?;
                        self.send_request(ServerRequest::SetListenPort(
                            self.config.listen_port as u32,
                        ))
                        .await?;
                    }
                    soulseek_protocol::server::login::LoginResponse::Failure { reason } => {
                        tracing::warn!(reason = %reason, "Login failed");
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
                self.connected.store(false, Ordering::SeqCst);
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
        assert!(!engine.is_connected());
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
}
