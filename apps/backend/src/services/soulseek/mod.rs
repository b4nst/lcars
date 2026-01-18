//! Soulseek client service.
//!
//! Provides Soulseek network functionality for searching and downloading music.
//! Maintains a persistent connection to the Soulseek server and handles P2P
//! connections for file transfers.

mod connection;
mod engine;
mod events;
mod listener;
mod peer;
mod shares;
mod types;
mod uploads;

pub use engine::SoulseekEngine;
pub use events::SoulseekEvent;
pub use peer::{BrowseResult, PeerConnection};
pub use shares::{FileAttributes, ShareIndex, ShareStats, SharedFile};
pub use types::{
    BrowsedDirectory, BrowsedFile, DownloadRequest, DownloadState, DownloadStatus, FileResult,
    SearchResult, SearchState, ShareStatsResponse, SoulseekStats,
};
pub use uploads::{UploadQueue, UploadState, UploadStatus};
