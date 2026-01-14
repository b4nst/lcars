//! Soulseek client service.
//!
//! Provides Soulseek network functionality for searching and downloading music.
//! Maintains a persistent connection to the Soulseek server and handles P2P
//! connections for file transfers.

mod connection;
mod engine;
mod events;
mod types;

pub use engine::SoulseekEngine;
pub use events::SoulseekEvent;
pub use types::{FileResult, SearchResult, SearchState, SoulseekStats};
