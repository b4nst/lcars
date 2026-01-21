//! TV shows views - thin wrappers around API handlers
//!
//! These views call the existing API handlers to get data, then render
//! HTML templates. Mutations are handled via HTMX calling the REST API directly.

use askama::Template;
use axum::{
    extract::{Path, Query, State},
    response::{Html, IntoResponse, Redirect},
};
use axum_extra::extract::CookieJar;
use serde::Deserialize;

use axum_extra::extract::Form;

use crate::api::search::search_tmdb_tv;
use crate::api::tv::{
    add_show as api_add_show, get_show as api_get_show, list_shows as api_list_shows,
    AddShowRequest, ListShowsQuery,
};
use crate::db::models::ShowStatus;
use crate::AppState;

use super::auth;

// =============================================================================
// Templates
// =============================================================================

#[derive(Template)]
#[template(path = "pages/tv_list.html")]
pub struct TvListTemplate {
    pub shows: Vec<ShowView>,
    pub total: u64,
    pub current_status: Option<String>,
    pub statuses: Vec<String>,
}

#[derive(Template)]
#[template(path = "pages/tv_detail.html")]
pub struct TvDetailTemplate {
    pub show: ShowView,
    pub seasons: Vec<SeasonView>,
}

#[derive(Template)]
#[template(path = "partials/tmdb_search_results.html")]
pub struct SearchResultsTemplate {
    pub results: Vec<TmdbResult>,
    pub search_type: String,
}

#[derive(Template)]
#[template(path = "components/search_modal.html")]
pub struct SearchModalTemplate {
    pub search_type: String,
    pub search_endpoint: String,
}

// =============================================================================
// View Models
// =============================================================================

pub struct ShowView {
    pub id: i64,
    pub tmdb_id: i64,
    pub title: String,
    pub year_start: Option<i32>,
    pub year_end: Option<i32>,
    pub overview: Option<String>,
    pub poster_path: Option<String>,
    pub backdrop_path: Option<String>,
    pub status: String,
    pub monitored: bool,
}

pub struct SeasonView {
    pub season_number: i32,
    pub episode_count: i32,
    pub available_count: i32,
    pub episodes: Vec<EpisodeView>,
}

pub struct EpisodeView {
    pub id: i64,
    pub episode_number: i32,
    pub title: String,
    pub air_date: Option<String>,
    pub status: String,
    pub monitored: bool,
}

pub struct TmdbResult {
    pub id: i64,
    pub title: String,
    pub year: Option<String>,
    pub overview: Option<String>,
    pub poster_path: Option<String>,
}

// =============================================================================
// Query Types
// =============================================================================

#[derive(Deserialize)]
pub struct ListQuery {
    pub status: Option<String>,
    pub page: Option<u32>,
}

#[derive(Deserialize)]
pub struct SearchQuery {
    pub q: Option<String>,
}

/// Form data for adding a TV show via HTMX
#[derive(Deserialize)]
pub struct AddShowForm {
    pub tmdb_id: i32,
}

// =============================================================================
// View Handlers
// =============================================================================

/// List TV shows page - calls API handler
pub async fn list(
    State(state): State<AppState>,
    cookies: CookieJar,
    Query(query): Query<ListQuery>,
) -> impl IntoResponse {
    if auth::get_current_user(&state, &cookies).await.is_none() {
        return Redirect::to("/login").into_response();
    }

    // Convert status string to ShowStatus enum
    let status = query.status.as_deref().and_then(|s| match s {
        "continuing" => Some(ShowStatus::Continuing),
        "ended" => Some(ShowStatus::Ended),
        "canceled" => Some(ShowStatus::Canceled),
        "upcoming" => Some(ShowStatus::Upcoming),
        _ => None,
    });

    // Call API handler
    let response = api_list_shows(
        State(state),
        axum::extract::Query(ListShowsQuery {
            status,
            monitored: None,
            search: None,
            page: query.page,
            limit: Some(24),
        }),
    )
    .await;

    match response {
        Ok(json) => {
            let data = json.0;
            TvListTemplate {
                shows: data
                    .items
                    .into_iter()
                    .map(|s| ShowView {
                        id: s.id,
                        tmdb_id: s.tmdb_id,
                        title: s.title,
                        year_start: s.year_start,
                        year_end: s.year_end,
                        overview: s.overview,
                        poster_path: s.poster_path,
                        backdrop_path: s.backdrop_path,
                        status: s.status.to_string(),
                        monitored: s.monitored,
                    })
                    .collect(),
                total: data.total,
                current_status: query.status,
                statuses: vec![
                    "all".to_string(),
                    "continuing".to_string(),
                    "ended".to_string(),
                    "canceled".to_string(),
                ],
            }
            .into_response()
        }
        Err(_) => Html("<div class='lcars-error'>Failed to load TV shows</div>").into_response(),
    }
}

/// TV show detail page - calls API handler
pub async fn detail(
    State(state): State<AppState>,
    cookies: CookieJar,
    Path(id): Path<i64>,
) -> impl IntoResponse {
    if auth::get_current_user(&state, &cookies).await.is_none() {
        return Redirect::to("/login").into_response();
    }

    // Call API handler
    let response = api_get_show(State(state), Path(id)).await;

    match response {
        Ok(json) => {
            let data = json.0;
            TvDetailTemplate {
                show: ShowView {
                    id: data.show.id,
                    tmdb_id: data.show.tmdb_id,
                    title: data.show.title,
                    year_start: data.show.year_start,
                    year_end: data.show.year_end,
                    overview: data.show.overview,
                    poster_path: data.show.poster_path,
                    backdrop_path: data.show.backdrop_path,
                    status: data.show.status.to_string(),
                    monitored: data.show.monitored,
                },
                seasons: data
                    .seasons
                    .into_iter()
                    .map(|s| SeasonView {
                        season_number: s.season_number,
                        episode_count: s.total_count,
                        available_count: s.available_count,
                        episodes: s
                            .episodes
                            .into_iter()
                            .map(|e| EpisodeView {
                                id: e.id,
                                episode_number: e.episode_number,
                                title: e.title.unwrap_or_default(),
                                air_date: e.air_date,
                                status: e.status.to_string(),
                                monitored: e.monitored,
                            })
                            .collect(),
                    })
                    .collect(),
            }
            .into_response()
        }
        Err(_) => Html("<div class='lcars-error'>Show not found</div>").into_response(),
    }
}

/// Search modal fragment for TV shows
pub async fn search_modal() -> impl IntoResponse {
    SearchModalTemplate {
        search_type: "tv".to_string(),
        search_endpoint: "/search/tmdb/tv".to_string(),
    }
}

/// Search TMDB for TV shows - calls API handler (returns HTML fragment)
pub async fn search_tmdb(
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
    use crate::api::search::TmdbTvSearchQuery;
    let response =
        search_tmdb_tv(State(state), axum::extract::Query(TmdbTvSearchQuery { q })).await;

    match response {
        Ok(json) => SearchResultsTemplate {
            results: json
                .0
                .into_iter()
                .take(10)
                .map(|r| TmdbResult {
                    id: r.id as i64,
                    title: r.name,
                    year: r.first_air_date.and_then(|d| {
                        if d.len() >= 4 {
                            Some(d[..4].to_string())
                        } else {
                            None
                        }
                    }),
                    overview: r.overview,
                    poster_path: r.poster_path,
                })
                .collect(),
            search_type: "tv".to_string(),
        }
        .into_response(),
        Err(e) => Html(format!(
            "<div class='lcars-error'>Search failed: {}</div>",
            e
        ))
        .into_response(),
    }
}

/// Add a TV show via form submission - calls API handler
pub async fn add_show(
    State(state): State<AppState>,
    cookies: CookieJar,
    Form(form): Form<AddShowForm>,
) -> impl IntoResponse {
    // Get claims from cookies
    let Some(claims) = auth::get_current_user(&state, &cookies).await else {
        return Html("<div class='lcars-error'>Unauthorized</div>").into_response();
    };

    // Call API handler with JSON body
    let response = api_add_show(
        State(state),
        axum::Extension(claims),
        axum::Json(AddShowRequest {
            tmdb_id: form.tmdb_id,
            monitored: Some(true),
            quality_limit: None,
        }),
    )
    .await;

    match response {
        Ok(json) => {
            let show = json.0;
            // Use HX-Redirect header for HTMX client-side redirect
            (
                [(
                    axum::http::header::HeaderName::from_static("hx-redirect"),
                    format!("/tv/{}", show.show.id),
                )],
                Html(""),
            )
                .into_response()
        }
        Err(e) => Html(format!(
            "<div class='lcars-error'>Failed to add TV show: {}</div>",
            e
        ))
        .into_response(),
    }
}
