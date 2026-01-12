//! LCARS Backend Library
//!
//! Core functionality for the LCARS media collection manager backend.
//! This library exposes modules for use in integration tests.

use axum::response::Json;
use rusqlite::Connection;
use serde::Serialize;
use std::sync::Arc;
use tokio::sync::Mutex;

pub mod api;
pub mod config;
pub mod db;
pub mod error;
pub mod middleware;
pub mod services;

use config::Config;
use services::{
    AuthService, IndexerManager, MusicBrainzClient, Scheduler, StorageManager, TmdbClient,
    TorrentEngine,
};

/// Application state shared across handlers
#[derive(Clone)]
pub struct AppState {
    pub config: Arc<Config>,
    pub db: Arc<Mutex<Connection>>,
    pub auth_service: Arc<AuthService>,
    pub tmdb_client: Option<Arc<TmdbClient>>,
    pub musicbrainz_client: Option<Arc<MusicBrainzClient>>,
    pub indexer_manager: Arc<IndexerManager>,
    pub torrent_engine: Option<Arc<TorrentEngine>>,
    pub scheduler: Option<Arc<Scheduler>>,
    pub start_time: std::time::Instant,
    pub storage_manager: Option<Arc<StorageManager>>,
}

impl AppState {
    /// Get a reference to the auth service.
    pub fn auth_service(&self) -> &AuthService {
        &self.auth_service
    }

    /// Get a reference to the TMDB client, if configured.
    pub fn tmdb_client(&self) -> Option<&TmdbClient> {
        self.tmdb_client.as_deref()
    }

    /// Get a reference to the MusicBrainz client, if configured.
    pub fn musicbrainz_client(&self) -> Option<&MusicBrainzClient> {
        self.musicbrainz_client.as_deref()
    }

    /// Get a reference to the indexer manager.
    pub fn indexer_manager(&self) -> &IndexerManager {
        &self.indexer_manager
    }

    /// Get a reference to the torrent engine, if initialized.
    pub fn torrent_engine(&self) -> Option<&TorrentEngine> {
        self.torrent_engine.as_deref()
    }

    /// Get a reference to the scheduler, if initialized.
    pub fn scheduler(&self) -> Option<&Scheduler> {
        self.scheduler.as_deref()
    }

    /// Get the start time of the application.
    pub fn start_time(&self) -> std::time::Instant {
        self.start_time
    }

    /// Get a reference to the storage manager, if initialized.
    pub fn storage_manager(&self) -> Option<&StorageManager> {
        self.storage_manager.as_deref()
    }

    /// Create a job context for manual job execution.
    pub fn job_context(&self) -> services::JobContext {
        services::JobContext {
            db: Arc::clone(&self.db),
            tmdb_client: self.tmdb_client.clone(),
            musicbrainz_client: self.musicbrainz_client.clone(),
            indexer_manager: Arc::clone(&self.indexer_manager),
            torrent_engine: self.torrent_engine.clone(),
        }
    }
}

#[derive(Serialize)]
pub struct ApiResponse {
    pub message: String,
    pub version: String,
}

pub async fn health_check() -> Json<ApiResponse> {
    Json(ApiResponse {
        message: "LCARS Backend is running".to_string(),
        version: env!("CARGO_PKG_VERSION").to_string(),
    })
}
