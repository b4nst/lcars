//! Torrent indexer service for searching multiple torrent providers.
//!
//! Provides a unified interface for searching torrents across multiple indexer sites
//! and aggregating results.

pub mod parser;
pub mod providers;

use async_trait::async_trait;
use futures::future::join_all;
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::sync::Arc;

use crate::error::Result;
pub use parser::{parse_music_release, parse_release_name, Quality, Source};
use providers::{EztvProvider, LeetxProvider, RutrackerProvider, YtsProvider};

/// Type of media to search for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MediaSearchType {
    Movie,
    TvEpisode,
    MusicAlbum,
}

/// Search query parameters for indexer searches.
#[derive(Debug, Clone, Default)]
pub struct SearchQuery {
    /// Main search query string
    pub query: String,
    /// Type of media to search for
    pub media_type: Option<MediaSearchType>,
    /// IMDB ID for precise matching
    pub imdb_id: Option<String>,
    /// TMDB ID for movies/TV
    pub tmdb_id: Option<i32>,
    /// MusicBrainz ID for music
    pub mbid: Option<String>,
    /// Release year filter
    pub year: Option<i32>,
    /// Season number (for TV)
    pub season: Option<i32>,
    /// Episode number (for TV)
    pub episode: Option<i32>,
    /// Artist name (for music)
    pub artist: Option<String>,
    /// Album name (for music)
    pub album: Option<String>,
}

impl SearchQuery {
    /// Create a new search query with the given query string.
    pub fn new(query: impl Into<String>) -> Self {
        Self {
            query: query.into(),
            ..Default::default()
        }
    }

    /// Set the media type for this search.
    pub fn media_type(mut self, media_type: MediaSearchType) -> Self {
        self.media_type = Some(media_type);
        self
    }

    /// Set the year filter.
    pub fn year(mut self, year: i32) -> Self {
        self.year = Some(year);
        self
    }

    /// Set season and episode for TV searches.
    pub fn episode(mut self, season: i32, episode: i32) -> Self {
        self.season = Some(season);
        self.episode = Some(episode);
        self
    }

    /// Set IMDB ID for precise matching.
    pub fn imdb_id(mut self, imdb_id: impl Into<String>) -> Self {
        self.imdb_id = Some(imdb_id.into());
        self
    }

    /// Build a search query string suitable for indexers.
    pub fn build_query_string(&self) -> String {
        let mut parts = vec![self.query.clone()];

        if let Some(year) = self.year {
            parts.push(year.to_string());
        }

        if let (Some(season), Some(episode)) = (self.season, self.episode) {
            parts.push(format!("S{:02}E{:02}", season, episode));
        }

        parts.join(" ")
    }
}

/// Result of testing an indexer provider.
#[derive(Debug, Clone, Serialize)]
pub struct IndexerTestResult {
    /// Name of the indexer
    pub name: String,
    /// Whether the test was successful
    pub success: bool,
    /// Response time in milliseconds
    pub response_time_ms: u64,
    /// Error message if test failed
    pub error: Option<String>,
}

/// A release/torrent result from an indexer.
#[derive(Debug, Clone, Serialize)]
pub struct Release {
    /// Unique identifier (hash of indexer + title + magnet)
    pub id: String,
    /// Original release title
    pub title: String,
    /// Name of the indexer this came from
    pub indexer: String,
    /// Magnet link or torrent URL
    pub magnet: String,
    /// File size in bytes
    pub size_bytes: u64,
    /// Number of seeders
    pub seeders: u32,
    /// Number of leechers
    pub leechers: u32,
    /// Video quality
    pub quality: Quality,
    /// Video source
    pub source: Source,
    /// Video codec
    pub codec: Option<String>,
    /// Audio codec
    pub audio: Option<String>,
    /// Release group
    pub group: Option<String>,
    /// Is this a PROPER release
    pub proper: bool,
    /// Is this a REPACK release
    pub repack: bool,
    /// Upload timestamp (ISO 8601)
    pub uploaded_at: Option<String>,
}

impl Release {
    /// Generate a unique ID for this release based on indexer, title, and magnet.
    pub fn generate_id(indexer: &str, title: &str, magnet: &str) -> String {
        let mut hasher = Sha256::new();
        hasher.update(indexer.as_bytes());
        hasher.update(title.as_bytes());
        hasher.update(magnet.as_bytes());
        let result = hasher.finalize();
        hex::encode(&result[..8]) // Use first 8 bytes for shorter ID
    }

    /// Calculate a quality score for sorting (higher is better).
    pub fn score(&self) -> u32 {
        let quality_score = self.quality.score() * 100;
        let source_score = self.source.score() * 20;
        let seeder_score = self.seeders.min(100) * 2;
        let proper_bonus = if self.proper { 10 } else { 0 };

        quality_score + source_score + seeder_score + proper_bonus
    }
}

/// Trait for implementing torrent indexer providers.
#[async_trait]
pub trait IndexerProvider: Send + Sync {
    /// Get the name of this indexer.
    fn name(&self) -> &str;

    /// Check if this indexer supports movie searches.
    fn supports_movies(&self) -> bool;

    /// Check if this indexer supports TV show searches.
    fn supports_tv(&self) -> bool;

    /// Check if this indexer supports music searches.
    fn supports_music(&self) -> bool;

    /// Search for releases matching the given query.
    async fn search(&self, query: &SearchQuery) -> Result<Vec<Release>>;

    /// Test the indexer connection and functionality.
    async fn test(&self) -> Result<IndexerTestResult>;
}

/// Manager for coordinating searches across multiple indexer providers.
pub struct IndexerManager {
    providers: Vec<Arc<dyn IndexerProvider>>,
}

impl IndexerManager {
    /// Create a new indexer manager with default providers.
    pub fn new() -> Self {
        Self {
            providers: vec![
                Arc::new(LeetxProvider::new()),
                Arc::new(EztvProvider::new()),
                Arc::new(YtsProvider::new()),
                Arc::new(RutrackerProvider::new()),
            ],
        }
    }

    /// Create an indexer manager with custom providers.
    pub fn with_providers(providers: Vec<Arc<dyn IndexerProvider>>) -> Self {
        Self { providers }
    }

    /// Create an indexer manager wrapped in Arc for shared access.
    pub fn new_shared() -> Arc<Self> {
        Arc::new(Self::new())
    }

    /// Get all registered providers.
    pub fn providers(&self) -> &[Arc<dyn IndexerProvider>] {
        &self.providers
    }

    /// Search all appropriate providers for releases matching the query.
    ///
    /// Results are aggregated, deduplicated, and sorted by quality/seeders.
    pub async fn search(&self, query: &SearchQuery) -> Result<Vec<Release>> {
        // Filter providers based on media type
        let suitable_providers: Vec<_> = self
            .providers
            .iter()
            .filter(|p| match query.media_type {
                Some(MediaSearchType::Movie) => p.supports_movies(),
                Some(MediaSearchType::TvEpisode) => p.supports_tv(),
                Some(MediaSearchType::MusicAlbum) => p.supports_music(),
                None => true, // Search all if no type specified
            })
            .collect();

        // Search all providers in parallel
        let search_futures: Vec<_> = suitable_providers
            .iter()
            .map(|provider| {
                let provider = Arc::clone(provider);
                let query = query.clone();
                async move {
                    match provider.search(&query).await {
                        Ok(results) => results,
                        Err(e) => {
                            tracing::warn!(
                                indexer = %provider.name(),
                                error = %e,
                                "Indexer search failed"
                            );
                            Vec::new()
                        }
                    }
                }
            })
            .collect();

        let all_results: Vec<Vec<Release>> = join_all(search_futures).await;

        // Flatten and deduplicate results
        let mut releases: Vec<Release> = all_results.into_iter().flatten().collect();

        // Deduplicate by magnet link (keep the one with more seeders)
        let mut seen: HashMap<String, usize> = HashMap::new();
        let mut unique_releases: Vec<Release> = Vec::new();

        for release in releases.drain(..) {
            let magnet_hash = &release.magnet[..release.magnet.len().min(60)];
            if let Some(&idx) = seen.get(magnet_hash) {
                // Keep the one with more seeders
                if release.seeders > unique_releases[idx].seeders {
                    unique_releases[idx] = release;
                }
            } else {
                seen.insert(magnet_hash.to_string(), unique_releases.len());
                unique_releases.push(release);
            }
        }

        // Sort by score (quality + seeders)
        unique_releases.sort_by_key(|r| std::cmp::Reverse(r.score()));

        Ok(unique_releases)
    }

    /// Test all providers and return their status.
    pub async fn test_all(&self) -> Vec<IndexerTestResult> {
        let test_futures: Vec<_> = self
            .providers
            .iter()
            .map(|provider| {
                let provider = Arc::clone(provider);
                async move {
                    match provider.test().await {
                        Ok(result) => result,
                        Err(e) => IndexerTestResult {
                            name: provider.name().to_string(),
                            success: false,
                            response_time_ms: 0,
                            error: Some(e.to_string()),
                        },
                    }
                }
            })
            .collect();

        join_all(test_futures).await
    }
}

impl Default for IndexerManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_search_query_builder() {
        let query = SearchQuery::new("Breaking Bad")
            .media_type(MediaSearchType::TvEpisode)
            .year(2008)
            .episode(1, 1);

        assert_eq!(query.query, "Breaking Bad");
        assert_eq!(query.year, Some(2008));
        assert_eq!(query.season, Some(1));
        assert_eq!(query.episode, Some(1));
        assert_eq!(query.build_query_string(), "Breaking Bad 2008 S01E01");
    }

    #[test]
    fn test_release_id_generation() {
        let id1 = Release::generate_id("1337x", "Movie.2024.1080p", "magnet:?xt=urn:btih:abc");
        let id2 = Release::generate_id("1337x", "Movie.2024.1080p", "magnet:?xt=urn:btih:abc");
        let id3 = Release::generate_id("eztv", "Movie.2024.1080p", "magnet:?xt=urn:btih:abc");

        assert_eq!(id1, id2);
        assert_ne!(id1, id3);
    }

    #[test]
    fn test_release_scoring() {
        let high_quality = Release {
            id: "1".to_string(),
            title: "test".to_string(),
            indexer: "test".to_string(),
            magnet: "magnet:test".to_string(),
            size_bytes: 0,
            seeders: 100,
            leechers: 10,
            quality: Quality::P2160,
            source: Source::BluRay,
            codec: None,
            audio: None,
            group: None,
            proper: false,
            repack: false,
            uploaded_at: None,
        };

        let low_quality = Release {
            quality: Quality::P720,
            source: Source::Cam,
            seeders: 10,
            ..high_quality.clone()
        };

        assert!(high_quality.score() > low_quality.score());
    }
}
