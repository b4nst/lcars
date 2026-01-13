//! Soulseek event types for progress tracking and notifications.

use serde::Serialize;
use std::path::PathBuf;

use super::types::FileResult;

/// Event emitted by the Soulseek engine for status updates and progress tracking.
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum SoulseekEvent {
    /// Successfully connected to the Soulseek server.
    Connected,

    /// Disconnected from the Soulseek server.
    Disconnected { reason: String },

    /// Login attempt failed.
    LoginFailed { reason: String },

    /// Received search results from a peer.
    SearchResult {
        ticket: u32,
        username: String,
        files: Vec<FileResult>,
        has_free_slot: bool,
        average_speed: u32,
        queue_length: u32,
    },

    /// Search completed (no more results expected).
    SearchComplete { ticket: u32 },

    /// A download has been queued.
    DownloadQueued {
        id: String,
        username: String,
        filename: String,
    },

    /// A download has started transferring.
    DownloadStarted { id: String },

    /// Download progress update.
    DownloadProgress {
        id: String,
        progress: u64,
        total: u64,
        speed: u64,
    },

    /// Download completed successfully.
    DownloadComplete { id: String, path: PathBuf },

    /// Download failed with an error.
    DownloadFailed { id: String, error: String },
}
