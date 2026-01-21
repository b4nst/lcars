//! HTML templates for API responses.
//!
//! These templates are used when clients request HTML (via Accept header or HTMX).
//! They render partial HTML fragments that HTMX can swap into the page.
//!
//! NOTE: Content negotiation is not yet implemented in API handlers.
//! This module is a placeholder for future use.

use askama::Template;

// =============================================================================
// Search Templates
// =============================================================================

/// Template for TMDB movie search results.
#[derive(Template)]
#[template(path = "partials/tmdb_search_results.html")]
pub struct TmdbSearchResultsTemplate {
    pub results: Vec<TmdbResultView>,
    pub search_type: String,
}

pub struct TmdbResultView {
    pub id: i64,
    pub title: String,
    pub year: Option<String>,
    pub overview: Option<String>,
    pub poster_path: Option<String>,
}

/// Template for MusicBrainz artist search results.
#[derive(Template)]
#[template(path = "partials/musicbrainz_search_results.html")]
pub struct MusicBrainzSearchResultsTemplate {
    pub results: Vec<MbArtistResultView>,
    pub search_type: String,
}

pub struct MbArtistResultView {
    pub id: String,
    pub name: String,
    pub disambiguation: Option<String>,
    pub artist_type: Option<String>,
    pub country: Option<String>,
}
