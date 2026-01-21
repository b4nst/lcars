//! Movies views - thin wrappers around API handlers

use askama::Template;
use axum::{
    extract::{Path, Query, State},
    http::HeaderMap,
    response::{Html, IntoResponse, Redirect},
};
use axum_extra::extract::CookieJar;
use serde::Deserialize;

use axum_extra::extract::Form;

use crate::api::movies::{
    add_movie as api_add_movie, delete_movie as api_delete_movie, get_movie, list_movies,
    search_releases as api_search_releases, AddMovieRequest, DeleteMovieQuery, ListMoviesQuery,
};
use crate::api::search::search_tmdb_movies;
use crate::db::models::MediaStatus;
use crate::response::ContentNegotiation;
use crate::AppState;

use super::auth;
use super::utils::format_size;

#[derive(Template)]
#[template(path = "pages/movies_list.html")]
pub struct MoviesListTemplate {
    pub movies: Vec<MovieView>,
    pub total: u64,
    pub current_status: Option<String>,
    pub statuses: Vec<String>,
}

#[derive(Template)]
#[template(path = "partials/movies_content.html")]
pub struct MoviesContentTemplate {
    pub movies: Vec<MovieView>,
    pub current_status: Option<String>,
    pub statuses: Vec<String>,
}

#[derive(Template)]
#[template(path = "pages/movie_detail.html")]
pub struct MovieDetailTemplate {
    pub movie: MovieView,
    pub releases: Vec<ReleaseView>,
    pub media_type: String,
    pub media_id: i64,
}

#[derive(Template)]
#[template(path = "components/search_modal.html")]
pub struct SearchModalTemplate {
    pub search_type: String,
    pub search_endpoint: String,
}

#[derive(Template)]
#[template(path = "partials/tmdb_search_results.html")]
pub struct SearchResultsTemplate {
    pub results: Vec<TmdbResult>,
    pub search_type: String,
}

#[derive(Template)]
#[template(path = "partials/releases_list.html")]
pub struct ReleasesListTemplate {
    pub releases: Vec<ReleaseView>,
    pub media_type: String,
    pub media_id: i64,
}

pub struct MovieView {
    pub id: i64,
    pub tmdb_id: i64,
    pub title: String,
    pub year: Option<i32>,
    pub overview: Option<String>,
    pub poster_path: Option<String>,
    pub backdrop_path: Option<String>,
    pub runtime_minutes: Option<i32>,
    pub status: String,
    pub monitored: bool,
    pub file_path: Option<String>,
}

pub struct ReleaseView {
    pub id: String,
    pub title: String,
    pub indexer: String,
    pub size_bytes: u64,
    pub size_display: String,
    pub seeders: u32,
    pub leechers: u32,
    pub quality: String,
    pub source: String,
    pub magnet: String,
}

pub struct TmdbResult {
    pub id: i64,
    pub title: String,
    pub year: Option<String>,
    pub overview: Option<String>,
    pub poster_path: Option<String>,
}

#[derive(Deserialize)]
pub struct ListQuery {
    pub status: Option<String>,
    pub page: Option<u32>,
}

#[derive(Deserialize)]
pub struct SearchQuery {
    pub q: Option<String>,
    pub year: Option<i32>,
}

/// Form data for adding a movie via HTMX
#[derive(Deserialize)]
pub struct AddMovieForm {
    pub tmdb_id: i32,
}

/// List movies page - calls API handler
pub async fn list(
    State(state): State<AppState>,
    headers: HeaderMap,
    cookies: CookieJar,
    Query(query): Query<ListQuery>,
) -> impl IntoResponse {
    if auth::get_current_user(&state, &cookies).await.is_none() {
        return Redirect::to("/login").into_response();
    }

    // Convert status string to MediaStatus enum
    let status = query.status.as_deref().and_then(|s| match s {
        "missing" => Some(MediaStatus::Missing),
        "downloading" => Some(MediaStatus::Downloading),
        "available" => Some(MediaStatus::Available),
        _ => None,
    });

    // Call API handler
    let response = list_movies(
        State(state),
        axum::extract::Query(ListMoviesQuery {
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
            let movies: Vec<MovieView> = data
                .items
                .into_iter()
                .map(|m| MovieView {
                    id: m.id,
                    tmdb_id: m.tmdb_id,
                    title: m.title,
                    year: Some(m.year),
                    overview: m.overview,
                    poster_path: m.poster_path,
                    backdrop_path: m.backdrop_path,
                    runtime_minutes: m.runtime_minutes,
                    status: m.status.to_string(),
                    monitored: m.monitored,
                    file_path: m.file_path,
                })
                .collect();
            let statuses = vec![
                "all".to_string(),
                "missing".to_string(),
                "downloading".to_string(),
                "available".to_string(),
            ];

            // Return partial for HTMX requests, full page otherwise
            if headers.is_htmx() {
                MoviesContentTemplate {
                    movies,
                    current_status: query.status,
                    statuses,
                }
                .into_response()
            } else {
                MoviesListTemplate {
                    movies,
                    total: data.total,
                    current_status: query.status,
                    statuses,
                }
                .into_response()
            }
        }
        Err(_) => Html("<div class='lcars-error'>Failed to load movies</div>").into_response(),
    }
}

/// Movie detail page - calls API handler
pub async fn detail(
    State(state): State<AppState>,
    cookies: CookieJar,
    Path(id): Path<i64>,
) -> impl IntoResponse {
    if auth::get_current_user(&state, &cookies).await.is_none() {
        return Redirect::to("/login").into_response();
    }

    // Call API handler
    let response = get_movie(State(state), Path(id)).await;

    match response {
        Ok(json) => {
            let m = json.0;
            MovieDetailTemplate {
                movie: MovieView {
                    id: m.id,
                    tmdb_id: m.tmdb_id,
                    title: m.title,
                    year: Some(m.year),
                    overview: m.overview,
                    poster_path: m.poster_path,
                    backdrop_path: m.backdrop_path,
                    runtime_minutes: m.runtime_minutes,
                    status: m.status.to_string(),
                    monitored: m.monitored,
                    file_path: m.file_path,
                },
                releases: vec![],
                media_type: "movie".to_string(),
                media_id: m.id,
            }
            .into_response()
        }
        Err(_) => Html("<div class='lcars-error'>Movie not found</div>").into_response(),
    }
}

/// Search modal fragment
pub async fn search_modal() -> impl IntoResponse {
    SearchModalTemplate {
        search_type: "movies".to_string(),
        search_endpoint: "/search/tmdb/movies".to_string(),
    }
}

/// Search TMDB for movies - calls API handler
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
    use crate::api::search::TmdbMovieSearchQuery;
    let response = search_tmdb_movies(
        State(state),
        axum::extract::Query(TmdbMovieSearchQuery {
            q,
            year: query.year,
        }),
    )
    .await;

    match response {
        Ok(json) => SearchResultsTemplate {
            results: json
                .0
                .into_iter()
                .take(10)
                .map(|r| TmdbResult {
                    id: r.id as i64,
                    title: r.title,
                    year: r
                        .release_date
                        .map(|d| if d.len() >= 4 { d[..4].to_string() } else { d }),
                    overview: r.overview,
                    poster_path: r.poster_path,
                })
                .collect(),
            search_type: "movies".to_string(),
        }
        .into_response(),
        Err(e) => Html(format!(
            "<div class='lcars-error'>Search failed: {}</div>",
            e
        ))
        .into_response(),
    }
}

/// Search for releases - calls API handler
pub async fn search_releases(
    State(state): State<AppState>,
    cookies: CookieJar,
    Path(id): Path<i64>,
) -> impl IntoResponse {
    if auth::get_current_user(&state, &cookies).await.is_none() {
        return Html("<div class='lcars-error'>Unauthorized</div>").into_response();
    }

    // Call API handler
    let response = api_search_releases(State(state), Path(id)).await;

    match response {
        Ok(json) => ReleasesListTemplate {
            releases: json
                .0
                .into_iter()
                .map(|r| ReleaseView {
                    id: r.id,
                    title: r.title,
                    indexer: r.indexer,
                    size_display: format_size(r.size_bytes),
                    size_bytes: r.size_bytes,
                    seeders: r.seeders,
                    leechers: r.leechers,
                    quality: r.quality.to_string(),
                    source: r.source.to_string(),
                    magnet: r.magnet,
                })
                .collect(),
            media_type: "movie".to_string(),
            media_id: id,
        }
        .into_response(),
        Err(_) => Html("<div class='lcars-error'>Failed to search releases</div>").into_response(),
    }
}


/// Add a movie via form submission - calls API handler
pub async fn add_movie(
    State(state): State<AppState>,
    cookies: CookieJar,
    Form(form): Form<AddMovieForm>,
) -> impl IntoResponse {
    // Get claims from cookies
    let Some(claims) = auth::get_current_user(&state, &cookies).await else {
        return Html("<div class='lcars-error'>Unauthorized</div>").into_response();
    };

    // Call API handler with JSON body
    let response = api_add_movie(
        State(state),
        axum::Extension(claims),
        axum::Json(AddMovieRequest {
            tmdb_id: form.tmdb_id,
            monitored: Some(true),
            quality_limit: None,
        }),
    )
    .await;

    match response {
        Ok(json) => {
            let movie = json.0;
            // Use HX-Redirect header for HTMX client-side redirect
            (
                [(
                    axum::http::header::HeaderName::from_static("hx-redirect"),
                    format!("/movies/{}", movie.id),
                )],
                Html(""),
            )
                .into_response()
        }
        Err(e) => Html(format!(
            "<div class='lcars-error'>Failed to add movie: {}</div>",
            e
        ))
        .into_response(),
    }
}

/// Delete a movie - calls API handler
pub async fn delete(
    State(state): State<AppState>,
    cookies: CookieJar,
    Path(id): Path<i64>,
) -> impl IntoResponse {
    if auth::get_current_user(&state, &cookies).await.is_none() {
        return Html("<div class='lcars-error'>Unauthorized</div>").into_response();
    }

    // Call API handler
    let response = api_delete_movie(
        State(state),
        Path(id),
        axum::extract::Query(DeleteMovieQuery {
            delete_files: Some(false),
        }),
    )
    .await;

    match response {
        Ok(_) => {
            // Return HX-Redirect header to redirect to movies list
            (
                [(
                    axum::http::header::HeaderName::from_static("hx-redirect"),
                    "/movies",
                )],
                Html(""),
            )
                .into_response()
        }
        Err(e) => Html(format!(
            "<div class='lcars-error'>Failed to delete movie: {}</div>",
            e
        ))
        .into_response(),
    }
}
