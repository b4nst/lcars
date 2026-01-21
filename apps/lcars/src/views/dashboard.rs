//! Dashboard view

use askama::Template;
use axum::{
    extract::State,
    response::{IntoResponse, Redirect},
};
use axum_extra::extract::CookieJar;

use crate::api::movies::{list_movies, ListMoviesQuery};
use crate::api::system::get_system_status;
use crate::AppState;

use super::auth;

#[derive(Template)]
#[template(path = "pages/dashboard.html")]
pub struct DashboardTemplate {
    pub version: String,
    pub uptime_seconds: u64,
    pub active_downloads: i64,
    pub total_movies: i64,
    pub total_shows: i64,
    pub total_artists: i64,
    pub recent_movies: Vec<MovieSummary>,
}

pub struct MovieSummary {
    pub id: i64,
    pub title: String,
    pub year: Option<i32>,
    pub poster_path: Option<String>,
    pub status: String,
}

/// Render the dashboard page
pub async fn page(State(state): State<AppState>, cookies: CookieJar) -> impl IntoResponse {
    // Check authentication
    if auth::get_current_user(&state, &cookies).await.is_none() {
        return Redirect::to("/login").into_response();
    }

    // Get system status from API
    let status = get_system_status(State(state.clone()))
        .await
        .map(|r| r.0)
        .ok();

    // Get recent movies from API
    let recent_movies_response = list_movies(
        State(state.clone()),
        axum::extract::Query(ListMoviesQuery {
            status: None,
            monitored: None,
            search: None,
            page: Some(1),
            limit: Some(6),
        }),
    )
    .await
    .ok();

    // Get counts from database (these aren't exposed via API, keeping direct query for now)
    let (total_movies, total_shows, total_artists) = {
        let db = state.db.lock().await;
        let total_movies: i64 = db
            .query_row("SELECT COUNT(*) FROM movies", [], |row| row.get(0))
            .unwrap_or(0);
        let total_shows: i64 = db
            .query_row("SELECT COUNT(*) FROM tv_shows", [], |row| row.get(0))
            .unwrap_or(0);
        let total_artists: i64 = db
            .query_row("SELECT COUNT(*) FROM artists", [], |row| row.get(0))
            .unwrap_or(0);
        (total_movies, total_shows, total_artists)
    };

    let recent_movies: Vec<MovieSummary> = recent_movies_response
        .map(|r| {
            r.0.items
                .into_iter()
                .map(|m| MovieSummary {
                    id: m.id,
                    title: m.title,
                    year: Some(m.year),
                    poster_path: m.poster_path,
                    status: m.status.to_string(),
                })
                .collect()
        })
        .unwrap_or_default();

    DashboardTemplate {
        version: status
            .as_ref()
            .map(|s| s.version.clone())
            .unwrap_or_else(|| env!("CARGO_PKG_VERSION").to_string()),
        uptime_seconds: status.as_ref().map(|s| s.uptime_seconds).unwrap_or(0),
        active_downloads: status.as_ref().map(|s| s.downloads.active).unwrap_or(0),
        total_movies,
        total_shows,
        total_artists,
        recent_movies,
    }
    .into_response()
}
