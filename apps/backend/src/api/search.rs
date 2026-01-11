//! Search API endpoints for external metadata lookups.

use axum::{
    extract::{Query, State},
    Json,
};
use serde::{Deserialize, Serialize};

use crate::error::{AppError, Result};
use crate::AppState;

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
