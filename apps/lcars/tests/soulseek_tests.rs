//! Integration tests for Soulseek API endpoints.
//!
//! These tests verify the HTTP API behavior when Soulseek is not configured,
//! since we cannot establish real Soulseek connections in the test environment.

mod common;

use common::TestApp;

// =============================================================================
// Status endpoint tests
// =============================================================================

#[tokio::test]
async fn test_soulseek_status_without_engine() {
    let app = TestApp::new().await;
    let (_user_id, token) = app.create_user().await;
    let (name, value) = app.auth_header(&token);

    let response = app
        .server()
        .get("/api/soulseek/status")
        .add_header(name, value)
        .await;

    response.assert_status_ok();
    let body: serde_json::Value = response.json();

    // When soulseek is not configured, should return disconnected state
    assert_eq!(body["connected"], false);
    assert_eq!(body["connection_state"]["state"], "disconnected");
    assert_eq!(body["active_searches"], 0);
    assert_eq!(body["active_downloads"], 0);
}

#[tokio::test]
async fn test_soulseek_status_unauthenticated() {
    let app = TestApp::new().await;

    let response = app.server().get("/api/soulseek/status").await;

    response.assert_status_unauthorized();
}

// =============================================================================
// Search endpoint tests
// =============================================================================

#[tokio::test]
async fn test_soulseek_search_without_engine() {
    let app = TestApp::new().await;
    let (_user_id, token) = app.create_user().await;
    let (name, value) = app.auth_header(&token);

    let response = app
        .server()
        .post("/api/soulseek/search")
        .add_header(name, value)
        .json(&serde_json::json!({
            "query": "test artist album"
        }))
        .await;

    // Should return 503 Service Unavailable when Soulseek is not configured
    response.assert_status(axum::http::StatusCode::SERVICE_UNAVAILABLE);
}

#[tokio::test]
async fn test_soulseek_search_empty_query() {
    let app = TestApp::new().await;
    let (_user_id, token) = app.create_user().await;
    let (name, value) = app.auth_header(&token);

    let response = app
        .server()
        .post("/api/soulseek/search")
        .add_header(name, value)
        .json(&serde_json::json!({
            "query": "   "
        }))
        .await;

    // Empty query should return 400 Bad Request
    response.assert_status(axum::http::StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn test_soulseek_search_unauthenticated() {
    let app = TestApp::new().await;

    let response = app
        .server()
        .post("/api/soulseek/search")
        .json(&serde_json::json!({
            "query": "test"
        }))
        .await;

    response.assert_status_unauthorized();
}

// =============================================================================
// Download endpoint tests
// =============================================================================

#[tokio::test]
async fn test_soulseek_download_without_engine() {
    let app = TestApp::new().await;
    let (_user_id, token) = app.create_user().await;
    let (name, value) = app.auth_header(&token);

    let response = app
        .server()
        .post("/api/soulseek/download")
        .add_header(name, value)
        .json(&serde_json::json!({
            "username": "testuser",
            "filename": "/Music/Artist/Album/track.flac",
            "size": 50000000
        }))
        .await;

    // Should return 503 Service Unavailable when Soulseek is not configured
    response.assert_status(axum::http::StatusCode::SERVICE_UNAVAILABLE);
}

#[tokio::test]
async fn test_soulseek_download_empty_username() {
    let app = TestApp::new().await;
    let (_user_id, token) = app.create_user().await;
    let (name, value) = app.auth_header(&token);

    let response = app
        .server()
        .post("/api/soulseek/download")
        .add_header(name, value)
        .json(&serde_json::json!({
            "username": "  ",
            "filename": "/Music/test.flac",
            "size": 1000
        }))
        .await;

    // Empty username should return 400 Bad Request
    response.assert_status(axum::http::StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn test_soulseek_download_empty_filename() {
    let app = TestApp::new().await;
    let (_user_id, token) = app.create_user().await;
    let (name, value) = app.auth_header(&token);

    let response = app
        .server()
        .post("/api/soulseek/download")
        .add_header(name, value)
        .json(&serde_json::json!({
            "username": "testuser",
            "filename": "",
            "size": 1000
        }))
        .await;

    // Empty filename should return 400 Bad Request
    response.assert_status(axum::http::StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn test_soulseek_download_unauthenticated() {
    let app = TestApp::new().await;

    let response = app
        .server()
        .post("/api/soulseek/download")
        .json(&serde_json::json!({
            "username": "testuser",
            "filename": "/Music/test.flac",
            "size": 1000
        }))
        .await;

    response.assert_status_unauthorized();
}

// =============================================================================
// Downloads list endpoint tests
// =============================================================================

#[tokio::test]
async fn test_soulseek_list_downloads_without_engine() {
    let app = TestApp::new().await;
    let (_user_id, token) = app.create_user().await;
    let (name, value) = app.auth_header(&token);

    let response = app
        .server()
        .get("/api/soulseek/downloads")
        .add_header(name, value)
        .await;

    // Should return 503 Service Unavailable when Soulseek is not configured
    response.assert_status(axum::http::StatusCode::SERVICE_UNAVAILABLE);
}

#[tokio::test]
async fn test_soulseek_list_downloads_unauthenticated() {
    let app = TestApp::new().await;

    let response = app.server().get("/api/soulseek/downloads").await;

    response.assert_status_unauthorized();
}

// =============================================================================
// Browse endpoint tests
// =============================================================================

#[tokio::test]
async fn test_soulseek_browse_without_engine() {
    let app = TestApp::new().await;
    let (_user_id, token) = app.create_user().await;
    let (name, value) = app.auth_header(&token);

    let response = app
        .server()
        .get("/api/soulseek/browse/someuser")
        .add_header(name, value)
        .await;

    // Should return 503 Service Unavailable when Soulseek is not configured
    response.assert_status(axum::http::StatusCode::SERVICE_UNAVAILABLE);
}

#[tokio::test]
async fn test_soulseek_browse_unauthenticated() {
    let app = TestApp::new().await;

    let response = app.server().get("/api/soulseek/browse/someuser").await;

    response.assert_status_unauthorized();
}

// =============================================================================
// Shares endpoint tests
// =============================================================================

#[tokio::test]
async fn test_soulseek_shares_without_engine() {
    let app = TestApp::new().await;
    let (_user_id, token) = app.create_user().await;
    let (name, value) = app.auth_header(&token);

    let response = app
        .server()
        .get("/api/soulseek/shares")
        .add_header(name, value)
        .await;

    // Should return 503 Service Unavailable when Soulseek is not configured
    response.assert_status(axum::http::StatusCode::SERVICE_UNAVAILABLE);
}

#[tokio::test]
async fn test_soulseek_rescan_shares_without_engine() {
    let app = TestApp::new().await;
    let (_user_id, token) = app.create_user().await;
    let (name, value) = app.auth_header(&token);

    let response = app
        .server()
        .post("/api/soulseek/shares/rescan")
        .add_header(name, value)
        .await;

    // Should return 503 Service Unavailable when Soulseek is not configured
    response.assert_status(axum::http::StatusCode::SERVICE_UNAVAILABLE);
}

// =============================================================================
// Uploads endpoint tests
// =============================================================================

#[tokio::test]
async fn test_soulseek_list_uploads_without_engine() {
    let app = TestApp::new().await;
    let (_user_id, token) = app.create_user().await;
    let (name, value) = app.auth_header(&token);

    let response = app
        .server()
        .get("/api/soulseek/uploads")
        .add_header(name, value)
        .await;

    // Should return 503 Service Unavailable when Soulseek is not configured
    response.assert_status(axum::http::StatusCode::SERVICE_UNAVAILABLE);
}

#[tokio::test]
async fn test_soulseek_uploads_unauthenticated() {
    let app = TestApp::new().await;

    let response = app.server().get("/api/soulseek/uploads").await;

    response.assert_status_unauthorized();
}
