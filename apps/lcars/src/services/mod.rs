//! Application services for the LCARS backend.

pub mod activity;
pub mod auth;
pub mod indexer;
pub mod musicbrainz;
pub mod scheduler;
pub mod soulseek;
pub mod storage;
pub mod tmdb;
pub mod torrent;
pub mod wireguard;

pub use auth::{AuthService, Claims};
pub use indexer::IndexerManager;
pub use musicbrainz::MusicBrainzClient;
pub use scheduler::{JobContext, Scheduler};
pub use soulseek::SoulseekEngine;
#[allow(unused_imports)]
pub use storage::{LocalMount, MediaInfo, Mount, NamingEngine, ProcessedFile, StorageManager};
pub use tmdb::TmdbClient;
pub use torrent::TorrentEngine;
pub use wireguard::WireGuardService;
