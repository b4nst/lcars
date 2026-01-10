//! Rutracker torrent indexer provider.
//!
//! Scrapes Rutracker for music releases (primarily FLAC/lossless).

use async_trait::async_trait;
use reqwest::Client;
use scraper::{Html, Selector};
use std::time::{Duration, Instant};

use crate::error::{AppError, Result};
use crate::services::indexer::{
    parse_music_release, IndexerProvider, IndexerTestResult, Quality, Release, SearchQuery, Source,
};

const RUTRACKER_BASE_URL: &str = "https://rutracker.org";
const REQUEST_TIMEOUT_SECS: u64 = 30;

/// Rutracker torrent indexer provider (music focus).
pub struct RutrackerProvider {
    client: Client,
    base_url: String,
}

impl RutrackerProvider {
    /// Create a new Rutracker provider with default settings.
    pub fn new() -> Self {
        Self::with_base_url(RUTRACKER_BASE_URL.to_string())
    }

    /// Create a new Rutracker provider with a custom base URL.
    pub fn with_base_url(base_url: String) -> Self {
        let client = Client::builder()
            .timeout(Duration::from_secs(REQUEST_TIMEOUT_SECS))
            .user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36")
            .build()
            .expect("Failed to create HTTP client");

        Self { client, base_url }
    }

    /// Parse the search results page and extract release info.
    fn parse_search_results(&self, html: &str) -> Vec<PartialRelease> {
        let document = Html::parse_document(html);
        let mut releases = Vec::new();

        // Rutracker search results table
        let row_selector = Selector::parse("table#tor-tbl tbody tr").unwrap_or_else(|_| {
            // Fallback selector
            Selector::parse("tr.tCenter").unwrap()
        });
        let title_selector = Selector::parse("td.t-title-col a.tLink")
            .unwrap_or_else(|_| Selector::parse("a.tLink").unwrap());
        let size_selector =
            Selector::parse("td.tor-size").unwrap_or_else(|_| Selector::parse("a.tr-dl").unwrap());
        let seeds_selector = Selector::parse("td.seedmed, td b.seedmed").unwrap();
        let leechers_selector = Selector::parse("td.leechmed, td b.leechmed").unwrap();

        for row in document.select(&row_selector) {
            // Get title and link
            if let Some(title_elem) = row.select(&title_selector).next() {
                let title = title_elem.text().collect::<String>().trim().to_string();
                let href = title_elem.value().attr("href").unwrap_or("");

                // Extract topic ID from href
                let topic_id = extract_topic_id(href);

                // Get size
                let size_text = row
                    .select(&size_selector)
                    .next()
                    .map(|e| e.text().collect::<String>())
                    .unwrap_or_default();
                let size_bytes = parse_size(&size_text);

                // Get seeders
                let seeders = row
                    .select(&seeds_selector)
                    .next()
                    .map(|e| e.text().collect::<String>().trim().parse().unwrap_or(0))
                    .unwrap_or(0);

                // Get leechers
                let leechers = row
                    .select(&leechers_selector)
                    .next()
                    .map(|e| e.text().collect::<String>().trim().parse().unwrap_or(0))
                    .unwrap_or(0);

                if let Some(id) = topic_id {
                    if !title.is_empty() {
                        releases.push(PartialRelease {
                            title,
                            topic_id: id,
                            seeders,
                            leechers,
                            size_bytes,
                        });
                    }
                }
            }
        }

        releases
    }

    /// Build a magnet link using the topic ID.
    /// Note: Rutracker requires authentication for actual magnet links,
    /// so we generate a placeholder that could be resolved with auth.
    fn build_placeholder_magnet(topic_id: &str, title: &str) -> String {
        // Generate a fake hash based on topic ID for identification
        let hash = format!("RUTRACKER{:0>32}", topic_id);
        let encoded_title = urlencoding::encode(title);
        format!(
            "magnet:?xt=urn:btih:{}&dn={}&tr=http://bt.t-ru.org/ann?magnet",
            hash, encoded_title
        )
    }
}

impl Default for RutrackerProvider {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl IndexerProvider for RutrackerProvider {
    fn name(&self) -> &str {
        "Rutracker"
    }

    fn supports_movies(&self) -> bool {
        false // While Rutracker has movies, we focus on music
    }

    fn supports_tv(&self) -> bool {
        false
    }

    fn supports_music(&self) -> bool {
        true
    }

    async fn search(&self, query: &SearchQuery) -> Result<Vec<Release>> {
        // Build search query for music
        let search_query = if let (Some(artist), Some(album)) = (&query.artist, &query.album) {
            format!("{} {}", artist, album)
        } else {
            query.query.clone()
        };

        let encoded_query = urlencoding::encode(&search_query);

        // Rutracker music category is 409 (Losless) and 410 (Lossy)
        // We'll search in the FLAC/Lossless category
        let url = format!(
            "{}/forum/tracker.php?nm={}&f=409",
            self.base_url, encoded_query
        );

        tracing::debug!(url = %url, "Searching Rutracker");

        let response = self.client.get(&url).send().await;

        // Rutracker might be inaccessible, handle gracefully
        let response = match response {
            Ok(r) => r,
            Err(e) => {
                tracing::warn!(error = %e, "Rutracker is inaccessible");
                return Ok(Vec::new());
            }
        };

        if !response.status().is_success() {
            tracing::warn!(
                status = %response.status(),
                "Rutracker returned non-success status"
            );
            return Ok(Vec::new());
        }

        let html = response
            .text()
            .await
            .map_err(|e| AppError::Internal(format!("Failed to read Rutracker response: {}", e)))?;

        let partial_releases = self.parse_search_results(&html);

        // Convert partial releases to full releases
        let releases: Vec<Release> = partial_releases
            .into_iter()
            .take(30) // Limit results
            .map(|partial| {
                let parsed = parse_music_release(&partial.title);
                let magnet = Self::build_placeholder_magnet(&partial.topic_id, &partial.title);

                Release {
                    id: Release::generate_id(self.name(), &partial.title, &magnet),
                    title: partial.title,
                    indexer: self.name().to_string(),
                    magnet,
                    size_bytes: partial.size_bytes,
                    seeders: partial.seeders,
                    leechers: partial.leechers,
                    quality: Quality::Unknown, // Music doesn't use video quality
                    source: Source::Unknown,
                    codec: parsed.audio_format.map(|f| format!("{:?}", f)),
                    audio: parsed.audio_format.map(|f| format!("{:?}", f)),
                    group: parsed.group,
                    proper: false,
                    repack: false,
                    uploaded_at: None,
                }
            })
            .collect();

        Ok(releases)
    }

    async fn test(&self) -> Result<IndexerTestResult> {
        let start = Instant::now();

        let result = self.client.get(&self.base_url).send().await;

        let elapsed = start.elapsed().as_millis() as u64;

        match result {
            Ok(response) => {
                let success = response.status().is_success();
                Ok(IndexerTestResult {
                    name: self.name().to_string(),
                    success,
                    response_time_ms: elapsed,
                    error: if success {
                        None
                    } else {
                        Some(format!("HTTP status: {}", response.status()))
                    },
                })
            }
            Err(e) => Ok(IndexerTestResult {
                name: self.name().to_string(),
                success: false,
                response_time_ms: elapsed,
                error: Some(format!("Connection failed: {}", e)),
            }),
        }
    }
}

/// Partial release info before building full release.
struct PartialRelease {
    title: String,
    topic_id: String,
    seeders: u32,
    leechers: u32,
    size_bytes: u64,
}

/// Extract topic ID from Rutracker URL.
fn extract_topic_id(href: &str) -> Option<String> {
    // URL format: viewtopic.php?t=1234567 or /forum/viewtopic.php?t=1234567
    if let Some(pos) = href.find("t=") {
        let start = pos + 2;
        let end = href[start..]
            .find(|c: char| !c.is_ascii_digit())
            .map(|p| start + p)
            .unwrap_or(href.len());
        let id = &href[start..end];
        if !id.is_empty() {
            return Some(id.to_string());
        }
    }
    None
}

/// Parse a human-readable size string to bytes.
fn parse_size(size_str: &str) -> u64 {
    let clean = size_str
        .trim()
        .to_uppercase()
        .replace('\u{a0}', " ") // Replace non-breaking space
        .replace(',', ".");

    let parts: Vec<&str> = clean.split_whitespace().collect();
    if parts.is_empty() {
        return 0;
    }

    let (num_str, unit) = if parts.len() >= 2 {
        (parts[0], parts[1])
    } else {
        let s = parts[0];
        let pos = s.find(|c: char| c.is_alphabetic()).unwrap_or(s.len());
        (&s[..pos], &s[pos..])
    };

    let num: f64 = num_str.parse().unwrap_or(0.0);

    // Handle both English and Russian abbreviations
    let multiplier: u64 = match unit {
        "B" | "Б" => 1,
        "KB" | "КБ" => 1024,
        "MB" | "МБ" => 1024 * 1024,
        "GB" | "ГБ" => 1024 * 1024 * 1024,
        "TB" | "ТБ" => 1024 * 1024 * 1024 * 1024,
        _ => 1,
    };

    (num * multiplier as f64) as u64
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rutracker_provider_capabilities() {
        let provider = RutrackerProvider::new();
        assert_eq!(provider.name(), "Rutracker");
        assert!(!provider.supports_movies());
        assert!(!provider.supports_tv());
        assert!(provider.supports_music());
    }

    #[test]
    fn test_extract_topic_id() {
        assert_eq!(
            extract_topic_id("viewtopic.php?t=1234567"),
            Some("1234567".to_string())
        );
        assert_eq!(
            extract_topic_id("/forum/viewtopic.php?t=999&other=param"),
            Some("999".to_string())
        );
        assert_eq!(extract_topic_id("invalid"), None);
    }

    #[test]
    fn test_parse_size_russian() {
        // Russian units
        assert!(parse_size("1.5 ГБ") > 1_000_000_000);
        assert!(parse_size("500 МБ") > 400_000_000);
    }

    #[test]
    fn test_placeholder_magnet() {
        let magnet = RutrackerProvider::build_placeholder_magnet("12345", "Test Album FLAC");
        assert!(magnet.starts_with("magnet:?xt=urn:btih:"));
        assert!(magnet.contains("dn=Test%20Album%20FLAC"));
    }
}
