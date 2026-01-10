//! EZTV torrent indexer provider.
//!
//! Uses the EZTV API for TV show searches (JSON responses).

use async_trait::async_trait;
use reqwest::Client;
use serde::Deserialize;
use std::time::{Duration, Instant};

use crate::error::{AppError, Result};
use crate::services::indexer::{
    parse_release_name, IndexerProvider, IndexerTestResult, Release, SearchQuery,
};

const EZTV_BASE_URL: &str = "https://eztv.re";
const EZTV_API_URL: &str = "https://eztv.re/api/get-torrents";
const REQUEST_TIMEOUT_SECS: u64 = 30;

/// EZTV torrent indexer provider (TV shows only).
pub struct EztvProvider {
    client: Client,
    base_url: String,
    api_url: String,
}

impl EztvProvider {
    /// Create a new EZTV provider with default settings.
    pub fn new() -> Self {
        Self::with_urls(EZTV_BASE_URL.to_string(), EZTV_API_URL.to_string())
    }

    /// Create a new EZTV provider with custom URLs.
    pub fn with_urls(base_url: String, api_url: String) -> Self {
        let client = Client::builder()
            .timeout(Duration::from_secs(REQUEST_TIMEOUT_SECS))
            .user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36")
            .build()
            .expect("Failed to create HTTP client");

        Self {
            client,
            base_url,
            api_url,
        }
    }
}

impl Default for EztvProvider {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl IndexerProvider for EztvProvider {
    fn name(&self) -> &str {
        "EZTV"
    }

    fn supports_movies(&self) -> bool {
        false
    }

    fn supports_tv(&self) -> bool {
        true
    }

    fn supports_music(&self) -> bool {
        false
    }

    async fn search(&self, query: &SearchQuery) -> Result<Vec<Release>> {
        // Build search parameters
        let search_query = query.build_query_string();

        // EZTV API uses IMDB ID preferentially
        let url = if let Some(ref imdb_id) = query.imdb_id {
            // Strip "tt" prefix if present
            let imdb_num = imdb_id.trim_start_matches("tt");
            format!("{}?imdb_id={}&limit=50", self.api_url, imdb_num)
        } else {
            // Fall back to text search
            let encoded = urlencoding::encode(&search_query);
            format!("{}?limit=50&page=1&query={}", self.api_url, encoded)
        };

        tracing::debug!(url = %url, "Searching EZTV");

        let response = self
            .client
            .get(&url)
            .send()
            .await
            .map_err(|e| AppError::Internal(format!("EZTV search request failed: {}", e)))?;

        if !response.status().is_success() {
            return Err(AppError::Internal(format!(
                "EZTV returned status: {}",
                response.status()
            )));
        }

        let api_response: EztvApiResponse = response
            .json()
            .await
            .map_err(|e| AppError::Internal(format!("Failed to parse EZTV response: {}", e)))?;

        let releases = api_response
            .torrents
            .unwrap_or_default()
            .into_iter()
            .filter(|t| {
                // Filter by season/episode if specified
                if let (Some(season), Some(episode)) = (query.season, query.episode) {
                    t.season.map(|s| s == season).unwrap_or(false)
                        && t.episode.map(|e| e == episode).unwrap_or(false)
                } else {
                    true
                }
            })
            .map(|torrent| {
                let parsed = parse_release_name(&torrent.title);

                Release {
                    id: Release::generate_id(self.name(), &torrent.title, &torrent.magnet_url),
                    title: torrent.title,
                    indexer: self.name().to_string(),
                    magnet: torrent.magnet_url,
                    size_bytes: torrent.size_bytes.parse().unwrap_or(0),
                    seeders: torrent.seeds,
                    leechers: torrent.peers.saturating_sub(torrent.seeds),
                    quality: parsed.quality,
                    source: parsed.source,
                    codec: parsed.codec,
                    audio: parsed.audio,
                    group: parsed.group,
                    proper: parsed.proper,
                    repack: parsed.repack,
                    uploaded_at: Some(torrent.date_released_unix.to_string()),
                }
            })
            .collect();

        Ok(releases)
    }

    async fn test(&self) -> Result<IndexerTestResult> {
        let start = Instant::now();

        let result = self
            .client
            .get(&self.base_url)
            .send()
            .await
            .map_err(|e| AppError::Internal(format!("EZTV test failed: {}", e)))?;

        let elapsed = start.elapsed().as_millis() as u64;
        let success = result.status().is_success();

        Ok(IndexerTestResult {
            name: self.name().to_string(),
            success,
            response_time_ms: elapsed,
            error: if success {
                None
            } else {
                Some(format!("HTTP status: {}", result.status()))
            },
        })
    }
}

/// EZTV API response structure.
#[derive(Debug, Deserialize)]
struct EztvApiResponse {
    #[allow(dead_code)]
    torrents_count: Option<i32>,
    torrents: Option<Vec<EztvTorrent>>,
}

/// Individual torrent from EZTV API.
#[derive(Debug, Deserialize)]
struct EztvTorrent {
    #[allow(dead_code)]
    id: i64,
    title: String,
    magnet_url: String,
    size_bytes: String,
    seeds: u32,
    peers: u32,
    date_released_unix: i64,
    season: Option<i32>,
    episode: Option<i32>,
    #[serde(default)]
    #[allow(dead_code)]
    imdb_id: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_eztv_provider_capabilities() {
        let provider = EztvProvider::new();
        assert_eq!(provider.name(), "EZTV");
        assert!(!provider.supports_movies());
        assert!(provider.supports_tv());
        assert!(!provider.supports_music());
    }
}
