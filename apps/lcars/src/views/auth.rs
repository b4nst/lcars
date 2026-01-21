//! Authentication views

use askama::Template;
use axum::{
    extract::State,
    http::{header, HeaderMap, StatusCode},
    response::{IntoResponse, Redirect},
    Form,
};
use axum_extra::extract::cookie::{Cookie, CookieJar};
use serde::Deserialize;

use crate::services::auth::Claims;
use crate::AppState;

#[derive(Template)]
#[template(path = "login.html")]
pub struct LoginTemplate {
    pub error: Option<String>,
}

#[derive(Deserialize)]
pub struct LoginForm {
    pub username: String,
    pub password: String,
}

/// Render the login page
pub async fn login_page() -> impl IntoResponse {
    LoginTemplate { error: None }
}

/// Handle login form submission
pub async fn login_submit(
    State(state): State<AppState>,
    headers: HeaderMap,
    cookies: CookieJar,
    Form(form): Form<LoginForm>,
) -> impl IntoResponse {
    // Check if this is an HTMX request
    let is_htmx = headers.contains_key("hx-request");

    // Query user from database
    let db = state.db.lock().await;
    let user_result: Result<(i64, String, String), rusqlite::Error> = db.query_row(
        "SELECT id, password_hash, role FROM users WHERE username = ?1",
        [&form.username],
        |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
    );
    drop(db);

    match user_result {
        Ok((user_id, password_hash, role)) => {
            // Verify password
            match state
                .auth_service()
                .verify_password(&form.password, &password_hash)
            {
                Ok(true) => {
                    // Create JWT token
                    match state.auth_service().create_token(user_id, &role) {
                        Ok(token) => {
                            // Set session cookie (7 days)
                            let cookie = Cookie::build(("session", token))
                                .path("/")
                                .http_only(true)
                                .secure(state.config.server.secure_cookies)
                                .same_site(axum_extra::extract::cookie::SameSite::Lax)
                                .max_age(::time::Duration::days(7))
                                .build();

                            if is_htmx {
                                // HTMX request: return HX-Redirect header
                                (
                                    cookies.add(cookie),
                                    [(header::HeaderName::from_static("hx-redirect"), "/")],
                                    StatusCode::OK,
                                )
                                    .into_response()
                            } else {
                                // Standard form submission: HTTP redirect
                                (cookies.add(cookie), Redirect::to("/")).into_response()
                            }
                        }
                        Err(_) => LoginTemplate {
                            error: Some("Authentication failed".to_string()),
                        }
                        .into_response(),
                    }
                }
                _ => {
                    // Invalid password
                    LoginTemplate {
                        error: Some("Invalid username or password".to_string()),
                    }
                    .into_response()
                }
            }
        }
        Err(_) => {
            // User not found
            LoginTemplate {
                error: Some("Invalid username or password".to_string()),
            }
            .into_response()
        }
    }
}

/// Handle logout
pub async fn logout(headers: HeaderMap, cookies: CookieJar) -> impl IntoResponse {
    let is_htmx = headers.contains_key("hx-request");

    let cookie = Cookie::build(("session", ""))
        .path("/")
        .max_age(::time::Duration::ZERO)
        .build();

    if is_htmx {
        (
            cookies.remove(cookie),
            [(header::HeaderName::from_static("hx-redirect"), "/login")],
            StatusCode::OK,
        )
            .into_response()
    } else {
        (cookies.remove(cookie), Redirect::to("/login")).into_response()
    }
}

/// Extract session token from cookies and validate
pub async fn get_current_user(state: &AppState, cookies: &CookieJar) -> Option<Claims> {
    let session = cookies.get("session")?;
    state.auth_service().verify_token(session.value()).ok()
}
