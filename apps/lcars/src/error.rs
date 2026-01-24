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

    /// External service unavailable (TMDB, MusicBrainz, indexers)
    #[error("Service unavailable: {0}")]
    ServiceUnavailable(String),

    /// Rate limited by external service
    #[error("Rate limited: retry after {0} seconds")]
    RateLimited(u32),

    /// Request timeout
    #[error("Request timeout: {0}")]
    Timeout(String),

    /// Conflict (e.g., duplicate resource)
    #[error("Conflict: {0}")]
    Conflict(String),

    /// VPN-related errors
    #[error("VPN error: {0}")]
    Vpn(String),

    /// VPN not configured or enabled
    #[error("VPN not configured")]
    VpnNotConfigured,

    /// Operation blocked by VPN kill switch
    #[error("VPN kill switch active")]
    VpnKillSwitch,

    /// DNS management errors
    #[error("DNS error: {0}")]
    Dns(String),
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
            AppError::ServiceUnavailable(service) => {
                tracing::warn!("Service unavailable: {}", service);
                (
                    StatusCode::SERVICE_UNAVAILABLE,
                    "service_unavailable",
                    Some(format!("{} is temporarily unavailable", service)),
                )
            }
            AppError::RateLimited(retry_after) => {
                tracing::warn!("Rate limited, retry after {} seconds", retry_after);
                (
                    StatusCode::TOO_MANY_REQUESTS,
                    "rate_limited",
                    Some(format!("Rate limited. Retry after {} seconds", retry_after)),
                )
            }
            AppError::Timeout(operation) => {
                tracing::warn!("Timeout during: {}", operation);
                (
                    StatusCode::GATEWAY_TIMEOUT,
                    "timeout",
                    Some(format!("{} timed out", operation)),
                )
            }
            AppError::Conflict(msg) => (StatusCode::CONFLICT, "conflict", Some(msg.clone())),
            AppError::Vpn(msg) => {
                // Log full error details but don't expose to client
                tracing::error!("VPN error: {}", msg);
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "vpn_error",
                    None, // Don't expose internal VPN errors to clients
                )
            }
            AppError::VpnNotConfigured => {
                tracing::warn!("VPN not configured");
                (
                    StatusCode::SERVICE_UNAVAILABLE,
                    "vpn_not_configured",
                    Some("VPN is not configured or enabled".to_string()),
                )
            }
            AppError::VpnKillSwitch => {
                tracing::warn!("VPN kill switch active");
                (
                    StatusCode::SERVICE_UNAVAILABLE,
                    "vpn_kill_switch",
                    Some(
                        "Operation blocked: VPN is disconnected and kill switch is active"
                            .to_string(),
                    ),
                )
            }
            AppError::Dns(msg) => {
                // Log full error details but don't expose to client
                tracing::error!("DNS error: {}", msg);
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "dns_error",
                    None, // Don't expose internal DNS errors to clients
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

    #[test]
    fn test_service_unavailable_status() {
        let error = AppError::ServiceUnavailable("TMDB".to_string());
        let response = error.into_response();
        assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
    }

    #[test]
    fn test_rate_limited_status() {
        let error = AppError::RateLimited(60);
        let response = error.into_response();
        assert_eq!(response.status(), StatusCode::TOO_MANY_REQUESTS);
    }

    #[test]
    fn test_timeout_status() {
        let error = AppError::Timeout("metadata lookup".to_string());
        let response = error.into_response();
        assert_eq!(response.status(), StatusCode::GATEWAY_TIMEOUT);
    }

    #[test]
    fn test_conflict_status() {
        let error = AppError::Conflict("Movie already exists".to_string());
        let response = error.into_response();
        assert_eq!(response.status(), StatusCode::CONFLICT);
    }
}
