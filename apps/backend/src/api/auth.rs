//! Authentication API endpoints.

use axum::{extract::State, Extension, Json};
use serde::{Deserialize, Serialize};

use crate::db::models::UserRole;
use crate::error::{AppError, Result};
use crate::services::Claims;
use crate::AppState;

/// Login request body.
#[derive(Debug, Deserialize)]
pub struct LoginRequest {
    pub username: String,
    pub password: String,
}

/// Login response with JWT token.
#[derive(Debug, Serialize)]
pub struct LoginResponse {
    pub token: String,
    pub user: UserInfo,
}

/// User information returned in responses (without password hash).
#[derive(Debug, Clone, Serialize)]
pub struct UserInfo {
    pub id: i64,
    pub username: String,
    pub role: UserRole,
}

/// Generic success response.
#[derive(Debug, Serialize)]
pub struct SuccessResponse {
    pub message: String,
}

/// POST /api/auth/login
///
/// Authenticates a user and returns a JWT token.
pub async fn login(
    State(state): State<AppState>,
    Json(body): Json<LoginRequest>,
) -> Result<Json<LoginResponse>> {
    // Validate input
    if body.username.is_empty() || body.password.is_empty() {
        return Err(AppError::BadRequest(
            "Username and password are required".to_string(),
        ));
    }

    // Look up user by username
    let db = state.db.lock().await;

    let user = db
        .query_row(
            "SELECT id, username, password_hash, role FROM users WHERE username = ?1",
            [&body.username],
            |row| {
                let role_str: String = row.get(3)?;
                let role = match role_str.as_str() {
                    "admin" => UserRole::Admin,
                    _ => UserRole::User,
                };
                Ok((
                    row.get::<_, i64>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    role,
                ))
            },
        )
        .map_err(|e| match e {
            rusqlite::Error::QueryReturnedNoRows => AppError::Unauthorized,
            _ => AppError::Sqlite(e),
        })?;

    let (user_id, username, password_hash, role) = user;

    // Verify password
    let auth_service = state.auth_service();
    if !auth_service.verify_password(&body.password, &password_hash)? {
        return Err(AppError::Unauthorized);
    }

    // Create JWT token
    let token = auth_service.create_token(user_id, &role.to_string())?;

    // Create session in database
    let token_hash = auth_service.hash_password(&token)?;
    db.execute(
        "INSERT INTO sessions (user_id, token_hash, expires_at)
         VALUES (?1, ?2, datetime('now', '+24 hours'))",
        rusqlite::params![user_id, token_hash],
    )?;

    tracing::info!(user_id = user_id, username = %username, "User logged in");

    Ok(Json(LoginResponse {
        token,
        user: UserInfo {
            id: user_id,
            username,
            role,
        },
    }))
}

/// POST /api/auth/logout
///
/// Invalidates the current session.
pub async fn logout(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
) -> Result<Json<SuccessResponse>> {
    let db = state.db.lock().await;

    // Delete all sessions for this user (simple approach)
    // In production, you might want to invalidate only the current token
    db.execute("DELETE FROM sessions WHERE user_id = ?1", [claims.sub])?;

    tracing::info!(user_id = claims.sub, "User logged out");

    Ok(Json(SuccessResponse {
        message: "Logged out successfully".to_string(),
    }))
}

/// GET /api/auth/me
///
/// Returns the current authenticated user's information.
pub async fn me(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
) -> Result<Json<UserInfo>> {
    let db = state.db.lock().await;

    let user = db
        .query_row(
            "SELECT id, username, role FROM users WHERE id = ?1",
            [claims.sub],
            |row| {
                let role_str: String = row.get(2)?;
                let role = match role_str.as_str() {
                    "admin" => UserRole::Admin,
                    _ => UserRole::User,
                };
                Ok(UserInfo {
                    id: row.get(0)?,
                    username: row.get(1)?,
                    role,
                })
            },
        )
        .map_err(|e| match e {
            rusqlite::Error::QueryReturnedNoRows => {
                AppError::NotFound("User not found".to_string())
            }
            _ => AppError::Sqlite(e),
        })?;

    Ok(Json(user))
}
