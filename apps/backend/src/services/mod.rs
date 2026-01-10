//! Application services for the LCARS backend.

pub mod auth;
pub mod indexer;
pub mod musicbrainz;
pub mod storage;
pub mod tmdb;
pub mod torrent;

pub use auth::{AuthService, Claims};
pub use indexer::IndexerManager;
pub use musicbrainz::MusicBrainzClient;
#[allow(unused_imports)]
pub use storage::{LocalMount, MediaInfo, Mount, NamingEngine, ProcessedFile, StorageManager};
pub use tmdb::TmdbClient;
pub use torrent::TorrentEngine;
