//! Authentication API endpoints.

use axum::{extract::State, Extension, Json};
use serde::{Deserialize, Serialize};

use crate::db::models::UserRole;
use crate::error::{AppError, Result};
use crate::services::Claims;
use crate::AppState;

/// Dummy hash for timing attack prevention.
/// This is a valid Argon2 hash that will always fail verification.
const DUMMY_HASH: &str =
    "$argon2id$v=19$m=19456,t=2,p=1$dGltaW5nYXR0YWNr$AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA";

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

    let db = state.db.lock().await;
    let auth_service = state.auth_service();

    // Look up user by username
    let user_result = db.query_row(
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
    );

    // Perform constant-time verification to prevent timing attacks
    let (user_id, username, role, authenticated) = match user_result {
        Ok((id, name, hash, r)) => {
            let valid = auth_service.verify_password(&body.password, &hash)?;
            (id, name, r, valid)
        }
        Err(rusqlite::Error::QueryReturnedNoRows) => {
            // Perform dummy verification to prevent timing attack
            let _ = auth_service.verify_password(&body.password, DUMMY_HASH);
            return Err(AppError::Unauthorized);
        }
        Err(e) => return Err(AppError::Sqlite(e)),
    };

    if !authenticated {
        return Err(AppError::Unauthorized);
    }

    // Create JWT token (stateless - no session storage needed)
    let token = auth_service.create_token(user_id, &role.to_string())?;

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
/// Logs out the user. Since we use stateless JWT tokens, this is a no-op
/// on the server side. The client should discard the token.
pub async fn logout(Extension(claims): Extension<Claims>) -> Result<Json<SuccessResponse>> {
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
