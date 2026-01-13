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
}
