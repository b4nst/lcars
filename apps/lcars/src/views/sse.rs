//! Server-Sent Events for real-time updates
//!
//! SSE streams are a special case - they need to continuously stream HTML
//! fragments. We call API handlers internally to get the data, then render
//! it as HTML for HTMX to consume.

use std::convert::Infallible;
use std::pin::Pin;
use std::time::Duration;

use askama::Template;
use async_stream::stream;
use axum::{
    extract::State,
    response::{
        sse::{Event, KeepAlive, Sse},
        IntoResponse, Response,
    },
};
use axum_extra::extract::CookieJar;
use futures::Stream;

use crate::api::downloads::{list_downloads as api_list_downloads, ListDownloadsQuery};
use crate::api::system::get_system_status as api_get_system_status;
use crate::db::models::DownloadStatus;
use crate::AppState;

use super::auth;
use super::downloads::DownloadView;
use super::utils::{format_size, format_speed};

// =============================================================================
// Templates
// =============================================================================

#[derive(Template)]
#[template(path = "partials/download_progress.html")]
pub struct DownloadProgressTemplate {
    pub download: DownloadView,
}

#[derive(Template)]
#[template(path = "partials/status_cards.html")]
pub struct StatusCardsTemplate {
    pub active_downloads: i64,
    pub total_movies: i64,
    pub total_shows: i64,
    pub total_artists: i64,
}

type EventStream = Pin<Box<dyn Stream<Item = Result<Event, Infallible>> + Send>>;

/// SSE stream for download progress updates
pub async fn downloads_stream(State(state): State<AppState>, cookies: CookieJar) -> Response {
    // Check auth - for SSE we can't redirect, so just return empty stream if not authenticated
    if auth::get_current_user(&state, &cookies).await.is_none() {
        let empty_stream: EventStream = Box::pin(stream! {
            yield Ok(Event::default().event("error").data("Unauthorized"));
        });
        return Sse::new(empty_stream)
            .keep_alive(KeepAlive::default())
            .into_response();
    }

    let stream: EventStream = Box::pin(stream! {
        let mut interval = tokio::time::interval(Duration::from_secs(2));

        loop {
            interval.tick().await;

            // Call API to get downloads
            let response = api_list_downloads(
                State(state.clone()),
                axum::extract::Query(ListDownloadsQuery {
                    status: Some("downloading".to_string()),
                    source: None,
                }),
            )
            .await;

            if let Ok(json) = response {
                for d in json.0 {
                    if d.status != DownloadStatus::Downloading {
                        continue;
                    }

                    let view = DownloadView {
                        id: d.id,
                        name: d.name,
                        media_type: d.media_type.to_string(),
                        media_id: d.media_id,
                        status: d.status.to_string(),
                        progress: d.progress,
                        progress_percent: format!("{:.1}%", d.progress * 100.0),
                        download_speed: format_speed(d.download_speed as u64),
                        upload_speed: format_speed(d.upload_speed as u64),
                        size_display: format_size(d.size_bytes.unwrap_or(0) as u64),
                        downloaded_display: format_size(d.downloaded_bytes as u64),
                        peers: d.peers,
                        error_message: d.error_message,
                    };

                    let template = DownloadProgressTemplate { download: view };
                    if let Ok(html) = template.render() {
                        yield Ok(
                            Event::default()
                                .event(format!("download-{}", d.id))
                                .data(html)
                        );
                    }
                }
            }
        }
    });

    Sse::new(stream)
        .keep_alive(
            KeepAlive::new()
                .interval(Duration::from_secs(15))
                .text("keep-alive"),
        )
        .into_response()
}

/// SSE stream for system status updates
pub async fn status_stream(State(state): State<AppState>, cookies: CookieJar) -> Response {
    if auth::get_current_user(&state, &cookies).await.is_none() {
        let empty_stream: EventStream = Box::pin(stream! {
            yield Ok(Event::default().event("error").data("Unauthorized"));
        });
        return Sse::new(empty_stream)
            .keep_alive(KeepAlive::default())
            .into_response();
    }

    let stream: EventStream = Box::pin(stream! {
        let mut interval = tokio::time::interval(Duration::from_secs(5));

        loop {
            interval.tick().await;

            // Get download stats from API
            let active_downloads = api_get_system_status(State(state.clone()))
                .await
                .map(|r| r.0.downloads.active)
                .unwrap_or(0);

            // Get library counts directly from DB (not exposed via API)
            let (total_movies, total_shows, total_artists) = {
                let db = state.db.lock().await;
                let movies = db
                    .query_row("SELECT COUNT(*) FROM movies", [], |row| row.get::<_, i64>(0))
                    .unwrap_or(0);
                let shows = db
                    .query_row("SELECT COUNT(*) FROM tv_shows", [], |row| row.get::<_, i64>(0))
                    .unwrap_or(0);
                let artists = db
                    .query_row("SELECT COUNT(*) FROM artists", [], |row| row.get::<_, i64>(0))
                    .unwrap_or(0);
                (movies, shows, artists)
            };

            let template = StatusCardsTemplate {
                active_downloads,
                total_movies,
                total_shows,
                total_artists,
            };

            if let Ok(html) = template.render() {
                yield Ok(
                    Event::default()
                        .event("status")
                        .data(html)
                );
            }
        }
    });

    Sse::new(stream)
        .keep_alive(
            KeepAlive::new()
                .interval(Duration::from_secs(15))
                .text("keep-alive"),
        )
        .into_response()
}

