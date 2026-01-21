//! Integration tests for authentication endpoints.

mod common;

use common::TestApp;

#[tokio::test]
async fn test_login_success() {
    let app = TestApp::new().await;

    // Create a test user
    let _user_id = app
        .create_test_user("testuser", "password123", "user")
        .await;

    // Attempt login
    let response = app
        .server()
        .post("/api/auth/login")
        .json(&serde_json::json!({
            "username": "testuser",
            "password": "password123"
        }))
        .await;

    response.assert_status_ok();
    let body: serde_json::Value = response.json();
    assert!(body.get("token").is_some());
}

#[tokio::test]
async fn test_login_wrong_password() {
    let app = TestApp::new().await;

    // Create a test user
    let _user_id = app
        .create_test_user("testuser", "password123", "user")
        .await;

    // Attempt login with wrong password
    let response = app
        .server()
        .post("/api/auth/login")
        .json(&serde_json::json!({
            "username": "testuser",
            "password": "wrongpassword"
        }))
        .await;

    response.assert_status_unauthorized();
}

#[tokio::test]
async fn test_login_nonexistent_user() {
    let app = TestApp::new().await;

    // Attempt login with nonexistent user
    let response = app
        .server()
        .post("/api/auth/login")
        .json(&serde_json::json!({
            "username": "nonexistent",
            "password": "password123"
        }))
        .await;

    response.assert_status_unauthorized();
}

#[tokio::test]
async fn test_me_endpoint_authenticated() {
    let app = TestApp::new().await;

    // Create user and get token
    let (user_id, token) = app.create_user().await;
    let (name, value) = app.auth_header(&token);

    // Access /me endpoint
    let response = app
        .server()
        .get("/api/auth/me")
        .add_header(name, value)
        .await;

    response.assert_status_ok();
    let body: serde_json::Value = response.json();
    assert_eq!(body["id"], user_id);
    assert_eq!(body["username"], "testuser");
    assert_eq!(body["role"], "user");
}

#[tokio::test]
async fn test_me_endpoint_unauthenticated() {
    let app = TestApp::new().await;

    // Attempt to access /me without token
    let response = app.server().get("/api/auth/me").await;

    response.assert_status_unauthorized();
}

#[tokio::test]
async fn test_me_endpoint_invalid_token() {
    let app = TestApp::new().await;

    // Attempt to access /me with invalid token
    let (name, value) = app.auth_header("invalid-token-xyz");
    let response = app
        .server()
        .get("/api/auth/me")
        .add_header(name, value)
        .await;

    response.assert_status_unauthorized();
}

#[tokio::test]
async fn test_logout_success() {
    let app = TestApp::new().await;

    // Create user and get token
    let (_user_id, token) = app.create_user().await;
    let (name, value) = app.auth_header(&token);

    // Logout
    let response = app
        .server()
        .post("/api/auth/logout")
        .add_header(name, value)
        .await;

    response.assert_status_ok();
    let body: serde_json::Value = response.json();
    assert_eq!(body["message"], "Logged out successfully");
}

#[tokio::test]
async fn test_logout_unauthenticated() {
    let app = TestApp::new().await;

    // Attempt to logout without token
    let response = app.server().post("/api/auth/logout").await;

    response.assert_status_unauthorized();
}
