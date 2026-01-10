//! MusicBrainz service client.
//!
//! Provides methods to search and fetch music metadata from MusicBrainz API.
//! Includes rate limiting to comply with MusicBrainz's 1 request/second limit.

use reqwest::Client;
use serde::Deserialize;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::Mutex;

use crate::error::{AppError, Result};

const MB_BASE_URL: &str = "https://musicbrainz.org/ws/2";
const COVER_ART_BASE: &str = "https://coverartarchive.org";
const REQUEST_TIMEOUT_SECS: u64 = 30;

// =============================================================================
// Rate Limiter
// =============================================================================

/// Rate limiter to enforce MusicBrainz's 1 request per second limit.
struct RateLimiter {
    last_request: Mutex<Instant>,
    min_interval: Duration,
}

impl RateLimiter {
    /// Create a new rate limiter with the specified minimum interval between requests.
    pub fn new(min_interval: Duration) -> Self {
        Self {
            // Initialize to past so first request can proceed immediately
            last_request: Mutex::new(Instant::now() - min_interval),
            min_interval,
        }
    }

    /// Wait until the rate limit allows another request.
    pub async fn wait(&self) {
        let mut last = self.last_request.lock().await;
        let elapsed = last.elapsed();
        if elapsed < self.min_interval {
            tokio::time::sleep(self.min_interval - elapsed).await;
        }
        *last = Instant::now();
    }
}

// =============================================================================
// MusicBrainz Client
// =============================================================================

/// MusicBrainz API client for fetching music metadata.
pub struct MusicBrainzClient {
    client: Client,
    rate_limiter: RateLimiter,
}

impl MusicBrainzClient {
    /// Create a new MusicBrainz client.
    ///
    /// MusicBrainz requires a proper User-Agent header with application name,
    /// version, and contact information.
    ///
    /// # Arguments
    /// * `app_name` - Application name (e.g., "lcars")
    /// * `app_version` - Application version (e.g., "0.1.0")
    /// * `contact` - Contact email or URL
    /// * `rate_limit_ms` - Minimum interval between requests in milliseconds
    ///
    /// # Errors
    /// Returns an error if any of the required parameters are empty or if the
    /// rate limit is too high (max 60000ms).
    pub fn new(
        app_name: &str,
        app_version: &str,
        contact: &str,
        rate_limit_ms: u64,
    ) -> Result<Self> {
        if app_name.trim().is_empty() {
            return Err(AppError::Internal(
                "MusicBrainz app name cannot be empty".to_string(),
            ));
        }

        if app_version.trim().is_empty() {
            return Err(AppError::Internal(
                "MusicBrainz app version cannot be empty".to_string(),
            ));
        }

        if contact.trim().is_empty() {
            return Err(AppError::Internal(
                "MusicBrainz contact information cannot be empty".to_string(),
            ));
        }

        // MusicBrainz requires at least 1 second between requests
        if rate_limit_ms < 1000 {
            tracing::warn!(
                "Rate limit {}ms is below MusicBrainz minimum of 1000ms, this may result in rate limiting",
                rate_limit_ms
            );
        }

        // Reasonable upper bound to catch configuration errors
        if rate_limit_ms > 60_000 {
            return Err(AppError::Internal(format!(
                "Rate limit {}ms is unreasonably high (max 60000ms)",
                rate_limit_ms
            )));
        }

        let user_agent = format!("{}/{} ({})", app_name, app_version, contact);

        let client = Client::builder()
            .user_agent(user_agent)
            .timeout(Duration::from_secs(REQUEST_TIMEOUT_SECS))
            .build()
            .map_err(|e| AppError::Internal(format!("Failed to create HTTP client: {}", e)))?;

        Ok(Self {
            client,
            rate_limiter: RateLimiter::new(Duration::from_millis(rate_limit_ms)),
        })
    }

    /// Create a new MusicBrainz client wrapped in Arc for shared access.
    pub fn new_shared(
        app_name: &str,
        app_version: &str,
        contact: &str,
        rate_limit_ms: u64,
    ) -> Result<Arc<Self>> {
        Ok(Arc::new(Self::new(
            app_name,
            app_version,
            contact,
            rate_limit_ms,
        )?))
    }

    // =========================================================================
    // Artist Operations
    // =========================================================================

    /// Search for artists by name.
    pub async fn search_artists(&self, query: &str) -> Result<Vec<MbArtist>> {
        tracing::debug!(query = %query, "Searching MusicBrainz artists");

        let params = [("query", query.to_string()), ("fmt", "json".to_string())];

        let response: MbSearchResponse<MbArtist> = self.get_with_params("/artist", &params).await?;
        Ok(response.artists.unwrap_or_default())
    }

    /// Get detailed information about a specific artist.
    pub async fn get_artist(&self, mbid: &str) -> Result<MbArtistDetails> {
        tracing::debug!(mbid = %mbid, "Fetching MusicBrainz artist details");

        let params = [
            ("inc", "release-groups".to_string()),
            ("fmt", "json".to_string()),
        ];

        self.get_with_params(&format!("/artist/{}", mbid), &params)
            .await
    }

    /// Get release groups (albums) for an artist.
    pub async fn get_artist_releases(&self, mbid: &str) -> Result<Vec<MbReleaseGroup>> {
        tracing::debug!(mbid = %mbid, "Fetching MusicBrainz artist releases");

        let params = [("artist", mbid.to_string()), ("fmt", "json".to_string())];

        let response: MbSearchResponse<MbReleaseGroup> =
            self.get_with_params("/release-group", &params).await?;
        Ok(response.release_groups.unwrap_or_default())
    }

    // =========================================================================
    // Release Group Operations
    // =========================================================================

    /// Search for release groups (albums) by title.
    ///
    /// Optionally filter by artist MBID.
    pub async fn search_release_groups(
        &self,
        query: &str,
        artist_mbid: Option<&str>,
    ) -> Result<Vec<MbReleaseGroup>> {
        tracing::debug!(query = %query, artist = ?artist_mbid, "Searching MusicBrainz release groups");

        let full_query = match artist_mbid {
            Some(artist) => format!("{} AND arid:{}", query, artist),
            None => query.to_string(),
        };

        let params = [("query", full_query), ("fmt", "json".to_string())];

        let response: MbSearchResponse<MbReleaseGroup> =
            self.get_with_params("/release-group", &params).await?;
        Ok(response.release_groups.unwrap_or_default())
    }

    /// Get detailed information about a specific release group.
    pub async fn get_release_group(&self, mbid: &str) -> Result<MbReleaseGroupDetails> {
        tracing::debug!(mbid = %mbid, "Fetching MusicBrainz release group details");

        let params = [
            ("inc", "releases+artist-credits".to_string()),
            ("fmt", "json".to_string()),
        ];

        self.get_with_params(&format!("/release-group/{}", mbid), &params)
            .await
    }

    // =========================================================================
    // Release Operations
    // =========================================================================

    /// Get releases for a release group.
    pub async fn get_releases_for_group(&self, release_group_mbid: &str) -> Result<Vec<MbRelease>> {
        tracing::debug!(mbid = %release_group_mbid, "Fetching MusicBrainz releases for group");

        let params = [
            ("release-group", release_group_mbid.to_string()),
            ("fmt", "json".to_string()),
        ];

        let response: MbSearchResponse<MbRelease> =
            self.get_with_params("/release", &params).await?;
        Ok(response.releases.unwrap_or_default())
    }

    /// Get detailed information about a specific release.
    pub async fn get_release(&self, mbid: &str) -> Result<MbReleaseDetails> {
        tracing::debug!(mbid = %mbid, "Fetching MusicBrainz release details");

        let params = [
            ("inc", "recordings+artist-credits+media".to_string()),
            ("fmt", "json".to_string()),
        ];

        self.get_with_params(&format!("/release/{}", mbid), &params)
            .await
    }

    // =========================================================================
    // Cover Art Archive Operations
    // =========================================================================

    /// Get cover art information for a release.
    ///
    /// Returns `None` if no cover art is available (404 is normal).
    pub async fn get_cover_art(&self, release_mbid: &str) -> Result<Option<CoverArt>> {
        tracing::debug!(mbid = %release_mbid, "Fetching cover art");

        let url = format!("{}/release/{}", COVER_ART_BASE, release_mbid);

        // Don't use rate limiter for Cover Art Archive (separate service)
        let response = self
            .client
            .get(&url)
            .send()
            .await
            .map_err(|e| AppError::Internal(format!("Cover art request failed: {}", e)))?;

        let status = response.status();

        // 404 is normal - no cover art available
        if status == reqwest::StatusCode::NOT_FOUND {
            return Ok(None);
        }

        if !status.is_success() {
            return Err(AppError::Internal(format!(
                "Cover Art Archive returned error status: {}",
                status
            )));
        }

        let cover_art = response.json::<CoverArt>().await.map_err(|e| {
            AppError::Internal(format!("Failed to parse cover art response: {}", e))
        })?;

        Ok(Some(cover_art))
    }

    /// Generate a cover art URL for a release.
    ///
    /// # Arguments
    /// * `release_mbid` - MusicBrainz release ID
    /// * `size` - Image size: "250", "500", "1200"
    pub fn cover_url(&self, release_mbid: &str, size: &str) -> String {
        format!("{}/release/{}/front-{}", COVER_ART_BASE, release_mbid, size)
    }

    // =========================================================================
    // Internal Helpers
    // =========================================================================

    /// Internal helper to perform GET requests with query parameters and deserialize JSON responses.
    async fn get_with_params<T, P>(&self, path: &str, params: &[P]) -> Result<T>
    where
        T: for<'de> Deserialize<'de>,
        P: serde::Serialize,
    {
        // Enforce rate limit
        self.rate_limiter.wait().await;

        let url = format!("{}{}", MB_BASE_URL, path);

        let response = self
            .client
            .get(&url)
            .query(params)
            .send()
            .await
            .map_err(|e| {
                AppError::Internal(format!("MusicBrainz request to {} failed: {}", path, e))
            })?;

        let status = response.status();

        if status == reqwest::StatusCode::NOT_FOUND {
            return Err(AppError::NotFound(format!(
                "MusicBrainz resource not found: {}",
                path
            )));
        }

        if status == reqwest::StatusCode::TOO_MANY_REQUESTS {
            return Err(AppError::Internal(
                "MusicBrainz rate limit exceeded, please try again later".to_string(),
            ));
        }

        if status == reqwest::StatusCode::SERVICE_UNAVAILABLE {
            return Err(AppError::Internal(
                "MusicBrainz service temporarily unavailable".to_string(),
            ));
        }

        if !status.is_success() {
            return Err(AppError::Internal(format!(
                "MusicBrainz API {} returned error status: {}",
                path, status
            )));
        }

        response.json::<T>().await.map_err(|e| {
            AppError::Internal(format!(
                "Failed to parse MusicBrainz response from {}: {}",
                path, e
            ))
        })
    }
}

// =============================================================================
// Response Types
// =============================================================================

/// Generic search response wrapper from MusicBrainz API.
#[derive(Debug, Deserialize)]
#[allow(dead_code)]
pub struct MbSearchResponse<T> {
    /// Artist search results
    pub artists: Option<Vec<T>>,
    /// Release group search results
    #[serde(rename = "release-groups")]
    pub release_groups: Option<Vec<T>>,
    /// Release search results
    pub releases: Option<Vec<T>>,
    /// Total number of results (useful for pagination in the future)
    pub count: Option<i32>,
    /// Offset for pagination (useful for pagination in the future)
    pub offset: Option<i32>,
}

// =============================================================================
// Artist Types
// =============================================================================

/// Artist search result from MusicBrainz.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct MbArtist {
    /// MusicBrainz ID (UUID)
    pub id: String,
    /// Artist name
    pub name: String,
    /// Sort name (e.g., "Beatles, The")
    pub sort_name: String,
    /// Disambiguation comment (e.g., "UK rock band")
    pub disambiguation: Option<String>,
    /// Artist type: Person, Group, Orchestra, Choir, Character, Other
    #[serde(rename = "type")]
    pub artist_type: Option<String>,
    /// Country code (ISO 3166-1 alpha-2)
    pub country: Option<String>,
    /// Life span information
    pub life_span: Option<LifeSpan>,
    /// Search relevance score (0-100)
    pub score: Option<u8>,
}

/// Artist details with release groups.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct MbArtistDetails {
    /// MusicBrainz ID (UUID)
    pub id: String,
    /// Artist name
    pub name: String,
    /// Sort name (e.g., "Beatles, The")
    pub sort_name: String,
    /// Disambiguation comment
    pub disambiguation: Option<String>,
    /// Artist type: Person, Group, Orchestra, Choir, Character, Other
    #[serde(rename = "type")]
    pub artist_type: Option<String>,
    /// Country code (ISO 3166-1 alpha-2)
    pub country: Option<String>,
    /// Life span information
    pub life_span: Option<LifeSpan>,
    /// Release groups by this artist
    #[serde(default)]
    pub release_groups: Vec<MbReleaseGroup>,
}

/// Life span of an artist or group.
#[derive(Debug, Deserialize)]
pub struct LifeSpan {
    /// Begin date (YYYY, YYYY-MM, or YYYY-MM-DD)
    pub begin: Option<String>,
    /// End date (YYYY, YYYY-MM, or YYYY-MM-DD)
    pub end: Option<String>,
    /// Whether the artist/group has ended
    pub ended: Option<bool>,
}

// =============================================================================
// Release Group Types
// =============================================================================

/// Release group (album) from MusicBrainz.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct MbReleaseGroup {
    /// MusicBrainz ID (UUID)
    pub id: String,
    /// Title of the release group
    pub title: String,
    /// Primary type: Album, Single, EP, Broadcast, Other
    pub primary_type: Option<String>,
    /// Secondary types: Compilation, Soundtrack, Spokenword, Interview, etc.
    #[serde(default)]
    pub secondary_types: Vec<String>,
    /// First release date (YYYY, YYYY-MM, or YYYY-MM-DD)
    pub first_release_date: Option<String>,
    /// Artist credits
    #[serde(default)]
    pub artist_credit: Vec<ArtistCredit>,
    /// Search relevance score (0-100)
    pub score: Option<u8>,
}

/// Release group details with releases.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct MbReleaseGroupDetails {
    /// MusicBrainz ID (UUID)
    pub id: String,
    /// Title of the release group
    pub title: String,
    /// Primary type: Album, Single, EP, Broadcast, Other
    pub primary_type: Option<String>,
    /// Secondary types
    #[serde(default)]
    pub secondary_types: Vec<String>,
    /// First release date
    pub first_release_date: Option<String>,
    /// Artist credits
    #[serde(default)]
    pub artist_credit: Vec<ArtistCredit>,
    /// Releases in this release group
    #[serde(default)]
    pub releases: Vec<MbRelease>,
}

// =============================================================================
// Release Types
// =============================================================================

/// Release (specific edition) from MusicBrainz.
#[derive(Debug, Deserialize)]
pub struct MbRelease {
    /// MusicBrainz ID (UUID)
    pub id: String,
    /// Release title
    pub title: String,
    /// Status: Official, Promotion, Bootleg, Pseudo-Release
    pub status: Option<String>,
    /// Country code (ISO 3166-1 alpha-2)
    pub country: Option<String>,
    /// Release date (YYYY, YYYY-MM, or YYYY-MM-DD)
    pub date: Option<String>,
    /// Barcode (EAN/UPC)
    pub barcode: Option<String>,
}

/// Release details with track listing.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct MbReleaseDetails {
    /// MusicBrainz ID (UUID)
    pub id: String,
    /// Release title
    pub title: String,
    /// Status: Official, Promotion, Bootleg, Pseudo-Release
    pub status: Option<String>,
    /// Country code
    pub country: Option<String>,
    /// Release date
    pub date: Option<String>,
    /// Barcode
    pub barcode: Option<String>,
    /// Artist credits
    #[serde(default)]
    pub artist_credit: Vec<ArtistCredit>,
    /// Media (discs/sides)
    #[serde(default)]
    pub media: Vec<MbMedium>,
}

/// Medium (disc/side) in a release.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct MbMedium {
    /// Position in release (1, 2, ...)
    pub position: u32,
    /// Format: CD, Vinyl, Digital Media, etc.
    pub format: Option<String>,
    /// Number of tracks
    pub track_count: u32,
    /// Tracks on this medium
    #[serde(default)]
    pub tracks: Vec<MbTrack>,
}

/// Track on a medium.
#[derive(Debug, Deserialize)]
pub struct MbTrack {
    /// Track ID (not the same as recording ID)
    pub id: String,
    /// Track title (may differ from recording title)
    pub title: String,
    /// Position on medium
    pub position: u32,
    /// Track length in milliseconds
    pub length: Option<u32>,
    /// Recording information
    pub recording: MbRecording,
}

/// Recording (unique performance).
#[derive(Debug, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct MbRecording {
    /// MusicBrainz ID (UUID)
    pub id: String,
    /// Recording title
    pub title: String,
    /// Recording length in milliseconds
    pub length: Option<u32>,
    /// Artist credits for this recording
    #[serde(default)]
    pub artist_credit: Vec<ArtistCredit>,
}

/// Artist credit for a release or recording.
#[derive(Debug, Deserialize)]
pub struct ArtistCredit {
    /// The artist
    pub artist: MbArtistRef,
    /// Join phrase to next artist (e.g., " & ", " feat. ")
    pub joinphrase: Option<String>,
}

/// Minimal artist reference.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct MbArtistRef {
    /// MusicBrainz ID (UUID)
    pub id: String,
    /// Artist name
    pub name: String,
    /// Sort name
    pub sort_name: Option<String>,
}

// =============================================================================
// Cover Art Types
// =============================================================================

/// Cover art information from Cover Art Archive.
#[derive(Debug, Deserialize)]
pub struct CoverArt {
    /// List of available images
    pub images: Vec<CoverArtImage>,
    /// URL to the release on MusicBrainz
    pub release: String,
}

/// Individual cover art image.
#[derive(Debug, Deserialize)]
pub struct CoverArtImage {
    /// Whether this is the front cover
    pub front: bool,
    /// Whether this is the back cover
    pub back: bool,
    /// Image types (e.g., "Front", "Back", "Booklet")
    pub types: Vec<String>,
    /// Full-size image URL
    pub image: String,
    /// Thumbnail URLs
    pub thumbnails: CoverArtThumbnails,
}

/// Thumbnail URLs for cover art.
#[derive(Debug, Deserialize)]
pub struct CoverArtThumbnails {
    /// 250px thumbnail
    #[serde(rename = "250")]
    pub small: Option<String>,
    /// 500px thumbnail
    #[serde(rename = "500")]
    pub large: Option<String>,
    /// 1200px thumbnail
    #[serde(rename = "1200")]
    pub xlarge: Option<String>,
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cover_url() {
        let client = MusicBrainzClient::new("test-app", "0.1.0", "test@example.com", 1000).unwrap();
        let url = client.cover_url("12345678-1234-1234-1234-123456789012", "500");
        assert_eq!(
            url,
            "https://coverartarchive.org/release/12345678-1234-1234-1234-123456789012/front-500"
        );
    }

    #[test]
    fn test_empty_app_name_rejected() {
        let result = MusicBrainzClient::new("", "0.1.0", "test@example.com", 1000);
        assert!(result.is_err());
    }

    #[test]
    fn test_whitespace_app_name_rejected() {
        let result = MusicBrainzClient::new("   ", "0.1.0", "test@example.com", 1000);
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_rate_limiter() {
        let limiter = RateLimiter::new(Duration::from_millis(100));

        let start = Instant::now();

        // First request should be immediate
        limiter.wait().await;
        let first_elapsed = start.elapsed();
        assert!(first_elapsed < Duration::from_millis(50));

        // Second request should wait
        limiter.wait().await;
        let second_elapsed = start.elapsed();
        assert!(second_elapsed >= Duration::from_millis(100));
    }

    #[test]
    fn test_empty_app_version_rejected() {
        let result = MusicBrainzClient::new("test-app", "", "test@example.com", 1000);
        assert!(result.is_err());
    }

    #[test]
    fn test_whitespace_app_version_rejected() {
        let result = MusicBrainzClient::new("test-app", "   ", "test@example.com", 1000);
        assert!(result.is_err());
    }

    #[test]
    fn test_empty_contact_rejected() {
        let result = MusicBrainzClient::new("test-app", "0.1.0", "", 1000);
        assert!(result.is_err());
    }

    #[test]
    fn test_whitespace_contact_rejected() {
        let result = MusicBrainzClient::new("test-app", "0.1.0", "   ", 1000);
        assert!(result.is_err());
    }

    #[test]
    fn test_excessive_rate_limit_rejected() {
        let result = MusicBrainzClient::new("test-app", "0.1.0", "test@example.com", 100_000);
        assert!(result.is_err());
    }

    #[test]
    fn test_deserialize_artist_search_response() {
        let json = r#"{
            "artists": [
                {
                    "id": "b10bbbfc-cf9e-42e0-be17-e2c3e1d2600d",
                    "name": "The Beatles",
                    "sort-name": "Beatles, The",
                    "type": "Group",
                    "score": 100
                }
            ],
            "count": 1,
            "offset": 0
        }"#;

        let response: MbSearchResponse<MbArtist> =
            serde_json::from_str(json).expect("Should deserialize");
        let artists = response.artists.unwrap();
        assert_eq!(artists.len(), 1);
        assert_eq!(artists[0].name, "The Beatles");
        assert_eq!(artists[0].sort_name, "Beatles, The");
        assert_eq!(artists[0].artist_type, Some("Group".to_string()));
    }

    #[test]
    fn test_deserialize_artist_with_missing_optional_fields() {
        let json = r#"{
            "id": "b10bbbfc-cf9e-42e0-be17-e2c3e1d2600d",
            "name": "The Beatles",
            "sort-name": "Beatles, The"
        }"#;

        let artist: MbArtist =
            serde_json::from_str(json).expect("Should deserialize with missing optional fields");
        assert_eq!(artist.name, "The Beatles");
        assert!(artist.country.is_none());
        assert!(artist.life_span.is_none());
        assert!(artist.artist_type.is_none());
    }

    #[test]
    fn test_deserialize_release_group() {
        let json = r#"{
            "id": "1234-5678",
            "title": "Abbey Road",
            "primary-type": "Album",
            "secondary-types": ["Compilation"],
            "first-release-date": "1969-09-26"
        }"#;

        let rg: MbReleaseGroup =
            serde_json::from_str(json).expect("Should deserialize release group");
        assert_eq!(rg.title, "Abbey Road");
        assert_eq!(rg.primary_type, Some("Album".to_string()));
        assert_eq!(rg.secondary_types, vec!["Compilation".to_string()]);
        assert_eq!(rg.first_release_date, Some("1969-09-26".to_string()));
    }
}
