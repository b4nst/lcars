//! Soulseek engine service.
//!
//! Main service struct that provides Soulseek network functionality
//! for searching and downloading music files.

use rand::Rng;
use soulseek_protocol::{
    peers::p2p::search::SearchReply,
    server::{
        login::LoginRequest, request::ServerRequest, response::ServerResponse,
        search::SearchRequest,
    },
};
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tokio::sync::{broadcast, mpsc, RwLock};

use crate::config::SoulseekConfig;
use crate::error::{AppError, Result};

use super::connection::SoulseekConnection;
use super::events::SoulseekEvent;
use super::types::{FileResult, SearchResult, SearchState, SoulseekStats};

/// Soulseek client engine.
///
/// Manages connection to the Soulseek server and provides search functionality.
/// P2P file transfers will be added in a future phase.
pub struct SoulseekEngine {
    /// Configuration.
    config: SoulseekConfig,
    /// Active server connection.
    connection: Arc<RwLock<Option<SoulseekConnection>>>,
    /// Event broadcaster.
    event_tx: broadcast::Sender<SoulseekEvent>,
    /// Active searches indexed by ticket.
    searches: Arc<RwLock<HashMap<u32, SearchState>>>,
    /// Connection status flag.
    connected: AtomicBool,
    /// Username we logged in with.
    username: Arc<RwLock<Option<String>>>,
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
            connected: AtomicBool::new(false),
            username: Arc::new(RwLock::new(None)),
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

        SoulseekStats {
            connected: self.is_connected(),
            active_searches,
            active_downloads: 0,    // TODO: Phase 3
            completed_downloads: 0, // TODO: Phase 3
        }
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
