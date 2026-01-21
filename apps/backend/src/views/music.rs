//! Music views - thin wrappers around API handlers
//!
//! These views call the existing API handlers to get data, then render
//! HTML templates. Mutations (delete, download, etc.) are handled via
//! HTMX calling the REST API directly.

use askama::Template;
use axum::{
    extract::{Path, Query, State},
    response::{Html, IntoResponse, Redirect},
};
use axum_extra::extract::CookieJar;
use serde::Deserialize;

use axum_extra::extract::Form;

use crate::api::music::{
    add_artist as api_add_artist, get_album as api_get_album, get_artist as api_get_artist,
    list_artists as api_list_artists, unified_search_album as api_unified_search, AddArtistRequest,
    ListArtistsQuery, UnifiedSearchRequest,
};
use crate::api::search::{search_mb_artists, SearchQuery as ApiSearchQuery};
use crate::AppState;

use super::auth;

// =============================================================================
// Templates
// =============================================================================

#[derive(Template)]
#[template(path = "pages/artists.html")]
pub struct ArtistsTemplate {
    pub artists: Vec<ArtistView>,
    pub total: u64,
}

#[derive(Template)]
#[template(path = "pages/artist_detail.html")]
pub struct ArtistDetailTemplate {
    pub artist: ArtistView,
    pub albums: Vec<AlbumView>,
}

#[derive(Template)]
#[template(path = "pages/album_detail.html")]
pub struct AlbumDetailTemplate {
    pub album: AlbumView,
    pub artist: ArtistView,
    pub tracks: Vec<TrackView>,
}

#[derive(Template)]
#[template(path = "partials/musicbrainz_search_results.html")]
pub struct SearchResultsTemplate {
    pub results: Vec<MusicBrainzResult>,
    pub search_type: String,
}

#[derive(Template)]
#[template(path = "components/search_modal.html")]
pub struct SearchModalTemplate {
    pub search_type: String,
    pub search_endpoint: String,
}

#[derive(Template)]
#[template(path = "partials/album_releases.html")]
pub struct AlbumReleasesTemplate {
    pub torrent_releases: Vec<ReleaseView>,
    pub soulseek_results: Vec<SoulseekResultView>,
    pub album_id: i64,
}

// =============================================================================
// View Models
// =============================================================================

pub struct ArtistView {
    pub id: i64,
    pub mbid: String,
    pub name: String,
    pub sort_name: Option<String>,
    pub disambiguation: Option<String>,
    pub artist_type: Option<String>,
    pub country: Option<String>,
    pub overview: Option<String>,
    pub image_path: Option<String>,
    pub monitored: bool,
}

pub struct AlbumView {
    pub id: i64,
    pub mbid: String,
    pub artist_id: i64,
    pub title: String,
    pub album_type: Option<String>,
    pub release_date: Option<String>,
    pub cover_path: Option<String>,
    pub total_tracks: i32,
    pub status: String,
    pub monitored: bool,
}

pub struct TrackView {
    pub id: i64,
    pub track_number: i32,
    pub title: String,
    pub duration_ms: Option<i32>,
    pub status: String,
    pub audio_format: Option<String>,
    pub bitrate: Option<i32>,
}

pub struct ReleaseView {
    pub id: String,
    pub title: String,
    pub indexer: String,
    pub size_display: String,
    pub seeders: u32,
    pub quality: Option<String>,
    pub magnet: String,
}

pub struct SoulseekResultView {
    pub username: String,
    pub folder: String,
    pub file_count: usize,
    pub quality: String,
    pub user_speed: String,
    pub files_json: String, // JSON encoded for form submission
}

pub struct MusicBrainzResult {
    pub id: String,
    pub name: String,
    pub disambiguation: Option<String>,
    pub artist_type: Option<String>,
    pub country: Option<String>,
}

// =============================================================================
// Query Types
// =============================================================================

#[derive(Deserialize)]
pub struct ListQuery {
    pub page: Option<u32>,
    pub search: Option<String>,
}

#[derive(Deserialize)]
pub struct SearchQuery {
    pub q: Option<String>,
}

/// Form data for adding an artist via HTMX
#[derive(Deserialize)]
pub struct AddArtistForm {
    pub mbid: String,
}

// =============================================================================
// View Handlers
// =============================================================================

/// List artists page - calls API handler
pub async fn artists(
    State(state): State<AppState>,
    cookies: CookieJar,
    Query(query): Query<ListQuery>,
) -> impl IntoResponse {
    if auth::get_current_user(&state, &cookies).await.is_none() {
        return Redirect::to("/login").into_response();
    }

    // Call API handler
    let response = api_list_artists(
        State(state),
        axum::extract::Query(ListArtistsQuery {
            monitored: None,
            search: query.search,
            page: query.page,
            limit: Some(24),
        }),
    )
    .await;

    match response {
        Ok(json) => {
            let data = json.0;
            ArtistsTemplate {
                artists: data
                    .items
                    .into_iter()
                    .map(|a| ArtistView {
                        id: a.artist.id,
                        mbid: a.artist.mbid,
                        name: a.artist.name,
                        sort_name: a.artist.sort_name,
                        disambiguation: a.artist.disambiguation,
                        artist_type: a.artist.artist_type,
                        country: a.artist.country,
                        overview: a.artist.overview,
                        image_path: a.artist.image_path,
                        monitored: a.artist.monitored,
                    })
                    .collect(),
                total: data.total,
            }
            .into_response()
        }
        Err(_) => Html("<div class='lcars-error'>Failed to load artists</div>").into_response(),
    }
}

/// Artist detail page - calls API handler
pub async fn artist_detail(
    State(state): State<AppState>,
    cookies: CookieJar,
    Path(id): Path<i64>,
) -> impl IntoResponse {
    if auth::get_current_user(&state, &cookies).await.is_none() {
        return Redirect::to("/login").into_response();
    }

    // Call API handler
    let response = api_get_artist(State(state), Path(id)).await;

    match response {
        Ok(json) => {
            let data = json.0;
            ArtistDetailTemplate {
                artist: ArtistView {
                    id: data.artist.id,
                    mbid: data.artist.mbid,
                    name: data.artist.name,
                    sort_name: data.artist.sort_name,
                    disambiguation: data.artist.disambiguation,
                    artist_type: data.artist.artist_type,
                    country: data.artist.country,
                    overview: data.artist.overview,
                    image_path: data.artist.image_path,
                    monitored: data.artist.monitored,
                },
                albums: data
                    .albums
                    .into_iter()
                    .map(|a| AlbumView {
                        id: a.id,
                        mbid: a.mbid,
                        artist_id: a.artist_id,
                        title: a.title,
                        album_type: a.album_type,
                        release_date: a.release_date,
                        cover_path: a.cover_path,
                        total_tracks: a.total_tracks.unwrap_or(0),
                        status: a.status.to_string(),
                        monitored: a.monitored,
                    })
                    .collect(),
            }
            .into_response()
        }
        Err(_) => Html("<div class='lcars-error'>Artist not found</div>").into_response(),
    }
}

/// Album detail page - calls API handler
pub async fn album_detail(
    State(state): State<AppState>,
    cookies: CookieJar,
    Path(id): Path<i64>,
) -> impl IntoResponse {
    if auth::get_current_user(&state, &cookies).await.is_none() {
        return Redirect::to("/login").into_response();
    }

    // Call API handler to get album with tracks
    let response = api_get_album(State(state.clone()), Path(id)).await;

    match response {
        Ok(json) => {
            let data = json.0;

            // We need artist info too - call artist API
            let artist_response = api_get_artist(State(state), Path(data.album.artist_id)).await;

            let artist = match artist_response {
                Ok(artist_json) => {
                    let artist_data = artist_json.0.artist;
                    ArtistView {
                        id: artist_data.id,
                        mbid: artist_data.mbid,
                        name: artist_data.name,
                        sort_name: artist_data.sort_name,
                        disambiguation: artist_data.disambiguation,
                        artist_type: artist_data.artist_type,
                        country: artist_data.country,
                        overview: artist_data.overview,
                        image_path: artist_data.image_path,
                        monitored: artist_data.monitored,
                    }
                }
                Err(_) => ArtistView {
                    id: 0,
                    mbid: String::new(),
                    name: "Unknown Artist".to_string(),
                    sort_name: None,
                    disambiguation: None,
                    artist_type: None,
                    country: None,
                    overview: None,
                    image_path: None,
                    monitored: false,
                },
            };

            AlbumDetailTemplate {
                album: AlbumView {
                    id: data.album.id,
                    mbid: data.album.mbid,
                    artist_id: data.album.artist_id,
                    title: data.album.title,
                    album_type: data.album.album_type,
                    release_date: data.album.release_date,
                    cover_path: data.album.cover_path,
                    total_tracks: data.album.total_tracks.unwrap_or(0),
                    status: data.album.status.to_string(),
                    monitored: data.album.monitored,
                },
                artist,
                tracks: data
                    .tracks
                    .into_iter()
                    .map(|t| TrackView {
                        id: t.id,
                        track_number: t.track_number,
                        title: t.title,
                        duration_ms: t.duration_ms,
                        status: t.status.to_string(),
                        audio_format: t.audio_format,
                        bitrate: t.bitrate,
                    })
                    .collect(),
            }
            .into_response()
        }
        Err(_) => Html("<div class='lcars-error'>Album not found</div>").into_response(),
    }
}

/// Search modal fragment for music artists
pub async fn search_modal() -> impl IntoResponse {
    SearchModalTemplate {
        search_type: "music".to_string(),
        search_endpoint: "/search/musicbrainz/artists".to_string(),
    }
}

/// Search MusicBrainz for artists - calls API handler (returns HTML fragment)
pub async fn search_artists(
    State(state): State<AppState>,
    cookies: CookieJar,
    Query(query): Query<SearchQuery>,
) -> impl IntoResponse {
    if auth::get_current_user(&state, &cookies).await.is_none() {
        return Html("<div class='lcars-error'>Unauthorized</div>").into_response();
    }

    let Some(q) = query.q.filter(|s| !s.is_empty()) else {
        return Html("").into_response();
    };

    // Call API handler
    let response =
        search_mb_artists(State(state), axum::extract::Query(ApiSearchQuery { q })).await;

    match response {
        Ok(json) => SearchResultsTemplate {
            results: json
                .0
                .into_iter()
                .take(10)
                .map(|r| MusicBrainzResult {
                    id: r.mbid,
                    name: r.name,
                    disambiguation: r.disambiguation,
                    artist_type: r.artist_type,
                    country: r.country,
                })
                .collect(),
            search_type: "music".to_string(),
        }
        .into_response(),
        Err(e) => Html(format!(
            "<div class='lcars-error'>Search failed: {}</div>",
            e
        ))
        .into_response(),
    }
}

/// Search for album releases - calls unified search API (returns HTML fragment)
pub async fn album_search(
    State(state): State<AppState>,
    cookies: CookieJar,
    Path(id): Path<i64>,
) -> impl IntoResponse {
    if auth::get_current_user(&state, &cookies).await.is_none() {
        return Html("<div class='lcars-error'>Unauthorized</div>").into_response();
    }

    // Call unified search API handler
    let response = api_unified_search(
        State(state),
        Path(id),
        axum::Json(UnifiedSearchRequest {
            sources: Some(vec!["all".to_string()]),
            query: None,
        }),
    )
    .await;

    match response {
        Ok(json) => {
            let data = json.0;

            // Convert indexer results to view model
            let torrent_releases: Vec<ReleaseView> = data
                .indexer_results
                .into_iter()
                .map(|r| ReleaseView {
                    id: r.id,
                    title: r.title,
                    indexer: r.indexer,
                    size_display: format_size(r.size_bytes),
                    seeders: r.seeders,
                    quality: Some(r.quality.to_string()),
                    magnet: r.magnet,
                })
                .collect();

            // Convert soulseek results to view model
            let soulseek_results: Vec<SoulseekResultView> = data
                .soulseek_results
                .into_iter()
                .map(|r| {
                    // Serialize files for form submission
                    let files_for_json: Vec<serde_json::Value> = r
                        .files
                        .iter()
                        .map(|f| {
                            serde_json::json!({
                                "filename": f.filename,
                                "size": f.size,
                            })
                        })
                        .collect();

                    SoulseekResultView {
                        username: r.username,
                        folder: r.folder,
                        file_count: r.files.len(),
                        quality: format!("{} {}kbps", r.quality.format, r.quality.bitrate),
                        user_speed: format_speed(r.user_speed as u64),
                        files_json: serde_json::to_string(&files_for_json).unwrap_or_default(),
                    }
                })
                .collect();

            AlbumReleasesTemplate {
                torrent_releases,
                soulseek_results,
                album_id: id,
            }
            .into_response()
        }
        Err(_) => Html("<div class='lcars-error'>Search failed</div>").into_response(),
    }
}

// =============================================================================
// Helpers
// =============================================================================

fn format_size(bytes: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = KB * 1024;
    const GB: u64 = MB * 1024;

    if bytes >= GB {
        format!("{:.2} GB", bytes as f64 / GB as f64)
    } else if bytes >= MB {
        format!("{:.2} MB", bytes as f64 / MB as f64)
    } else if bytes >= KB {
        format!("{:.2} KB", bytes as f64 / KB as f64)
    } else {
        format!("{} B", bytes)
    }
}

fn format_speed(bytes_per_sec: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = KB * 1024;

    if bytes_per_sec >= MB {
        format!("{:.1} MB/s", bytes_per_sec as f64 / MB as f64)
    } else if bytes_per_sec >= KB {
        format!("{:.1} KB/s", bytes_per_sec as f64 / KB as f64)
    } else {
        format!("{} B/s", bytes_per_sec)
    }
}

/// Add an artist via form submission - calls API handler
pub async fn add_artist(
    State(state): State<AppState>,
    cookies: CookieJar,
    Form(form): Form<AddArtistForm>,
) -> impl IntoResponse {
    // Get claims from cookies
    let Some(claims) = auth::get_current_user(&state, &cookies).await else {
        return Html("<div class='lcars-error'>Unauthorized</div>").into_response();
    };

    // Call API handler with JSON body
    let response = api_add_artist(
        State(state),
        axum::Extension(claims),
        axum::Json(AddArtistRequest {
            mbid: form.mbid,
            monitored: Some(true),
            quality_limit: None,
        }),
    )
    .await;

    match response {
        Ok(json) => {
            let artist = json.0;
            // Use HX-Redirect header for HTMX client-side redirect
            (
                [(
                    axum::http::header::HeaderName::from_static("hx-redirect"),
                    format!("/music/artists/{}", artist.artist.id),
                )],
                Html(""),
            )
                .into_response()
        }
        Err(e) => Html(format!(
            "<div class='lcars-error'>Failed to add artist: {}</div>",
            e
        ))
        .into_response(),
    }
}
