//! YTS torrent indexer provider.
//!
//! Uses the YTS API for movie searches (JSON responses).

use async_trait::async_trait;
use reqwest::Client;
use serde::Deserialize;
use std::time::{Duration, Instant};

use crate::error::{AppError, Result};
use crate::services::indexer::{
    IndexerProvider, IndexerTestResult, Quality, Release, SearchQuery, Source,
};

const YTS_BASE_URL: &str = "https://yts.mx";
const YTS_API_URL: &str = "https://yts.mx/api/v2/list_movies.json";
const REQUEST_TIMEOUT_SECS: u64 = 30;

/// YTS torrent indexer provider (movies only).
pub struct YtsProvider {
    client: Client,
    base_url: String,
    api_url: String,
}

impl YtsProvider {
    /// Create a new YTS provider with default settings.
    pub fn new() -> Self {
        Self::with_urls(YTS_BASE_URL.to_string(), YTS_API_URL.to_string())
    }

    /// Create a new YTS provider with custom URLs.
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

    /// Build magnet link from torrent hash and movie name.
    fn build_magnet(hash: &str, name: &str) -> String {
        let encoded_name = urlencoding::encode(name);
        format!(
            "magnet:?xt=urn:btih:{}&dn={}&tr=udp://open.demonii.com:1337/announce&tr=udp://tracker.openbittorrent.com:80&tr=udp://tracker.coppersurfer.tk:6969&tr=udp://glotorrents.pw:6969/announce&tr=udp://tracker.opentrackr.org:1337/announce&tr=udp://torrent.gresille.org:80/announce&tr=udp://p4p.arenabg.com:1337&tr=udp://tracker.leechers-paradise.org:6969",
            hash, encoded_name
        )
    }

    /// Parse YTS quality string to Quality enum.
    fn parse_quality(quality: &str) -> Quality {
        match quality {
            "2160p" => Quality::P2160,
            "1080p" => Quality::P1080,
            "720p" => Quality::P720,
            "480p" => Quality::P480,
            _ => Quality::Unknown,
        }
    }
}

impl Default for YtsProvider {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl IndexerProvider for YtsProvider {
    fn name(&self) -> &str {
        "YTS"
    }

    fn supports_movies(&self) -> bool {
        true
    }

    fn supports_tv(&self) -> bool {
        false
    }

    fn supports_music(&self) -> bool {
        false
    }

    async fn search(&self, query: &SearchQuery) -> Result<Vec<Release>> {
        // Build URL with query parameters
        let mut url = format!("{}?limit=50", self.api_url);

        // Add search query
        if !query.query.is_empty() {
            url.push_str(&format!(
                "&query_term={}",
                urlencoding::encode(&query.query)
            ));
        }

        // Add year filter if specified
        if let Some(year) = query.year {
            // YTS doesn't have exact year filter, but we can use it as part of query
            url.push_str(&format!("&query_term={}", year));
        }

        // Sort by seeds for best results
        url.push_str("&sort_by=seeds");

        tracing::debug!(url = %url, "Searching YTS");

        let response = self
            .client
            .get(&url)
            .send()
            .await
            .map_err(|e| AppError::Internal(format!("YTS search request failed: {}", e)))?;

        if !response.status().is_success() {
            return Err(AppError::Internal(format!(
                "YTS returned status: {}",
                response.status()
            )));
        }

        let api_response: YtsApiResponse = response
            .json()
            .await
            .map_err(|e| AppError::Internal(format!("Failed to parse YTS response: {}", e)))?;

        // Check API status
        if api_response.status != "ok" {
            return Err(AppError::Internal(format!(
                "YTS API error: {}",
                api_response.status_message
            )));
        }

        let movies = api_response.data.movies.unwrap_or_default();

        // Flatten movies into releases (one movie can have multiple quality versions)
        let releases: Vec<Release> = movies
            .into_iter()
            .flat_map(|movie| {
                movie.torrents.into_iter().map(move |torrent| {
                    let title = format!(
                        "{}.{}.{}.{}.YTS",
                        movie.title.replace(' ', "."),
                        movie.year,
                        torrent.quality,
                        torrent.video_codec.as_deref().unwrap_or("x264")
                    );

                    let magnet = Self::build_magnet(&torrent.hash, &title);
                    let quality = Self::parse_quality(&torrent.quality);

                    Release {
                        id: Release::generate_id("YTS", &title, &magnet),
                        title,
                        indexer: "YTS".to_string(),
                        magnet,
                        size_bytes: torrent.size_bytes,
                        seeders: torrent.seeds,
                        leechers: torrent.peers,
                        quality,
                        source: Source::BluRay, // YTS primarily has BluRay sources
                        codec: torrent.video_codec.clone(),
                        audio: Some(torrent.audio_channels.clone()),
                        group: Some("YTS".to_string()),
                        proper: false,
                        repack: false,
                        uploaded_at: Some(torrent.date_uploaded.clone()),
                    }
                })
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
            .map_err(|e| AppError::Internal(format!("YTS test failed: {}", e)))?;

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

/// YTS API response structure.
#[derive(Debug, Deserialize)]
struct YtsApiResponse {
    status: String,
    status_message: String,
    data: YtsData,
}

/// YTS data wrapper.
#[derive(Debug, Deserialize)]
struct YtsData {
    #[allow(dead_code)]
    movie_count: Option<i32>,
    movies: Option<Vec<YtsMovie>>,
}

/// Individual movie from YTS API.
#[derive(Debug, Clone, Deserialize)]
struct YtsMovie {
    #[allow(dead_code)]
    id: i64,
    title: String,
    year: i32,
    #[allow(dead_code)]
    imdb_code: String,
    torrents: Vec<YtsTorrent>,
}

/// Individual torrent quality from YTS API.
#[derive(Debug, Clone, Deserialize)]
struct YtsTorrent {
    hash: String,
    quality: String,
    #[serde(rename = "type")]
    #[allow(dead_code)]
    torrent_type: String,
    video_codec: Option<String>,
    audio_channels: String,
    size_bytes: u64,
    seeds: u32,
    peers: u32,
    date_uploaded: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_yts_provider_capabilities() {
        let provider = YtsProvider::new();
        assert_eq!(provider.name(), "YTS");
        assert!(provider.supports_movies());
        assert!(!provider.supports_tv());
        assert!(!provider.supports_music());
    }

    #[test]
    fn test_parse_quality() {
        assert_eq!(YtsProvider::parse_quality("2160p"), Quality::P2160);
        assert_eq!(YtsProvider::parse_quality("1080p"), Quality::P1080);
        assert_eq!(YtsProvider::parse_quality("720p"), Quality::P720);
        assert_eq!(YtsProvider::parse_quality("480p"), Quality::P480);
        assert_eq!(YtsProvider::parse_quality("unknown"), Quality::Unknown);
    }

    #[test]
    fn test_build_magnet() {
        let magnet = YtsProvider::build_magnet("ABC123", "Test Movie");
        assert!(magnet.starts_with("magnet:?xt=urn:btih:ABC123"));
        assert!(magnet.contains("dn=Test%20Movie"));
    }
}
