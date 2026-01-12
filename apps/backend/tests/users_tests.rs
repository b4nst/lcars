//! Integration tests for user management endpoints.

mod common;

use common::TestApp;

#[tokio::test]
async fn test_list_users_as_admin() {
    let app = TestApp::new().await;

    // Create admin and regular user
    let (_admin_id, admin_token) = app.create_admin().await;
    let _user_id = app.create_test_user("regularuser", "pass123", "user").await;

    let (name, value) = app.auth_header(&admin_token);

    // List users as admin
    let response = app.server().get("/api/users").add_header(name, value).await;

    response.assert_status_ok();
    let body: serde_json::Value = response.json();
    let users = body.as_array().expect("Response should be an array");
    assert!(users.len() >= 2); // At least admin and regularuser
}

#[tokio::test]
async fn test_list_users_as_regular_user() {
    let app = TestApp::new().await;

    // Create regular user
    let (_user_id, user_token) = app.create_user().await;
    let (name, value) = app.auth_header(&user_token);

    // Attempt to list users as regular user
    let response = app.server().get("/api/users").add_header(name, value).await;

    response.assert_status_forbidden();
}

#[tokio::test]
async fn test_list_users_unauthenticated() {
    let app = TestApp::new().await;

    // Attempt to list users without authentication
    let response = app.server().get("/api/users").await;

    response.assert_status_unauthorized();
}

#[tokio::test]
async fn test_create_user_as_admin() {
    let app = TestApp::new().await;

    // Create admin
    let (_admin_id, admin_token) = app.create_admin().await;
    let (name, value) = app.auth_header(&admin_token);

    // Create new user
    let response = app
        .server()
        .post("/api/users")
        .add_header(name, value)
        .json(&serde_json::json!({
            "username": "newuser",
            "password": "newpass123",
            "role": "user"
        }))
        .await;

    response.assert_status_ok();
    let body: serde_json::Value = response.json();
    assert!(body.get("id").is_some());
    assert_eq!(body["username"], "newuser");
    assert_eq!(body["role"], "user");
}

#[tokio::test]
async fn test_create_user_as_regular_user() {
    let app = TestApp::new().await;

    // Create regular user
    let (_user_id, user_token) = app.create_user().await;
    let (name, value) = app.auth_header(&user_token);

    // Attempt to create user as regular user
    let response = app
        .server()
        .post("/api/users")
        .add_header(name, value)
        .json(&serde_json::json!({
            "username": "newuser",
            "password": "newpass123",
            "role": "user"
        }))
        .await;

    response.assert_status_forbidden();
}

#[tokio::test]
async fn test_update_user_as_admin() {
    let app = TestApp::new().await;

    // Create admin and regular user
    let (_admin_id, admin_token) = app.create_admin().await;
    let user_id = app.create_test_user("updateme", "pass123", "user").await;

    let (name, value) = app.auth_header(&admin_token);

    // Update user
    let response = app
        .server()
        .put(&format!("/api/users/{}", user_id))
        .add_header(name, value)
        .json(&serde_json::json!({
            "username": "updated_username",
            "role": "user"
        }))
        .await;

    response.assert_status_ok();
    let body: serde_json::Value = response.json();
    assert_eq!(body["username"], "updated_username");
}

#[tokio::test]
async fn test_delete_user_as_admin() {
    let app = TestApp::new().await;

    // Create admin and user to delete
    let (_admin_id, admin_token) = app.create_admin().await;
    let user_id = app.create_test_user("deleteme", "pass123", "user").await;

    let (name, value) = app.auth_header(&admin_token);

    // Delete user
    let response = app
        .server()
        .delete(&format!("/api/users/{}", user_id))
        .add_header(name, value)
        .await;

    response.assert_status_ok();

    // Verify user is deleted
    let db = app.db().lock().await;
    let exists: bool = db
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM users WHERE id = ?1)",
            [user_id],
            |row| row.get(0),
        )
        .unwrap_or(false);
    assert!(!exists);
}

#[tokio::test]
async fn test_delete_user_as_regular_user() {
    let app = TestApp::new().await;

    // Create two regular users
    let (_user_id, user_token) = app.create_user().await;
    let victim_id = app.create_test_user("victim", "pass123", "user").await;

    let (name, value) = app.auth_header(&user_token);

    // Attempt to delete user as regular user
    let response = app
        .server()
        .delete(&format!("/api/users/{}", victim_id))
        .add_header(name, value)
        .await;

    response.assert_status_forbidden();
}
