//! Content negotiation for API responses.
//!
//! This module provides utilities for returning either JSON or HTML from the same
//! API endpoint based on the `Accept` header. This allows HTMX to request HTML
//! directly from API endpoints while other clients get JSON.
//!
//! # Example
//!
//! ```rust,ignore
//! use crate::response::{Negotiate, negotiate};
//!
//! #[derive(Template)]
//! #[template(path = "pages/movies_list.html")]
//! struct MoviesListTemplate {
//!     movies: Vec<MovieView>,
//! }
//!
//! async fn list_movies(
//!     headers: HeaderMap,
//!     // ... other extractors
//! ) -> Result<Negotiate<Json<PaginatedResponse<Movie>>, MoviesListTemplate>> {
//!     let data = fetch_movies().await?;
//!
//!     Ok(negotiate(
//!         &headers,
//!         || Json(data.clone()),
//!         || MoviesListTemplate { movies: data.into_iter().map(Into::into).collect() },
//!     ))
//! }
//! ```

use askama::Template;
use axum::{
    http::{header::ACCEPT, HeaderMap},
    response::{IntoResponse, Response},
    Json,
};
use serde::Serialize;

/// A response that can be either JSON or HTML based on content negotiation.
pub enum Negotiate<J, H> {
    Json(J),
    Html(H),
}

impl<J, H> IntoResponse for Negotiate<J, H>
where
    J: IntoResponse,
    H: Template,
{
    fn into_response(self) -> Response {
        match self {
            Negotiate::Json(json) => json.into_response(),
            Negotiate::Html(template) => match template.render() {
                Ok(html) => axum::response::Html(html).into_response(),
                Err(e) => {
                    tracing::error!("Template render error: {}", e);
                    (
                        axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                        "Template render error",
                    )
                        .into_response()
                }
            },
        }
    }
}

/// Check if the request prefers HTML based on the Accept header.
///
/// Returns true if:
/// - Accept header contains "text/html"
/// - Request has "HX-Request" header (HTMX request)
pub fn prefers_html(headers: &HeaderMap) -> bool {
    // HTMX always wants HTML
    if headers.contains_key("hx-request") {
        return true;
    }

    // Check Accept header
    headers
        .get(ACCEPT)
        .and_then(|v| v.to_str().ok())
        .map(|accept| accept.contains("text/html"))
        .unwrap_or(false)
}

/// Create a negotiated response based on the Accept header.
///
/// - If client prefers HTML (or is HTMX), calls `html_fn` to get the HTML template
/// - Otherwise, calls `json_fn` to get the JSON response
///
/// Both closures are used to avoid computing both responses when only one is needed.
pub fn negotiate<J, H, FJ, FH>(headers: &HeaderMap, json_fn: FJ, html_fn: FH) -> Negotiate<J, H>
where
    FJ: FnOnce() -> J,
    FH: FnOnce() -> H,
{
    if prefers_html(headers) {
        Negotiate::Html(html_fn())
    } else {
        Negotiate::Json(json_fn())
    }
}

/// Simpler negotiation when you already have both values.
pub fn negotiate_response<J, H>(headers: &HeaderMap, json: J, html: H) -> Negotiate<J, H> {
    if prefers_html(headers) {
        Negotiate::Html(html)
    } else {
        Negotiate::Json(json)
    }
}

/// Extension trait for HeaderMap to check content preferences.
pub trait ContentNegotiation {
    fn prefers_html(&self) -> bool;
    fn is_htmx(&self) -> bool;
}

impl ContentNegotiation for HeaderMap {
    fn prefers_html(&self) -> bool {
        prefers_html(self)
    }

    fn is_htmx(&self) -> bool {
        self.contains_key("hx-request")
    }
}

// =============================================================================
// Convenience types for common patterns
// =============================================================================

/// A JSON response that can also be rendered as HTML.
///
/// Use this when the JSON and HTML data structures are the same
/// (template struct implements both Serialize and Template).
pub enum JsonOrHtml<T> {
    Json(T),
    Html(T),
}

impl<T> IntoResponse for JsonOrHtml<T>
where
    T: Serialize + Template,
{
    fn into_response(self) -> Response {
        match self {
            JsonOrHtml::Json(data) => Json(data).into_response(),
            JsonOrHtml::Html(template) => match template.render() {
                Ok(html) => axum::response::Html(html).into_response(),
                Err(e) => {
                    tracing::error!("Template render error: {}", e);
                    (
                        axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                        "Template render error",
                    )
                        .into_response()
                }
            },
        }
    }
}

impl<T> JsonOrHtml<T> {
    pub fn negotiate(headers: &HeaderMap, data: T) -> Self {
        if prefers_html(headers) {
            JsonOrHtml::Html(data)
        } else {
            JsonOrHtml::Json(data)
        }
    }
}
