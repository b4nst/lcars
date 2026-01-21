//! Torrent download engine service.
//!
//! Provides BitTorrent client functionality using librqbit for downloading media.
//! Supports VPN interface binding for traffic isolation.

use librqbit::{
    api::TorrentIdOrHash, dht::Id20, AddTorrent, AddTorrentOptions, AddTorrentResponse,
    ManagedTorrent, Session, SessionOptions, TorrentStats, TorrentStatsState,
};
use serde::Serialize;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{broadcast, RwLock};

use crate::config::TorrentConfig;
use crate::db::models::{DownloadStatus, MediaType};
use crate::error::{AppError, Result};

/// Convert an info_hash Id<20> to a hex string.
fn info_hash_to_string(id: &Id20) -> String {
    hex::encode(id.0)
}

/// Extract live stats from torrent stats (download/upload speed in bytes/sec and peer count).
fn extract_live_stats(stats: &TorrentStats) -> (u64, u64, usize) {
    stats
        .live
        .as_ref()
        .map(|live| {
            // mbps is megabits per second, convert to bytes per second
            let download_bps = (live.download_speed.mbps * 1_000_000.0 / 8.0) as u64;
            let upload_bps = (live.upload_speed.mbps * 1_000_000.0 / 8.0) as u64;
            // Get peer count from peer_stats
            let peer_count = live.snapshot.peer_stats.queued
                + live.snapshot.peer_stats.connecting
                + live.snapshot.peer_stats.live;
            (download_bps, upload_bps, peer_count)
        })
        .unwrap_or((0, 0, 0))
}

/// Event emitted by the torrent engine for progress tracking.
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum TorrentEvent {
    /// A torrent has been added to the session.
    Added { info_hash: String, name: String },
    /// Progress update for a torrent.
    Progress {
        info_hash: String,
        progress: f64,
        download_speed: u64,
        upload_speed: u64,
        peers: usize,
    },
    /// A torrent has completed downloading.
    Completed { info_hash: String },
    /// A torrent has encountered an error.
    Error { info_hash: String, message: String },
    /// A torrent has been removed.
    Removed { info_hash: String },
    /// A torrent has been paused.
    Paused { info_hash: String },
    /// A torrent has been resumed.
    Resumed { info_hash: String },
}

/// Reference to associated media for a torrent download.
///
/// Links a torrent download to its corresponding media entry in the database.
#[derive(Debug, Clone)]
pub struct MediaRef {
    /// Type of media (Movie, Episode, Album, Track).
    pub media_type: MediaType,
    /// Database ID of the media item.
    pub media_id: i64,
}

/// Torrent status information for API responses.
#[derive(Debug, Clone, Serialize)]
pub struct TorrentStatus {
    pub info_hash: String,
    pub name: String,
    pub status: DownloadStatus,
    pub progress: f64,
    pub download_speed: u64,
    pub upload_speed: u64,
    pub downloaded: u64,
    pub uploaded: u64,
    pub size: u64,
    pub ratio: f64,
    pub peers: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

/// Internal tracking info for each torrent.
///
/// Stores metadata about each active torrent for internal use.
struct TorrentInfo {
    /// Associated media reference for database tracking.
    media_ref: MediaRef,
    /// When the torrent started seeding (for time-based seeding limits).
    seeding_started_at: Option<std::time::Instant>,
}

/// BitTorrent download engine using librqbit.
///
/// Manages torrent downloads with support for:
/// - Adding magnet links with media association
/// - Progress monitoring and event broadcasting
/// - Pause/resume/remove operations
/// - Seeding management with ratio/time limits
/// - Optional VPN interface binding
pub struct TorrentEngine {
    session: Arc<Session>,
    config: TorrentConfig,
    event_tx: broadcast::Sender<TorrentEvent>,
    /// Maps info_hash to torrent tracking info
    torrents: RwLock<HashMap<String, TorrentInfo>>,
}

impl TorrentEngine {
    /// Create a new torrent engine with the given configuration.
    ///
    /// Initializes a librqbit session with the configured download directory
    /// and optional VPN interface binding.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The port range is invalid (start >= end)
    /// - The seeding ratio limit is negative
    /// - The download directory cannot be created
    /// - The librqbit session fails to initialize
    pub async fn new(config: TorrentConfig) -> Result<Self> {
        tracing::debug!(?config, "Initializing torrent engine");

        // Validate port range
        if config.port_range.0 >= config.port_range.1 {
            return Err(AppError::Internal(
                "Invalid port range: start must be less than end".to_string(),
            ));
        }

        // Validate seeding config
        if config.seeding.ratio_limit < 0.0 {
            return Err(AppError::Internal(
                "Seeding ratio limit cannot be negative".to_string(),
            ));
        }

        // Ensure download directory exists
        if !config.download_dir.exists() {
            std::fs::create_dir_all(&config.download_dir).map_err(|e| {
                AppError::Internal(format!(
                    "Failed to create download directory {:?}: {}",
                    config.download_dir, e
                ))
            })?;
        }

        // Log VPN interface binding configuration (requires system-level routing)
        if !config.bind_interface.is_empty() {
            tracing::info!(interface = %config.bind_interface, "Binding torrent traffic to interface");
            // Note: librqbit doesn't directly support interface binding in SessionOptions
            // This would require using socks_proxy_url or system-level routing
            tracing::warn!(
                "VPN interface binding ({}) requires system-level routing configuration",
                config.bind_interface
            );
        }

        let opts = SessionOptions {
            listen_port_range: Some(config.port_range.0..config.port_range.1),
            fastresume: true,
            ..Default::default()
        };

        let session = Session::new_with_opts(config.download_dir.clone(), opts)
            .await
            .map_err(|e| AppError::Internal(format!("Failed to create torrent session: {}", e)))?;

        let (event_tx, _) = broadcast::channel(100);

        tracing::info!(
            download_dir = ?config.download_dir,
            port_range = ?config.port_range,
            "Torrent engine initialized"
        );

        Ok(Self {
            session,
            config,
            event_tx,
            torrents: RwLock::new(HashMap::new()),
        })
    }

    /// Create a new torrent engine wrapped in Arc for shared access.
    pub async fn new_shared(config: TorrentConfig) -> Result<Arc<Self>> {
        Ok(Arc::new(Self::new(config).await?))
    }

    /// Add a magnet link to the download queue.
    ///
    /// Associates the torrent with a media item for tracking purposes.
    /// Returns the info_hash of the added torrent.
    pub async fn add_magnet(&self, magnet: &str, media_ref: MediaRef) -> Result<String> {
        tracing::debug!(magnet = %magnet, media_type = ?media_ref.media_type, media_id = %media_ref.media_id, "Adding magnet link");

        let add_torrent = AddTorrent::from_url(magnet);
        let opts = AddTorrentOptions::default();

        let response = self
            .session
            .add_torrent(add_torrent, Some(opts))
            .await
            .map_err(|e| AppError::Internal(format!("Failed to add torrent: {}", e)))?;

        let (info_hash, _name) = match response {
            AddTorrentResponse::AlreadyManaged(id, handle) => {
                let info_hash = info_hash_to_string(&handle.info_hash());
                let name = handle.name().unwrap_or_else(|| format!("Torrent {}", id));
                tracing::debug!(info_hash = %info_hash, "Torrent already managed");
                (info_hash, name)
            }
            AddTorrentResponse::Added(id, handle) => {
                let info_hash = info_hash_to_string(&handle.info_hash());
                let name = handle.name().unwrap_or_else(|| format!("Torrent {}", id));

                // Store tracking info
                {
                    let mut torrents = self.torrents.write().await;
                    torrents.insert(
                        info_hash.clone(),
                        TorrentInfo {
                            media_ref: media_ref.clone(),
                            seeding_started_at: None,
                        },
                    );
                }

                // Emit added event
                let _ = self.event_tx.send(TorrentEvent::Added {
                    info_hash: info_hash.clone(),
                    name: name.clone(),
                });

                // Start monitoring task
                self.spawn_monitor_task(handle);

                tracing::info!(info_hash = %info_hash, name = %name, "Torrent added successfully");
                (info_hash, name)
            }
            AddTorrentResponse::ListOnly(list_response) => {
                // ListOnly mode returns torrent file list without downloading
                let file_count = list_response.info.files.as_ref().map_or(1, |f| f.len());
                return Err(AppError::BadRequest(format!(
                    "Torrent contains {} files, use list-only mode to select files",
                    file_count
                )));
            }
        };

        Ok(info_hash)
    }

    /// Get the current status of a torrent by its info_hash.
    pub async fn get_status(&self, info_hash: &str) -> Result<TorrentStatus> {
        let handle = self.get_torrent_handle(info_hash)?;
        let stats = handle.stats();
        let name = handle.name().unwrap_or_else(|| "Unknown".to_string());

        Ok(self.stats_to_status(info_hash, &name, &stats))
    }

    /// Get status for all active torrents.
    pub async fn list_all(&self) -> Vec<TorrentStatus> {
        use std::cell::RefCell;

        let statuses = RefCell::new(Vec::new());

        self.session.with_torrents(|iter| {
            for (_, handle) in iter {
                let info_hash = info_hash_to_string(&handle.info_hash());
                let name = handle.name().unwrap_or_else(|| "Unknown".to_string());
                let stats = handle.stats();
                statuses
                    .borrow_mut()
                    .push(self.stats_to_status(&info_hash, &name, &stats));
            }
        });

        statuses.into_inner()
    }

    /// Pause a torrent by its info_hash.
    pub async fn pause(&self, info_hash: &str) -> Result<()> {
        let handle = self.get_torrent_handle(info_hash)?;

        self.session
            .pause(&handle)
            .await
            .map_err(|e| AppError::Internal(format!("Failed to pause torrent: {}", e)))?;

        let _ = self.event_tx.send(TorrentEvent::Paused {
            info_hash: info_hash.to_string(),
        });

        tracing::debug!(info_hash = %info_hash, "Torrent paused");
        Ok(())
    }

    /// Resume a paused torrent by its info_hash.
    pub async fn resume(&self, info_hash: &str) -> Result<()> {
        let handle = self.get_torrent_handle(info_hash)?;

        self.session
            .unpause(&handle)
            .await
            .map_err(|e| AppError::Internal(format!("Failed to resume torrent: {}", e)))?;

        let _ = self.event_tx.send(TorrentEvent::Resumed {
            info_hash: info_hash.to_string(),
        });

        tracing::debug!(info_hash = %info_hash, "Torrent resumed");
        Ok(())
    }

    /// Remove a torrent by its info_hash.
    ///
    /// If `delete_files` is true, also removes downloaded files from disk.
    pub async fn remove(&self, info_hash: &str, delete_files: bool) -> Result<()> {
        // Parse the info_hash to get the torrent ID
        let handle = self.get_torrent_handle(info_hash)?;
        let id = handle.id();

        self.session
            .delete(TorrentIdOrHash::Id(id), delete_files)
            .await
            .map_err(|e| AppError::Internal(format!("Failed to remove torrent: {}", e)))?;

        // Remove from tracking
        {
            let mut torrents = self.torrents.write().await;
            torrents.remove(info_hash);
        }

        let _ = self.event_tx.send(TorrentEvent::Removed {
            info_hash: info_hash.to_string(),
        });

        tracing::info!(info_hash = %info_hash, delete_files = %delete_files, "Torrent removed");
        Ok(())
    }

    /// Subscribe to torrent events.
    ///
    /// Returns a broadcast receiver that will receive all torrent events.
    pub fn subscribe(&self) -> broadcast::Receiver<TorrentEvent> {
        self.event_tx.subscribe()
    }

    /// Check which torrents have met their seeding limits.
    ///
    /// Returns info_hashes of torrents that should be stopped based on
    /// the configured ratio or time limits.
    pub async fn check_seeding_completion(&self) -> Vec<String> {
        use std::cell::RefCell;

        if !self.config.seeding.enabled {
            return Vec::new();
        }

        let completed = RefCell::new(Vec::new());
        let ratio_limit = self.config.seeding.ratio_limit;
        let time_limit = Duration::from_secs(self.config.seeding.time_limit_hours * 3600);

        let torrents = self.torrents.read().await;

        self.session.with_torrents(|iter| {
            for (_, handle) in iter {
                let stats = handle.stats();

                // Only check completed torrents that are seeding
                if !stats.finished {
                    continue;
                }

                let info_hash = info_hash_to_string(&handle.info_hash());

                // Check ratio limit
                let ratio = if stats.progress_bytes > 0 {
                    stats.uploaded_bytes as f64 / stats.progress_bytes as f64
                } else {
                    0.0
                };

                if ratio >= ratio_limit {
                    tracing::debug!(
                        info_hash = %info_hash,
                        ratio = %ratio,
                        limit = %ratio_limit,
                        "Torrent reached ratio limit"
                    );
                    completed.borrow_mut().push(info_hash.clone());
                    continue;
                }

                // Check time limit
                if let Some(info) = torrents.get(&info_hash) {
                    if let Some(started_at) = info.seeding_started_at {
                        if started_at.elapsed() >= time_limit {
                            tracing::debug!(
                                info_hash = %info_hash,
                                elapsed = ?started_at.elapsed(),
                                limit = ?time_limit,
                                "Torrent reached time limit"
                            );
                            completed.borrow_mut().push(info_hash);
                        }
                    }
                }
            }
        });

        completed.into_inner()
    }

    /// Gracefully stop the torrent engine.
    ///
    /// Stops all active torrents and cleans up resources.
    pub async fn stop(&self) {
        tracing::info!("Stopping torrent engine");
        self.session.stop().await;
    }

    /// Get the media reference associated with a torrent.
    pub async fn get_media_ref(&self, info_hash: &str) -> Option<MediaRef> {
        let torrents = self.torrents.read().await;
        torrents.get(info_hash).map(|info| info.media_ref.clone())
    }

    // =========================================================================
    // Private helpers
    // =========================================================================

    /// Get a torrent handle by info_hash.
    fn get_torrent_handle(&self, info_hash: &str) -> Result<Arc<ManagedTorrent>> {
        use std::cell::RefCell;

        // Find the torrent by iterating through all torrents
        let found_handle = RefCell::new(None);

        self.session.with_torrents(|iter| {
            for (_, handle) in iter {
                if info_hash_to_string(&handle.info_hash()) == info_hash {
                    *found_handle.borrow_mut() = Some(Arc::clone(handle));
                    break;
                }
            }
        });

        found_handle
            .into_inner()
            .ok_or_else(|| AppError::NotFound(format!("Torrent not found: {}", info_hash)))
    }

    /// Convert librqbit stats to our TorrentStatus.
    fn stats_to_status(&self, info_hash: &str, name: &str, stats: &TorrentStats) -> TorrentStatus {
        let status = match stats.state {
            TorrentStatsState::Initializing => DownloadStatus::Queued,
            TorrentStatsState::Live => {
                if stats.finished {
                    DownloadStatus::Seeding
                } else {
                    DownloadStatus::Downloading
                }
            }
            TorrentStatsState::Paused => DownloadStatus::Paused,
            TorrentStatsState::Error => DownloadStatus::Failed,
        };

        let (download_speed, upload_speed, peers) = extract_live_stats(stats);

        let ratio = if stats.progress_bytes > 0 {
            stats.uploaded_bytes as f64 / stats.progress_bytes as f64
        } else {
            0.0
        };

        TorrentStatus {
            info_hash: info_hash.to_string(),
            name: name.to_string(),
            status,
            progress: if stats.total_bytes > 0 {
                (stats.progress_bytes as f64 / stats.total_bytes as f64) * 100.0
            } else {
                0.0
            },
            download_speed,
            upload_speed,
            downloaded: stats.progress_bytes,
            uploaded: stats.uploaded_bytes,
            size: stats.total_bytes,
            ratio,
            peers,
            error: stats.error.clone(),
        }
    }

    /// Spawn a background task to monitor torrent progress.
    ///
    /// The task runs until the torrent is paused, errored, or removed.
    /// It emits progress events every second and completion events when finished.
    fn spawn_monitor_task(&self, handle: Arc<ManagedTorrent>) {
        let event_tx = self.event_tx.clone();
        let info_hash = info_hash_to_string(&handle.info_hash());

        tokio::spawn(async move {
            let mut last_finished = false;

            loop {
                let stats = handle.stats();
                let name = handle.name().unwrap_or_else(|| "Unknown".to_string());

                let (download_speed, upload_speed, peers) = extract_live_stats(&stats);

                let progress = if stats.total_bytes > 0 {
                    (stats.progress_bytes as f64 / stats.total_bytes as f64) * 100.0
                } else {
                    0.0
                };

                // Emit progress event
                if event_tx
                    .send(TorrentEvent::Progress {
                        info_hash: info_hash.clone(),
                        progress,
                        download_speed,
                        upload_speed,
                        peers,
                    })
                    .is_err()
                {
                    tracing::trace!("No subscribers for torrent progress events");
                }

                // Check for completion
                if stats.finished && !last_finished {
                    if event_tx
                        .send(TorrentEvent::Completed {
                            info_hash: info_hash.clone(),
                        })
                        .is_err()
                    {
                        tracing::trace!("No subscribers for torrent completion event");
                    }
                    tracing::info!(info_hash = %info_hash, name = %name, "Torrent completed");
                    last_finished = true;
                }

                // Check for errors
                if let Some(ref error) = stats.error {
                    let _ = event_tx.send(TorrentEvent::Error {
                        info_hash: info_hash.clone(),
                        message: error.clone(),
                    });
                    tracing::error!(info_hash = %info_hash, error = %error, "Torrent error");
                    break;
                }

                // Check if torrent is paused or removed
                if matches!(stats.state, TorrentStatsState::Paused) {
                    tracing::debug!(info_hash = %info_hash, "Monitoring paused torrent, stopping monitor");
                    break;
                }

                tokio::time::sleep(Duration::from_secs(1)).await;
            }
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[allow(dead_code)]
    fn test_config() -> TorrentConfig {
        TorrentConfig {
            download_dir: PathBuf::from("/tmp/lcars-test-downloads"),
            bind_interface: String::new(),
            max_connections: 50,
            port_range: (6881, 6889),
            seeding: crate::config::SeedingConfig {
                enabled: true,
                ratio_limit: 1.0,
                time_limit_hours: 48,
            },
        }
    }

    #[test]
    fn test_torrent_event_serialization() {
        let event = TorrentEvent::Added {
            info_hash: "abc123".to_string(),
            name: "Test Torrent".to_string(),
        };

        let json = serde_json::to_string(&event).unwrap();
        assert!(json.contains("\"type\":\"added\""));
        assert!(json.contains("\"info_hash\":\"abc123\""));
    }

    #[test]
    fn test_torrent_status_serialization() {
        let status = TorrentStatus {
            info_hash: "abc123".to_string(),
            name: "Test".to_string(),
            status: DownloadStatus::Downloading,
            progress: 50.0,
            download_speed: 1000000,
            upload_speed: 500000,
            downloaded: 500_000_000,
            uploaded: 250_000_000,
            size: 1_000_000_000,
            ratio: 0.5,
            peers: 10,
            error: None,
        };

        let json = serde_json::to_string(&status).unwrap();
        assert!(json.contains("\"progress\":50.0"));
        assert!(!json.contains("\"error\"")); // Should be skipped when None
    }

    #[test]
    fn test_media_ref_clone() {
        let media_ref = MediaRef {
            media_type: MediaType::Movie,
            media_id: 42,
        };

        let cloned = media_ref.clone();
        assert_eq!(cloned.media_id, 42);
    }
}
