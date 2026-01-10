//! TMDB (The Movie Database) service client.
//!
//! Provides methods to search and fetch movie/TV show metadata from TMDB API.

use reqwest::Client;
use serde::Deserialize;
use std::sync::Arc;
use std::time::Duration;

use crate::error::{AppError, Result};

const TMDB_BASE_URL: &str = "https://api.themoviedb.org/3";
const TMDB_IMAGE_BASE: &str = "https://image.tmdb.org/t/p";
const REQUEST_TIMEOUT_SECS: u64 = 30;

/// TMDB API client for fetching movie and TV show metadata.
pub struct TmdbClient {
    client: Client,
    api_key: String,
}

impl TmdbClient {
    /// Create a new TMDB client with the given API key.
    ///
    /// Returns an error if the API key is empty or if the HTTP client cannot be built.
    pub fn new(api_key: String) -> Result<Self> {
        if api_key.trim().is_empty() {
            return Err(AppError::Internal(
                "TMDB API key cannot be empty".to_string(),
            ));
        }

        let client = Client::builder()
            .timeout(Duration::from_secs(REQUEST_TIMEOUT_SECS))
            .build()
            .map_err(|e| AppError::Internal(format!("Failed to create HTTP client: {}", e)))?;

        Ok(Self { client, api_key })
    }

    /// Create a new TMDB client wrapped in Arc for shared access.
    pub fn new_shared(api_key: String) -> Result<Arc<Self>> {
        Ok(Arc::new(Self::new(api_key)?))
    }

    /// Search for movies by title with optional year filter.
    ///
    /// TMDB allows up to 40 requests per 10 seconds.
    pub async fn search_movies(&self, query: &str, year: Option<i32>) -> Result<Vec<TmdbMovie>> {
        tracing::debug!(query = %query, year = ?year, "Searching TMDB movies");

        let mut params = vec![
            ("api_key", self.api_key.clone()),
            ("query", query.to_string()),
        ];

        if let Some(y) = year {
            params.push(("year", y.to_string()));
        }

        let response: TmdbSearchResponse<TmdbMovie> =
            self.get_with_params("/search/movie", &params).await?;
        Ok(response.results)
    }

    /// Search for TV shows by title.
    pub async fn search_tv(&self, query: &str) -> Result<Vec<TmdbTvShow>> {
        tracing::debug!(query = %query, "Searching TMDB TV shows");

        let params = [
            ("api_key", self.api_key.clone()),
            ("query", query.to_string()),
        ];

        let response: TmdbSearchResponse<TmdbTvShow> =
            self.get_with_params("/search/tv", &params).await?;
        Ok(response.results)
    }

    /// Get detailed information about a specific movie.
    pub async fn get_movie(&self, id: i32) -> Result<TmdbMovieDetails> {
        tracing::debug!(movie_id = %id, "Fetching TMDB movie details");

        let params = [("api_key", self.api_key.clone())];
        self.get_with_params(&format!("/movie/{}", id), &params)
            .await
    }

    /// Get detailed information about a specific TV show.
    pub async fn get_tv(&self, id: i32) -> Result<TmdbTvDetails> {
        tracing::debug!(tv_id = %id, "Fetching TMDB TV show details");

        let params = [("api_key", self.api_key.clone())];
        self.get_with_params(&format!("/tv/{}", id), &params).await
    }

    /// Get season details including all episodes.
    pub async fn get_season(&self, show_id: i32, season_number: i32) -> Result<TmdbSeason> {
        tracing::debug!(
            show_id = %show_id,
            season = %season_number,
            "Fetching TMDB season details"
        );

        let params = [("api_key", self.api_key.clone())];
        self.get_with_params(
            &format!("/tv/{}/season/{}", show_id, season_number),
            &params,
        )
        .await
    }

    /// Generate a poster URL for the given path and size.
    ///
    /// Common sizes: "w92", "w154", "w185", "w342", "w500", "w780", "original"
    pub fn poster_url(&self, path: &str, size: &str) -> String {
        format!("{}/{}{}", TMDB_IMAGE_BASE, size, path)
    }

    /// Generate a backdrop URL for the given path and size.
    ///
    /// Common sizes: "w300", "w780", "w1280", "original"
    pub fn backdrop_url(&self, path: &str, size: &str) -> String {
        format!("{}/{}{}", TMDB_IMAGE_BASE, size, path)
    }

    /// Internal helper to perform GET requests with query parameters and deserialize JSON responses.
    async fn get_with_params<T, P>(&self, path: &str, params: &[P]) -> Result<T>
    where
        T: for<'de> Deserialize<'de>,
        P: serde::Serialize,
    {
        let url = format!("{}{}", TMDB_BASE_URL, path);

        let response = self
            .client
            .get(&url)
            .query(params)
            .send()
            .await
            .map_err(|e| AppError::Internal(format!("TMDB request to {} failed: {}", path, e)))?;

        let status = response.status();

        if status == reqwest::StatusCode::UNAUTHORIZED {
            return Err(AppError::Internal(
                "TMDB API key is invalid or missing".to_string(),
            ));
        }

        if status == reqwest::StatusCode::NOT_FOUND {
            return Err(AppError::NotFound(format!(
                "TMDB resource not found: {}",
                path
            )));
        }

        if status == reqwest::StatusCode::TOO_MANY_REQUESTS {
            return Err(AppError::Internal(
                "TMDB rate limit exceeded, please try again later".to_string(),
            ));
        }

        if !status.is_success() {
            return Err(AppError::Internal(format!(
                "TMDB API {} returned error status: {}",
                path, status
            )));
        }

        response.json::<T>().await.map_err(|e| {
            AppError::Internal(format!(
                "Failed to parse TMDB response from {}: {}",
                path, e
            ))
        })
    }
}

// =============================================================================
// Response Types
// =============================================================================

/// Generic search response wrapper from TMDB API.
#[derive(Debug, Deserialize)]
pub struct TmdbSearchResponse<T> {
    pub results: Vec<T>,
    /// Current page number (for pagination support).
    #[allow(dead_code)]
    pub page: i32,
    /// Total number of pages available.
    #[allow(dead_code)]
    pub total_pages: i32,
    /// Total number of results across all pages.
    #[allow(dead_code)]
    pub total_results: i32,
}

/// Movie search result from TMDB.
#[derive(Debug, Deserialize)]
pub struct TmdbMovie {
    pub id: i32,
    pub title: String,
    pub original_title: String,
    pub overview: Option<String>,
    pub release_date: Option<String>,
    pub poster_path: Option<String>,
    pub backdrop_path: Option<String>,
    pub vote_average: f64,
}

/// Detailed movie information from TMDB.
#[derive(Debug, Deserialize)]
pub struct TmdbMovieDetails {
    pub id: i32,
    pub title: String,
    pub original_title: String,
    pub overview: Option<String>,
    pub release_date: Option<String>,
    pub poster_path: Option<String>,
    pub backdrop_path: Option<String>,
    pub vote_average: f64,
    pub runtime: Option<i32>,
    pub genres: Vec<TmdbGenre>,
    pub imdb_id: Option<String>,
    pub status: Option<String>,
    pub tagline: Option<String>,
    pub budget: Option<i64>,
    pub revenue: Option<i64>,
}

/// TV show search result from TMDB.
#[derive(Debug, Deserialize)]
pub struct TmdbTvShow {
    pub id: i32,
    pub name: String,
    pub original_name: String,
    pub overview: Option<String>,
    pub first_air_date: Option<String>,
    pub poster_path: Option<String>,
    pub backdrop_path: Option<String>,
    pub vote_average: f64,
}

/// Detailed TV show information from TMDB.
#[derive(Debug, Deserialize)]
pub struct TmdbTvDetails {
    pub id: i32,
    pub name: String,
    pub original_name: String,
    pub overview: Option<String>,
    pub first_air_date: Option<String>,
    pub last_air_date: Option<String>,
    pub poster_path: Option<String>,
    pub backdrop_path: Option<String>,
    pub vote_average: f64,
    pub genres: Vec<TmdbGenre>,
    pub status: Option<String>,
    pub number_of_seasons: Option<i32>,
    pub number_of_episodes: Option<i32>,
    pub seasons: Vec<TmdbSeasonSummary>,
    pub external_ids: Option<TmdbExternalIds>,
}

/// Season summary included in TV show details.
#[derive(Debug, Deserialize)]
pub struct TmdbSeasonSummary {
    pub id: i32,
    pub name: String,
    pub overview: Option<String>,
    pub air_date: Option<String>,
    pub episode_count: i32,
    pub poster_path: Option<String>,
    pub season_number: i32,
}

/// Full season details including episodes.
#[derive(Debug, Deserialize)]
pub struct TmdbSeason {
    pub id: i32,
    pub name: String,
    pub overview: Option<String>,
    pub air_date: Option<String>,
    pub poster_path: Option<String>,
    pub season_number: i32,
    pub episodes: Vec<TmdbEpisode>,
}

/// Episode information from TMDB.
#[derive(Debug, Deserialize)]
pub struct TmdbEpisode {
    pub id: i32,
    pub name: String,
    pub overview: Option<String>,
    pub air_date: Option<String>,
    pub episode_number: i32,
    pub season_number: i32,
    pub still_path: Option<String>,
    pub vote_average: f64,
    pub runtime: Option<i32>,
}

/// Genre information from TMDB.
#[derive(Debug, Deserialize)]
pub struct TmdbGenre {
    pub id: i32,
    pub name: String,
}

/// External IDs for cross-referencing with other databases.
#[derive(Debug, Deserialize)]
pub struct TmdbExternalIds {
    pub imdb_id: Option<String>,
    pub tvdb_id: Option<i32>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_poster_url() {
        let client = TmdbClient::new("test-key".to_string()).unwrap();
        let url = client.poster_url("/abc123.jpg", "w500");
        assert_eq!(url, "https://image.tmdb.org/t/p/w500/abc123.jpg");
    }

    #[test]
    fn test_backdrop_url() {
        let client = TmdbClient::new("test-key".to_string()).unwrap();
        let url = client.backdrop_url("/xyz789.jpg", "original");
        assert_eq!(url, "https://image.tmdb.org/t/p/original/xyz789.jpg");
    }

    #[test]
    fn test_empty_api_key_rejected() {
        let result = TmdbClient::new("".to_string());
        assert!(result.is_err());
    }

    #[test]
    fn test_whitespace_api_key_rejected() {
        let result = TmdbClient::new("   ".to_string());
        assert!(result.is_err());
    }
}
