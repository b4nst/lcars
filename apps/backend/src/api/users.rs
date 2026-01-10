//! Users API endpoints (admin only).

use axum::{
    extract::{Path, State},
    Json,
};
use serde::{Deserialize, Serialize};

use crate::api::auth::SuccessResponse;
use crate::db::models::UserRole;
use crate::error::{AppError, Result};
use crate::AppState;

/// Create user request body.
#[derive(Debug, Deserialize)]
pub struct CreateUserRequest {
    pub username: String,
    pub password: String,
    pub role: UserRole,
}

/// Update user request body.
#[derive(Debug, Deserialize)]
pub struct UpdateUserRequest {
    pub username: Option<String>,
    pub password: Option<String>,
    pub role: Option<UserRole>,
}

/// User response with timestamps.
#[derive(Debug, Serialize)]
pub struct UserResponse {
    pub id: i64,
    pub username: String,
    pub role: UserRole,
    pub created_at: String,
    pub updated_at: String,
}

/// GET /api/users
///
/// Lists all users (admin only).
pub async fn list_users(State(state): State<AppState>) -> Result<Json<Vec<UserResponse>>> {
    let db = state.db.lock().await;

    let mut stmt =
        db.prepare("SELECT id, username, role, created_at, updated_at FROM users ORDER BY id")?;

    let users = stmt
        .query_map([], |row| {
            let role_str: String = row.get(2)?;
            let role = match role_str.as_str() {
                "admin" => UserRole::Admin,
                _ => UserRole::User,
            };
            Ok(UserResponse {
                id: row.get(0)?,
                username: row.get(1)?,
                role,
                created_at: row.get(3)?,
                updated_at: row.get(4)?,
            })
        })?
        .collect::<std::result::Result<Vec<_>, _>>()?;

    Ok(Json(users))
}

/// POST /api/users
///
/// Creates a new user (admin only).
pub async fn create_user(
    State(state): State<AppState>,
    Json(body): Json<CreateUserRequest>,
) -> Result<Json<UserResponse>> {
    // Validate input
    if body.username.is_empty() {
        return Err(AppError::BadRequest("Username is required".to_string()));
    }
    if body.password.len() < 8 {
        return Err(AppError::BadRequest(
            "Password must be at least 8 characters".to_string(),
        ));
    }

    let auth_service = state.auth_service();
    let password_hash = auth_service.hash_password(&body.password)?;

    let db = state.db.lock().await;

    // Check if username already exists
    let exists: bool = db.query_row(
        "SELECT EXISTS(SELECT 1 FROM users WHERE username = ?1)",
        [&body.username],
        |row| row.get(0),
    )?;

    if exists {
        return Err(AppError::BadRequest("Username already exists".to_string()));
    }

    db.execute(
        "INSERT INTO users (username, password_hash, role) VALUES (?1, ?2, ?3)",
        rusqlite::params![body.username, password_hash, body.role.to_string()],
    )?;

    let user_id = db.last_insert_rowid();

    let user = db.query_row(
        "SELECT id, username, role, created_at, updated_at FROM users WHERE id = ?1",
        [user_id],
        |row| {
            let role_str: String = row.get(2)?;
            let role = match role_str.as_str() {
                "admin" => UserRole::Admin,
                _ => UserRole::User,
            };
            Ok(UserResponse {
                id: row.get(0)?,
                username: row.get(1)?,
                role,
                created_at: row.get(3)?,
                updated_at: row.get(4)?,
            })
        },
    )?;

    tracing::info!(user_id = user.id, username = %user.username, "User created");

    Ok(Json(user))
}

/// PUT /api/users/:id
///
/// Updates a user (admin only).
pub async fn update_user(
    State(state): State<AppState>,
    Path(user_id): Path<i64>,
    Json(body): Json<UpdateUserRequest>,
) -> Result<Json<UserResponse>> {
    let db = state.db.lock().await;

    // Check if user exists
    let exists: bool = db.query_row(
        "SELECT EXISTS(SELECT 1 FROM users WHERE id = ?1)",
        [user_id],
        |row| row.get(0),
    )?;

    if !exists {
        return Err(AppError::NotFound("User not found".to_string()));
    }

    // Build update query dynamically
    let mut updates = Vec::new();
    let mut params: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();

    if let Some(ref username) = body.username {
        if username.is_empty() {
            return Err(AppError::BadRequest("Username cannot be empty".to_string()));
        }
        // Check if new username already taken by another user
        let taken: bool = db.query_row(
            "SELECT EXISTS(SELECT 1 FROM users WHERE username = ?1 AND id != ?2)",
            rusqlite::params![username, user_id],
            |row| row.get(0),
        )?;
        if taken {
            return Err(AppError::BadRequest("Username already exists".to_string()));
        }
        updates.push("username = ?");
        params.push(Box::new(username.clone()));
    }

    if let Some(ref password) = body.password {
        if password.len() < 8 {
            return Err(AppError::BadRequest(
                "Password must be at least 8 characters".to_string(),
            ));
        }
        let auth_service = state.auth_service();
        let password_hash = auth_service.hash_password(password)?;
        updates.push("password_hash = ?");
        params.push(Box::new(password_hash));
    }

    if let Some(ref role) = body.role {
        updates.push("role = ?");
        params.push(Box::new(role.to_string()));
    }

    if updates.is_empty() {
        return Err(AppError::BadRequest("No fields to update".to_string()));
    }

    // Add updated_at and user_id
    updates.push("updated_at = datetime('now')");
    let query = format!("UPDATE users SET {} WHERE id = ?", updates.join(", "));

    params.push(Box::new(user_id));

    let param_refs: Vec<&dyn rusqlite::ToSql> = params.iter().map(|p| p.as_ref()).collect();
    db.execute(&query, param_refs.as_slice())?;

    let user = db.query_row(
        "SELECT id, username, role, created_at, updated_at FROM users WHERE id = ?1",
        [user_id],
        |row| {
            let role_str: String = row.get(2)?;
            let role = match role_str.as_str() {
                "admin" => UserRole::Admin,
                _ => UserRole::User,
            };
            Ok(UserResponse {
                id: row.get(0)?,
                username: row.get(1)?,
                role,
                created_at: row.get(3)?,
                updated_at: row.get(4)?,
            })
        },
    )?;

    tracing::info!(user_id = user.id, "User updated");

    Ok(Json(user))
}

/// DELETE /api/users/:id
///
/// Deletes a user (admin only).
pub async fn delete_user(
    State(state): State<AppState>,
    Path(user_id): Path<i64>,
) -> Result<Json<SuccessResponse>> {
    let db = state.db.lock().await;

    // Check if user exists
    let exists: bool = db.query_row(
        "SELECT EXISTS(SELECT 1 FROM users WHERE id = ?1)",
        [user_id],
        |row| row.get(0),
    )?;

    if !exists {
        return Err(AppError::NotFound("User not found".to_string()));
    }

    // Delete associated sessions first
    db.execute("DELETE FROM sessions WHERE user_id = ?1", [user_id])?;

    // Delete the user
    db.execute("DELETE FROM users WHERE id = ?1", [user_id])?;

    tracing::info!(user_id = user_id, "User deleted");

    Ok(Json(SuccessResponse {
        message: "User deleted successfully".to_string(),
    }))
}
