//! Soulseek event types for progress tracking and notifications.

use serde::Serialize;
use std::path::PathBuf;

use super::types::FileResult;

/// Event emitted by the Soulseek engine for status updates and progress tracking.
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum SoulseekEvent {
    // =========================================================================
    // Connection events
    // =========================================================================
    /// Successfully connected and logged in to the Soulseek server.
    Connected {
        /// The username that was used to log in.
        username: String,
    },

    /// Disconnected from the Soulseek server.
    Disconnected {
        /// Reason for disconnection.
        reason: String,
    },

    /// Attempting to reconnect to the server.
    Reconnecting {
        /// Current reconnection attempt number.
        attempt: u32,
        /// Seconds until next retry.
        next_retry_secs: u64,
    },

    /// Connection failed permanently (will not retry).
    ConnectionFailed {
        /// Error describing why the connection failed.
        error: String,
    },

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

    // =========================================================================
    // Sharing events
    // =========================================================================
    /// Share index has been updated.
    ShareIndexUpdated { files: u64, folders: u64 },

    /// An upload has been queued.
    UploadQueued {
        id: String,
        username: String,
        filename: String,
    },

    /// An upload has started transferring.
    UploadStarted { id: String },

    /// Upload progress update.
    UploadProgress {
        id: String,
        progress: u64,
        total: u64,
        speed: u64,
    },

    /// Upload completed successfully.
    UploadComplete { id: String },

    /// Upload failed with an error.
    UploadFailed { id: String, error: String },

    /// A peer browsed our shared files.
    PeerBrowsed { username: String },

    /// A peer searched our shared files.
    PeerSearched {
        username: String,
        query: String,
        results: usize,
    },
}
