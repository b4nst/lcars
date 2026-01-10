//! Application error types for the LCARS backend.
//!
//! Provides a unified error type that implements `IntoResponse` for Axum.

#![allow(dead_code)]

use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde::Serialize;
use thiserror::Error;

use crate::db::DbError;

/// Application-wide error type
#[derive(Error, Debug)]
pub enum AppError {
    /// Database-related errors
    #[error("Database error: {0}")]
    Database(#[from] DbError),

    /// SQLite-specific errors (for direct rusqlite usage)
    #[error("Database error: {0}")]
    Sqlite(#[from] rusqlite::Error),

    /// Configuration loading/parsing errors
    #[error("Configuration error: {0}")]
    Config(#[from] config::ConfigError),

    /// Resource not found
    #[error("Not found: {0}")]
    NotFound(String),

    /// Authentication required
    #[error("Unauthorized")]
    Unauthorized,

    /// Insufficient permissions
    #[error("Forbidden")]
    Forbidden,

    /// Invalid request data
    #[error("Bad request: {0}")]
    BadRequest(String),

    /// Internal server error
    #[error("Internal error: {0}")]
    Internal(String),
}

/// JSON error response body
#[derive(Serialize)]
struct ErrorResponse {
    error: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    message: Option<String>,
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let (status, error, message) = match &self {
            AppError::Database(e) => {
                // Log full error details but don't expose to client
                tracing::error!("Database error: {:?}", e);
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "database_error",
                    None, // Don't expose internal DB errors to clients
                )
            }
            AppError::Sqlite(e) => {
                // Log full error details but don't expose to client
                tracing::error!("SQLite error: {:?}", e);
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "database_error",
                    None, // Don't expose internal DB errors to clients
                )
            }
            AppError::Config(e) => {
                // Log full error details but don't expose to client
                tracing::error!("Config error: {:?}", e);
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "configuration_error",
                    None, // Don't expose config errors to clients
                )
            }
            AppError::NotFound(resource) => {
                (StatusCode::NOT_FOUND, "not_found", Some(resource.clone()))
            }
            AppError::Unauthorized => (StatusCode::UNAUTHORIZED, "unauthorized", None),
            AppError::Forbidden => (StatusCode::FORBIDDEN, "forbidden", None),
            AppError::BadRequest(msg) => {
                // Bad request messages are safe to expose (client-caused errors)
                (StatusCode::BAD_REQUEST, "bad_request", Some(msg.clone()))
            }
            AppError::Internal(msg) => {
                // Log full error but don't expose internal details
                tracing::error!("Internal error: {}", msg);
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "internal_error",
                    None, // Don't expose internal errors to clients
                )
            }
        };

        let body = ErrorResponse {
            error: error.to_string(),
            message,
        };

        (status, Json(body)).into_response()
    }
}

/// Result type alias for handlers
pub type Result<T> = std::result::Result<T, AppError>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_not_found_status() {
        let error = AppError::NotFound("test".to_string());
        let response = error.into_response();
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }

    #[test]
    fn test_unauthorized_status() {
        let error = AppError::Unauthorized;
        let response = error.into_response();
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }

    #[test]
    fn test_bad_request_status() {
        let error = AppError::BadRequest("invalid".to_string());
        let response = error.into_response();
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }

    #[test]
    fn test_forbidden_status() {
        let error = AppError::Forbidden;
        let response = error.into_response();
        assert_eq!(response.status(), StatusCode::FORBIDDEN);
    }
}
