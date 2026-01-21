//! Authentication middleware for LCARS.
//!
//! Provides JWT validation and role-based access control.

use axum::{
    body::Body,
    extract::State,
    http::{header::AUTHORIZATION, Request},
    middleware::Next,
    response::Response,
};

use crate::error::{AppError, Result};
use crate::services::auth::Claims;
use crate::AppState;

/// Extracts the Bearer token from the Authorization header.
fn extract_bearer_token(request: &Request<Body>) -> Option<&str> {
    request
        .headers()
        .get(AUTHORIZATION)?
        .to_str()
        .ok()?
        .strip_prefix("Bearer ")
}

/// Authentication middleware that validates JWT tokens.
///
/// Extracts the Bearer token from the Authorization header, validates it,
/// and adds the claims to the request extensions.
pub async fn auth_middleware(
    State(state): State<AppState>,
    mut request: Request<Body>,
    next: Next,
) -> Result<Response> {
    let token = extract_bearer_token(&request).ok_or(AppError::Unauthorized)?;

    let auth_service = state.auth_service();
    let claims = auth_service.verify_token(token)?;

    // Add claims to request extensions for downstream handlers
    request.extensions_mut().insert(claims);

    Ok(next.run(request).await)
}

/// Middleware that requires admin role.
///
/// Must be used after `auth_middleware` to ensure claims are present.
pub async fn require_admin(request: Request<Body>, next: Next) -> Result<Response> {
    let claims = request
        .extensions()
        .get::<Claims>()
        .ok_or(AppError::Unauthorized)?;

    if claims.role != "admin" {
        return Err(AppError::Forbidden);
    }

    Ok(next.run(request).await)
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::Request;

    #[test]
    fn test_extract_bearer_token_valid() {
        let request = Request::builder()
            .header(AUTHORIZATION, "Bearer my-token-123")
            .body(Body::empty())
            .unwrap();

        let token = extract_bearer_token(&request);
        assert_eq!(token, Some("my-token-123"));
    }

    #[test]
    fn test_extract_bearer_token_missing_header() {
        let request = Request::builder().body(Body::empty()).unwrap();

        let token = extract_bearer_token(&request);
        assert_eq!(token, None);
    }

    #[test]
    fn test_extract_bearer_token_wrong_scheme() {
        let request = Request::builder()
            .header(AUTHORIZATION, "Basic dXNlcjpwYXNz")
            .body(Body::empty())
            .unwrap();

        let token = extract_bearer_token(&request);
        assert_eq!(token, None);
    }

    #[test]
    fn test_extract_bearer_token_empty() {
        let request = Request::builder()
            .header(AUTHORIZATION, "Bearer ")
            .body(Body::empty())
            .unwrap();

        let token = extract_bearer_token(&request);
        assert_eq!(token, Some(""));
    }
}
