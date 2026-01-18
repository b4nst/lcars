//! Type definitions for Soulseek service.

use serde::Serialize;
use std::time::Instant;

/// State of an active search.
#[derive(Debug)]
pub struct SearchState {
    /// The search query string.
    pub query: String,
    /// Search ticket ID.
    pub ticket: u32,
    /// When the search was started.
    pub started_at: Instant,
    /// Accumulated results from all peers.
    pub results: Vec<SearchResult>,
    /// Whether the search is complete.
    pub complete: bool,
}

impl SearchState {
    /// Create a new search state.
    pub fn new(query: String, ticket: u32) -> Self {
        Self {
            query,
            ticket,
            started_at: Instant::now(),
            results: Vec::new(),
            complete: false,
        }
    }

    /// Add results from a peer.
    pub fn add_results(&mut self, result: SearchResult) {
        self.results.push(result);
    }

    /// Mark the search as complete.
    pub fn mark_complete(&mut self) {
        self.complete = true;
    }

    /// Get total number of files found.
    pub fn total_files(&self) -> usize {
        self.results.iter().map(|r| r.files.len()).sum()
    }
}

/// Search results from a single peer.
#[derive(Debug, Clone, Serialize)]
pub struct SearchResult {
    /// Username of the peer sharing the files.
    pub username: String,
    /// List of matching files.
    pub files: Vec<FileResult>,
    /// Whether the peer has free download slots.
    pub has_free_slot: bool,
    /// Average upload speed in bytes/second.
    pub average_speed: u32,
    /// Number of files in peer's queue.
    pub queue_length: u32,
}

/// A single file from search results.
#[derive(Debug, Clone, Serialize)]
pub struct FileResult {
    /// Full path of the file on the peer's system.
    pub filename: String,
    /// File size in bytes.
    pub size: u64,
    /// File extension (e.g., "mp3", "flac").
    pub extension: String,
    /// Bitrate in kbps (if available from attributes).
    pub bitrate: Option<u32>,
    /// Duration in seconds (if available from attributes).
    pub duration: Option<u32>,
    /// Sample rate in Hz (if available from attributes).
    pub sample_rate: Option<u32>,
    /// Bit depth (if available from attributes).
    pub bit_depth: Option<u32>,
}

impl FileResult {
    /// Create a FileResult from protocol file data.
    pub fn from_protocol_file(
        file: &soulseek_protocol::peers::p2p::shared_directories::File,
    ) -> Self {
        let filename = file.name.clone();
        let size = file.size;

        // Use the extension from the protocol or extract from filename
        let extension = if !file.extension.is_empty() {
            file.extension.to_lowercase()
        } else {
            filename.rsplit('.').next().unwrap_or("").to_lowercase()
        };

        // Extract attributes if present
        // Attribute place values: 0=bitrate, 1=duration, 4=sample_rate, 5=bit_depth
        let mut bitrate = None;
        let mut duration = None;
        let mut sample_rate = None;
        let mut bit_depth = None;

        for attr in &file.attributes {
            match attr.place {
                0 => bitrate = Some(attr.attribute),     // Bitrate
                1 => duration = Some(attr.attribute),    // Duration
                4 => sample_rate = Some(attr.attribute), // Sample rate
                5 => bit_depth = Some(attr.attribute),   // Bit depth
                _ => {}
            }
        }

        Self {
            filename,
            size,
            extension,
            bitrate,
            duration,
            sample_rate,
            bit_depth,
        }
    }
}

/// Statistics about the Soulseek engine.
#[derive(Debug, Clone, Serialize)]
pub struct SoulseekStats {
    /// Whether connected to the server.
    pub connected: bool,
    /// Number of active searches.
    pub active_searches: usize,
    /// Number of active downloads.
    pub active_downloads: usize,
    /// Number of completed downloads.
    pub completed_downloads: usize,
    /// Number of active uploads.
    pub active_uploads: usize,
    /// Number of queued uploads.
    pub queued_uploads: usize,
    /// Total bytes uploaded (lifetime).
    pub total_uploaded: u64,
    /// Number of shared files.
    pub shared_files: u64,
    /// Number of shared folders.
    pub shared_folders: u64,
}

/// Statistics about shared files (for API response).
#[derive(Debug, Clone, Serialize)]
pub struct ShareStatsResponse {
    /// Directories being shared.
    pub directories: Vec<String>,
    /// Total number of shared files.
    pub total_files: u64,
    /// Total number of shared folders.
    pub total_folders: u64,
    /// Total size of shared files in bytes.
    pub total_size: u64,
    /// When the index was last updated (ISO 8601 string).
    pub last_indexed: Option<String>,
    /// Whether sharing is enabled.
    pub sharing_enabled: bool,
}

/// Status of a Soulseek download.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DownloadStatus {
    /// Waiting for connection to peer.
    Connecting,
    /// Queued on the remote peer.
    Queued,
    /// Currently downloading.
    Downloading,
    /// Download completed successfully.
    Completed,
    /// Download failed.
    Failed,
    /// Download was cancelled.
    Cancelled,
}

impl std::fmt::Display for DownloadStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DownloadStatus::Connecting => write!(f, "connecting"),
            DownloadStatus::Queued => write!(f, "queued"),
            DownloadStatus::Downloading => write!(f, "downloading"),
            DownloadStatus::Completed => write!(f, "completed"),
            DownloadStatus::Failed => write!(f, "failed"),
            DownloadStatus::Cancelled => write!(f, "cancelled"),
        }
    }
}

/// State of a Soulseek download.
#[derive(Debug, Clone)]
pub struct DownloadState {
    /// Unique identifier for this download.
    pub id: String,
    /// Username of the peer sharing the file.
    pub username: String,
    /// Full path of the file on the remote peer.
    pub filename: String,
    /// File size in bytes.
    pub size: u64,
    /// Current download status.
    pub status: DownloadStatus,
    /// Bytes downloaded so far.
    pub downloaded: u64,
    /// Download speed in bytes/second.
    pub speed: u64,
    /// Position in the remote user's queue (if queued).
    pub queue_position: Option<u32>,
    /// Error message if download failed.
    pub error: Option<String>,
    /// Local path where the file will be saved.
    pub local_path: Option<std::path::PathBuf>,
    /// Optional media type link (track, album, etc.).
    pub media_type: Option<String>,
    /// Optional media ID to link to our library.
    pub media_id: Option<i64>,
    /// Token for the transfer.
    pub ticket: u32,
}

impl DownloadState {
    /// Create a new download state.
    pub fn new(id: String, username: String, filename: String, size: u64, ticket: u32) -> Self {
        Self {
            id,
            username,
            filename,
            size,
            status: DownloadStatus::Connecting,
            downloaded: 0,
            speed: 0,
            queue_position: None,
            error: None,
            local_path: None,
            media_type: None,
            media_id: None,
            ticket,
        }
    }

    /// Update progress.
    pub fn update_progress(&mut self, downloaded: u64, speed: u64) {
        self.downloaded = downloaded;
        self.speed = speed;
        self.status = DownloadStatus::Downloading;
    }

    /// Mark as queued with position.
    pub fn mark_queued(&mut self, position: u32) {
        self.status = DownloadStatus::Queued;
        self.queue_position = Some(position);
    }

    /// Mark as completed.
    pub fn mark_completed(&mut self, local_path: std::path::PathBuf) {
        self.status = DownloadStatus::Completed;
        self.downloaded = self.size;
        self.local_path = Some(local_path);
    }

    /// Mark as failed.
    pub fn mark_failed(&mut self, error: String) {
        self.status = DownloadStatus::Failed;
        self.error = Some(error);
    }

    /// Progress percentage (0-100).
    pub fn progress_percent(&self) -> u8 {
        if self.size == 0 {
            return 0;
        }
        ((self.downloaded * 100) / self.size).min(100) as u8
    }
}

/// Request to initiate a download.
#[derive(Debug, Clone)]
pub struct DownloadRequest {
    /// Username of the peer sharing the file.
    pub username: String,
    /// Full path of the file on the remote peer.
    pub filename: String,
    /// File size in bytes.
    pub size: u64,
    /// Optional media type (track, album, episode).
    pub media_type: Option<String>,
    /// Optional media ID to link to our library.
    pub media_id: Option<i64>,
}

/// Information about a browsed directory.
#[derive(Debug, Clone, Serialize)]
pub struct BrowsedDirectory {
    /// Path of the directory.
    pub path: String,
    /// Number of files in the directory.
    pub file_count: usize,
    /// Files in this directory.
    pub files: Vec<BrowsedFile>,
}

/// A file from a browsed directory.
#[derive(Debug, Clone, Serialize)]
pub struct BrowsedFile {
    /// Filename (without full path).
    pub name: String,
    /// Full path on the remote system.
    pub full_path: String,
    /// File size in bytes.
    pub size: u64,
    /// File extension.
    pub extension: String,
    /// Bitrate (if available).
    pub bitrate: Option<u32>,
    /// Duration in seconds (if available).
    pub duration: Option<u32>,
}
