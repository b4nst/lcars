//! HTML views for HTMX frontend
//!
//! This module contains route handlers that render Askama templates
//! for the HTMX-powered frontend.

pub mod auth;
pub mod dashboard;
pub mod downloads;
pub mod movies;
pub mod music;
pub mod settings;
pub mod sse;
pub mod tv;
pub mod utils;

use askama::Template;
use axum::{
    http::{StatusCode, Uri},
    response::IntoResponse,
    routing::get,
    Router,
};

use crate::AppState;

#[derive(Template)]
#[template(path = "pages/404.html")]
pub struct NotFoundTemplate {
    pub path: String,
    pub active_page: String,
}

/// 404 handler
pub async fn not_found(uri: Uri) -> impl IntoResponse {
    (
        StatusCode::NOT_FOUND,
        NotFoundTemplate {
            path: uri.path().to_string(),
            active_page: String::new(),
        },
    )
}

/// Build the HTML routes for the frontend
pub fn routes() -> Router<AppState> {
    Router::new()
        // Public routes
        .route("/login", get(auth::login_page).post(auth::login_submit))
        // Protected routes (auth checked in handlers for now)
        .route("/", get(dashboard::page))
        .route("/logout", axum::routing::post(auth::logout))
        .route("/movies", get(movies::list).post(movies::add_movie))
        .route("/movies/search-modal", get(movies::search_modal))
        .route("/movies/:id", get(movies::detail).delete(movies::delete))
        .route(
            "/movies/:id/search",
            axum::routing::post(movies::search_releases),
        )
        // Note: download is handled via HTMX calling /api/movies/:id/download directly
        .route("/tv", get(tv::list).post(tv::add_show))
        .route("/tv/:id", get(tv::detail).delete(tv::delete))
        .route("/music", get(music::artists).post(music::add_artist))
        .route(
            "/music/artists/:id",
            get(music::artist_detail).delete(music::delete_artist),
        )
        .route("/music/albums/:id", get(music::album_detail))
        .route(
            "/music/albums/:id/search",
            axum::routing::post(music::album_search),
        )
        // Note: downloads are handled via HTMX calling /api/music/albums/:id/unified-download directly
        .route("/downloads", get(downloads::page))
        .route(
            "/downloads/:id/pause",
            axum::routing::post(downloads::pause),
        )
        .route(
            "/downloads/:id/resume",
            axum::routing::post(downloads::resume),
        )
        .route("/downloads/:id", axum::routing::delete(downloads::cancel))
        .route("/settings", get(settings::page))
        // VPN routes
        .route("/vpn/status", get(settings::vpn_status_partial))
        .route("/vpn/connect", axum::routing::post(settings::vpn_connect))
        .route(
            "/vpn/disconnect",
            axum::routing::post(settings::vpn_disconnect),
        )
        // Search modals
        .route("/search/tv/modal", get(tv::search_modal))
        .route("/search/music/modal", get(music::search_modal))
        // Search endpoints (return HTML fragments)
        .route("/search/tmdb/movies", get(movies::search_tmdb))
        .route("/search/tmdb/tv", get(tv::search_tmdb))
        .route("/search/musicbrainz/artists", get(music::search_artists))
        // SSE endpoints
        .route("/sse/downloads", get(sse::downloads_stream))
        .route("/sse/status", get(sse::status_stream))
}
