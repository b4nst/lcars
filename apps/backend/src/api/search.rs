//! Search API endpoints for external metadata lookups.

use axum::{
    extract::{Query, State},
    Json,
};
use serde::{Deserialize, Serialize};

use crate::error::{AppError, Result};
use crate::AppState;

// =============================================================================
// TMDB Search Types
// =============================================================================

/// Query parameters for TMDB movie search.
#[derive(Debug, Deserialize)]
pub struct TmdbMovieSearchQuery {
    /// Search query string.
    pub q: String,
    /// Optional year filter.
    pub year: Option<i32>,
}

/// Query parameters for TMDB TV search.
#[derive(Debug, Deserialize)]
pub struct TmdbTvSearchQuery {
    /// Search query string.
    pub q: String,
}

/// TMDB movie search result.
#[derive(Debug, Serialize)]
pub struct TmdbMovieResult {
    pub id: i32,
    pub title: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub original_title: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub overview: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub poster_path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub backdrop_path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub release_date: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub vote_average: Option<f64>,
}

/// TMDB TV show search result.
#[derive(Debug, Serialize)]
pub struct TmdbTvResult {
    pub id: i32,
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub original_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub overview: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub poster_path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub backdrop_path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub first_air_date: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub vote_average: Option<f64>,
}

// =============================================================================
// Request/Response Types
// =============================================================================

/// Query parameters for search.
#[derive(Debug, Deserialize)]
pub struct SearchQuery {
    /// Search query string.
    pub q: String,
}

/// Query parameters for album search.
#[derive(Debug, Deserialize)]
pub struct SearchAlbumsQuery {
    /// Search query string.
    pub q: String,
    /// Optional artist MBID to filter results.
    pub artist_mbid: Option<String>,
}

/// MusicBrainz artist search result.
#[derive(Debug, Serialize)]
pub struct MbArtistResult {
    pub mbid: String,
    pub name: String,
    pub sort_name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub disambiguation: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub artist_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub country: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub begin_date: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub end_date: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub score: Option<u8>,
}

/// MusicBrainz album (release group) search result.
#[derive(Debug, Serialize)]
pub struct MbAlbumResult {
    pub mbid: String,
    pub title: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub primary_type: Option<String>,
    pub secondary_types: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub first_release_date: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub artist_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub artist_mbid: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub score: Option<u8>,
}

// =============================================================================
// Handlers
// =============================================================================

/// GET /api/search/musicbrainz/artists
///
/// Searches MusicBrainz for artists.
pub async fn search_mb_artists(
    State(state): State<AppState>,
    Query(query): Query<SearchQuery>,
) -> Result<Json<Vec<MbArtistResult>>> {
    let search_term = query.q.trim();
    if search_term.is_empty() {
        return Err(AppError::BadRequest("Search query is required".to_string()));
    }
    if search_term.len() < 2 {
        return Err(AppError::BadRequest(
            "Search query must be at least 2 characters".to_string(),
        ));
    }
    if search_term.len() > 200 {
        return Err(AppError::BadRequest(
            "Search query too long (max 200 characters)".to_string(),
        ));
    }

    let mb_client = state
        .musicbrainz_client()
        .ok_or_else(|| AppError::Internal("MusicBrainz client not configured".to_string()))?;

    let artists = mb_client.search_artists(search_term).await?;

    let results: Vec<MbArtistResult> = artists
        .into_iter()
        .map(|a| MbArtistResult {
            mbid: a.id,
            name: a.name,
            sort_name: a.sort_name,
            disambiguation: a.disambiguation,
            artist_type: a.artist_type,
            country: a.country,
            begin_date: a.life_span.as_ref().and_then(|ls| ls.begin.clone()),
            end_date: a.life_span.as_ref().and_then(|ls| ls.end.clone()),
            score: a.score,
        })
        .collect();

    tracing::debug!(
        query = %search_term,
        results = results.len(),
        "MusicBrainz artist search"
    );

    Ok(Json(results))
}

/// GET /api/search/musicbrainz/albums
///
/// Searches MusicBrainz for albums (release groups).
pub async fn search_mb_albums(
    State(state): State<AppState>,
    Query(query): Query<SearchAlbumsQuery>,
) -> Result<Json<Vec<MbAlbumResult>>> {
    let search_term = query.q.trim();
    if search_term.is_empty() {
        return Err(AppError::BadRequest("Search query is required".to_string()));
    }
    if search_term.len() < 2 {
        return Err(AppError::BadRequest(
            "Search query must be at least 2 characters".to_string(),
        ));
    }
    if search_term.len() > 200 {
        return Err(AppError::BadRequest(
            "Search query too long (max 200 characters)".to_string(),
        ));
    }

    let mb_client = state
        .musicbrainz_client()
        .ok_or_else(|| AppError::Internal("MusicBrainz client not configured".to_string()))?;

    let albums = mb_client
        .search_release_groups(search_term, query.artist_mbid.as_deref())
        .await?;

    let results: Vec<MbAlbumResult> = albums
        .into_iter()
        .map(|rg| {
            let (artist_name, artist_mbid) = rg
                .artist_credit
                .first()
                .map(|ac| (Some(ac.artist.name.clone()), Some(ac.artist.id.clone())))
                .unwrap_or((None, None));

            MbAlbumResult {
                mbid: rg.id,
                title: rg.title,
                primary_type: rg.primary_type,
                secondary_types: rg.secondary_types,
                first_release_date: rg.first_release_date,
                artist_name,
                artist_mbid,
                score: rg.score,
            }
        })
        .collect();

    tracing::debug!(
        query = %search_term,
        artist_mbid = ?query.artist_mbid,
        results = results.len(),
        "MusicBrainz album search"
    );

    Ok(Json(results))
}

/// GET /api/search/tmdb/movies
///
/// Searches TMDB for movies.
pub async fn search_tmdb_movies(
    State(state): State<AppState>,
    Query(query): Query<TmdbMovieSearchQuery>,
) -> Result<Json<Vec<TmdbMovieResult>>> {
    let search_term = query.q.trim();
    if search_term.is_empty() {
        return Err(AppError::BadRequest("Search query is required".to_string()));
    }
    if search_term.len() < 2 {
        return Err(AppError::BadRequest(
            "Search query must be at least 2 characters".to_string(),
        ));
    }
    if search_term.len() > 200 {
        return Err(AppError::BadRequest(
            "Search query too long (max 200 characters)".to_string(),
        ));
    }

    let tmdb_client = state
        .tmdb_client()
        .ok_or_else(|| AppError::Internal("TMDB client not configured".to_string()))?;

    let movies = tmdb_client.search_movies(search_term, query.year).await?;

    let results: Vec<TmdbMovieResult> = movies
        .into_iter()
        .map(|m| TmdbMovieResult {
            id: m.id,
            title: m.title,
            original_title: Some(m.original_title),
            overview: m.overview,
            poster_path: m.poster_path,
            backdrop_path: m.backdrop_path,
            release_date: m.release_date,
            vote_average: Some(m.vote_average),
        })
        .collect();

    tracing::debug!(
        query = %search_term,
        year = ?query.year,
        results = results.len(),
        "TMDB movie search"
    );

    Ok(Json(results))
}

/// GET /api/search/tmdb/tv
///
/// Searches TMDB for TV shows.
pub async fn search_tmdb_tv(
    State(state): State<AppState>,
    Query(query): Query<TmdbTvSearchQuery>,
) -> Result<Json<Vec<TmdbTvResult>>> {
    let search_term = query.q.trim();
    if search_term.is_empty() {
        return Err(AppError::BadRequest("Search query is required".to_string()));
    }
    if search_term.len() < 2 {
        return Err(AppError::BadRequest(
            "Search query must be at least 2 characters".to_string(),
        ));
    }
    if search_term.len() > 200 {
        return Err(AppError::BadRequest(
            "Search query too long (max 200 characters)".to_string(),
        ));
    }

    let tmdb_client = state
        .tmdb_client()
        .ok_or_else(|| AppError::Internal("TMDB client not configured".to_string()))?;

    let shows = tmdb_client.search_tv(search_term).await?;

    let results: Vec<TmdbTvResult> = shows
        .into_iter()
        .map(|s| TmdbTvResult {
            id: s.id,
            name: s.name,
            original_name: Some(s.original_name),
            overview: s.overview,
            poster_path: s.poster_path,
            backdrop_path: s.backdrop_path,
            first_air_date: s.first_air_date,
            vote_average: Some(s.vote_average),
        })
        .collect();

    tracing::debug!(
        query = %search_term,
        results = results.len(),
        "TMDB TV search"
    );

    Ok(Json(results))
}
