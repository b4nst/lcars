//! 1337x torrent indexer provider.
//!
//! Scrapes 1337x.to for torrent releases (movies, TV, music).

use async_trait::async_trait;
use reqwest::Client;
use scraper::{Html, Selector};
use std::time::{Duration, Instant};

use crate::error::{AppError, Result};
use crate::services::indexer::{
    parse_release_name, IndexerProvider, IndexerTestResult, Release, SearchQuery,
};

const LEETX_BASE_URL: &str = "https://1337x.to";
const REQUEST_TIMEOUT_SECS: u64 = 30;
const USER_AGENT: &str = concat!("LCARS/", env!("CARGO_PKG_VERSION"));

/// 1337x torrent indexer provider.
///
/// Scrapes the 1337x.to website for torrent releases. Supports movies, TV shows, and music.
/// Note: This provider makes multiple HTTP requests per search (one for the search page,
/// then one per result to fetch magnet links).
pub struct LeetxProvider {
    client: Client,
    base_url: String,
}

impl LeetxProvider {
    /// Create a new 1337x provider with default settings.
    pub fn new() -> Self {
        Self::with_base_url(LEETX_BASE_URL.to_string())
    }

    /// Create a new 1337x provider with a custom base URL.
    pub fn with_base_url(base_url: String) -> Self {
        // Client builder should not fail with these standard options
        let client = Client::builder()
            .timeout(Duration::from_secs(REQUEST_TIMEOUT_SECS))
            .user_agent(USER_AGENT)
            .build()
            .unwrap_or_else(|_| Client::new());

        Self { client, base_url }
    }

    /// Parse the search results page and extract release info.
    fn parse_search_results(&self, html: &str) -> Vec<PartialRelease> {
        let document = Html::parse_document(html);
        let mut releases = Vec::new();

        // Selector for result rows
        let row_selector = Selector::parse("table.table-list tbody tr").unwrap();
        let name_selector = Selector::parse("td.coll-1 a:nth-child(2)").unwrap();
        let seeds_selector = Selector::parse("td.coll-2").unwrap();
        let leechers_selector = Selector::parse("td.coll-3").unwrap();
        let size_selector = Selector::parse("td.coll-4").unwrap();

        for row in document.select(&row_selector) {
            // Get name and detail link
            if let Some(name_elem) = row.select(&name_selector).next() {
                let title = name_elem.text().collect::<String>().trim().to_string();
                let detail_path = name_elem.value().attr("href").unwrap_or("");

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

                // Get size
                let size_text = row
                    .select(&size_selector)
                    .next()
                    .map(|e| e.text().collect::<String>())
                    .unwrap_or_default();
                let size_bytes = parse_size(&size_text);

                if !title.is_empty() && !detail_path.is_empty() {
                    releases.push(PartialRelease {
                        title,
                        detail_url: format!("{}{}", self.base_url, detail_path),
                        seeders,
                        leechers,
                        size_bytes,
                    });
                }
            }
        }

        releases
    }

    /// Fetch magnet link from a detail page.
    async fn fetch_magnet(&self, detail_url: &str) -> Result<String> {
        let response = self
            .client
            .get(detail_url)
            .send()
            .await
            .map_err(|e| AppError::Internal(format!("Failed to fetch detail page: {}", e)))?;

        if !response.status().is_success() {
            return Err(AppError::Internal(format!(
                "Detail page returned status: {}",
                response.status()
            )));
        }

        let html = response
            .text()
            .await
            .map_err(|e| AppError::Internal(format!("Failed to read detail page: {}", e)))?;

        let document = Html::parse_document(&html);
        let magnet_selector = Selector::parse("a[href^='magnet:']").unwrap();

        document
            .select(&magnet_selector)
            .next()
            .and_then(|elem| elem.value().attr("href"))
            .map(|s| s.to_string())
            .ok_or_else(|| AppError::Internal("No magnet link found on detail page".to_string()))
    }
}

impl Default for LeetxProvider {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl IndexerProvider for LeetxProvider {
    fn name(&self) -> &str {
        "1337x"
    }

    fn supports_movies(&self) -> bool {
        true
    }

    fn supports_tv(&self) -> bool {
        true
    }

    fn supports_music(&self) -> bool {
        true
    }

    async fn search(&self, query: &SearchQuery) -> Result<Vec<Release>> {
        let search_query = query.build_query_string();
        let encoded_query = urlencoding::encode(&search_query);

        // Determine category path based on media type
        let category = match query.media_type {
            Some(crate::services::indexer::MediaSearchType::Movie) => "/category-search",
            Some(crate::services::indexer::MediaSearchType::TvEpisode) => "/category-search",
            Some(crate::services::indexer::MediaSearchType::MusicAlbum) => "/category-search",
            None => "/search",
        };

        let url = format!("{}{}/{}/1/", self.base_url, category, encoded_query);

        tracing::debug!(url = %url, "Searching 1337x");

        let response = self
            .client
            .get(&url)
            .send()
            .await
            .map_err(|e| AppError::Internal(format!("1337x search request failed: {}", e)))?;

        if !response.status().is_success() {
            return Err(AppError::Internal(format!(
                "1337x returned status: {}",
                response.status()
            )));
        }

        let html = response
            .text()
            .await
            .map_err(|e| AppError::Internal(format!("Failed to read 1337x response: {}", e)))?;

        let partial_releases = self.parse_search_results(&html);

        // Fetch magnet links for top results (limit to avoid too many requests)
        let mut releases = Vec::new();
        let max_results = 20;

        for partial in partial_releases.into_iter().take(max_results) {
            match self.fetch_magnet(&partial.detail_url).await {
                Ok(magnet) => {
                    let parsed = parse_release_name(&partial.title);

                    releases.push(Release {
                        id: Release::generate_id(self.name(), &partial.title, &magnet),
                        title: partial.title,
                        indexer: self.name().to_string(),
                        magnet,
                        size_bytes: partial.size_bytes,
                        seeders: partial.seeders,
                        leechers: partial.leechers,
                        quality: parsed.quality,
                        source: parsed.source,
                        codec: parsed.codec,
                        audio: parsed.audio,
                        group: parsed.group,
                        proper: parsed.proper,
                        repack: parsed.repack,
                        uploaded_at: None,
                    });
                }
                Err(e) => {
                    tracing::debug!(
                        title = %partial.title,
                        error = %e,
                        "Failed to fetch magnet for 1337x release"
                    );
                }
            }
        }

        Ok(releases)
    }

    async fn test(&self) -> Result<IndexerTestResult> {
        let start = Instant::now();

        let result = self
            .client
            .get(&self.base_url)
            .send()
            .await
            .map_err(|e| AppError::Internal(format!("1337x test failed: {}", e)))?;

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

/// Partial release info before fetching magnet.
struct PartialRelease {
    title: String,
    detail_url: String,
    seeders: u32,
    leechers: u32,
    size_bytes: u64,
}

/// Parse a human-readable size string to bytes.
fn parse_size(size_str: &str) -> u64 {
    let clean = size_str.trim().to_uppercase();

    // Try to extract number and unit
    let parts: Vec<&str> = clean.split_whitespace().collect();
    if parts.is_empty() {
        return 0;
    }

    // Handle formats like "1.5 GB" or "1.5GB"
    let (num_str, unit) = if parts.len() >= 2 {
        (parts[0], parts[1])
    } else {
        // Try to split number from unit
        let s = parts[0];
        let pos = s.find(|c: char| c.is_alphabetic()).unwrap_or(s.len());
        (&s[..pos], &s[pos..])
    };

    let num: f64 = num_str.parse().unwrap_or(0.0);

    let multiplier: u64 = match unit {
        "B" => 1,
        "KB" => 1024,
        "MB" => 1024 * 1024,
        "GB" => 1024 * 1024 * 1024,
        "TB" => 1024 * 1024 * 1024 * 1024,
        _ => 1,
    };

    (num * multiplier as f64) as u64
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_size() {
        assert_eq!(parse_size("1.5 GB"), 1610612736);
        assert_eq!(parse_size("500 MB"), 524288000);
        assert_eq!(parse_size("1 KB"), 1024);
        assert_eq!(parse_size("100 B"), 100);
    }

    #[test]
    fn test_parse_size_no_space() {
        assert_eq!(parse_size("1.5GB"), 1610612736);
        assert_eq!(parse_size("500MB"), 524288000);
    }
}
