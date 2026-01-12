//! Integration tests for movies endpoints.

mod common;

use common::TestApp;

// =============================================================================
// List Movies Tests
// =============================================================================

#[tokio::test]
async fn test_list_movies() {
    let app = TestApp::new().await;
    let (_user_id, token) = app.create_user().await;
    let (name, value) = app.auth_header(&token);

    let response = app
        .server()
        .get("/api/movies")
        .add_header(name, value)
        .await;

    // Should return OK with paginated response
    response.assert_status_ok();
    let body: serde_json::Value = response.json();
    // Movies endpoint returns a paginated response with items, total, page, pages
    assert!(body.get("items").is_some());
    assert!(body["items"].is_array());
    assert!(body.get("total").is_some());
    assert!(body.get("page").is_some());
    assert!(body.get("pages").is_some());
}

#[tokio::test]
async fn test_list_movies_unauthenticated() {
    let app = TestApp::new().await;

    let response = app.server().get("/api/movies").await;

    response.assert_status_unauthorized();
}

#[tokio::test]
async fn test_list_movies_with_status_filter() {
    let app = TestApp::new().await;
    let (user_id, token) = app.create_user().await;
    let (name, value) = app.auth_header(&token);

    // Insert test movies with different statuses
    let db = app.db().lock().await;
    db.execute(
        r#"
        INSERT INTO movies (tmdb_id, title, year, status, monitored, quality_limit, added_by)
        VALUES
            (1, 'Available Movie', 2023, 'available', 1, '1080p', ?1),
            (2, 'Missing Movie', 2023, 'missing', 1, '1080p', ?1),
            (3, 'Downloading Movie', 2023, 'downloading', 1, '1080p', ?1)
        "#,
        rusqlite::params![user_id],
    )
    .expect("Failed to insert test movies");
    drop(db);

    // Filter by available status
    let response = app
        .server()
        .get("/api/movies?status=available")
        .add_header(name.clone(), value.clone())
        .await;

    response.assert_status_ok();
    let body: serde_json::Value = response.json();
    let items = body["items"].as_array().expect("items should be an array");
    assert_eq!(items.len(), 1);
    assert_eq!(items[0]["title"], "Available Movie");
    assert_eq!(items[0]["status"], "available");

    // Filter by missing status
    let response = app
        .server()
        .get("/api/movies?status=missing")
        .add_header(name.clone(), value.clone())
        .await;

    response.assert_status_ok();
    let body: serde_json::Value = response.json();
    let items = body["items"].as_array().expect("items should be an array");
    assert_eq!(items.len(), 1);
    assert_eq!(items[0]["title"], "Missing Movie");
    assert_eq!(items[0]["status"], "missing");
}

#[tokio::test]
async fn test_list_movies_with_monitored_filter() {
    let app = TestApp::new().await;
    let (user_id, token) = app.create_user().await;
    let (name, value) = app.auth_header(&token);

    // Insert test movies with different monitored states
    let db = app.db().lock().await;
    db.execute(
        r#"
        INSERT INTO movies (tmdb_id, title, year, status, monitored, quality_limit, added_by)
        VALUES
            (10, 'Monitored Movie 1', 2023, 'missing', 1, '1080p', ?1),
            (11, 'Monitored Movie 2', 2023, 'missing', 1, '1080p', ?1),
            (12, 'Unmonitored Movie', 2023, 'missing', 0, '1080p', ?1)
        "#,
        rusqlite::params![user_id],
    )
    .expect("Failed to insert test movies");
    drop(db);

    // Filter by monitored=true
    let response = app
        .server()
        .get("/api/movies?monitored=true")
        .add_header(name.clone(), value.clone())
        .await;

    response.assert_status_ok();
    let body: serde_json::Value = response.json();
    let items = body["items"].as_array().expect("items should be an array");
    assert_eq!(items.len(), 2);
    assert!(items.iter().all(|m| m["monitored"] == true));

    // Filter by monitored=false
    let response = app
        .server()
        .get("/api/movies?monitored=false")
        .add_header(name.clone(), value.clone())
        .await;

    response.assert_status_ok();
    let body: serde_json::Value = response.json();
    let items = body["items"].as_array().expect("items should be an array");
    assert_eq!(items.len(), 1);
    assert_eq!(items[0]["title"], "Unmonitored Movie");
    assert_eq!(items[0]["monitored"], false);
}

#[tokio::test]
async fn test_list_movies_pagination() {
    let app = TestApp::new().await;
    let (user_id, token) = app.create_user().await;
    let (name, value) = app.auth_header(&token);

    // Insert 25 test movies
    let db = app.db().lock().await;
    for i in 1..=25 {
        db.execute(
            r#"
            INSERT INTO movies (tmdb_id, title, year, status, monitored, quality_limit, added_by)
            VALUES (?1, ?2, 2023, 'missing', 1, '1080p', ?3)
            "#,
            rusqlite::params![100 + i, format!("Movie {}", i), user_id],
        )
        .expect("Failed to insert test movie");
    }
    drop(db);

    // First page with default limit (20)
    let response = app
        .server()
        .get("/api/movies?page=1&limit=20")
        .add_header(name.clone(), value.clone())
        .await;

    response.assert_status_ok();
    let body: serde_json::Value = response.json();
    assert_eq!(body["total"], 25);
    assert_eq!(body["page"], 1);
    assert_eq!(body["pages"], 2);
    let items = body["items"].as_array().expect("items should be an array");
    assert_eq!(items.len(), 20);

    // Second page
    let response = app
        .server()
        .get("/api/movies?page=2&limit=20")
        .add_header(name.clone(), value.clone())
        .await;

    response.assert_status_ok();
    let body: serde_json::Value = response.json();
    assert_eq!(body["total"], 25);
    assert_eq!(body["page"], 2);
    assert_eq!(body["pages"], 2);
    let items = body["items"].as_array().expect("items should be an array");
    assert_eq!(items.len(), 5);

    // Custom limit
    let response = app
        .server()
        .get("/api/movies?page=1&limit=10")
        .add_header(name.clone(), value.clone())
        .await;

    response.assert_status_ok();
    let body: serde_json::Value = response.json();
    assert_eq!(body["total"], 25);
    assert_eq!(body["page"], 1);
    assert_eq!(body["pages"], 3);
    let items = body["items"].as_array().expect("items should be an array");
    assert_eq!(items.len(), 10);
}

// =============================================================================
// Get Movie Tests
// =============================================================================

#[tokio::test]
async fn test_get_movie() {
    let app = TestApp::new().await;
    let (user_id, token) = app.create_user().await;
    let (name, value) = app.auth_header(&token);

    // Insert a test movie
    let db = app.db().lock().await;
    db.execute(
        r#"
        INSERT INTO movies (tmdb_id, imdb_id, title, original_title, year, overview, status, monitored, quality_limit, added_by)
        VALUES (550, 'tt0137523', 'Fight Club', 'Fight Club', 1999, 'An insomniac office worker...', 'available', 1, '1080p', ?1)
        "#,
        rusqlite::params![user_id],
    )
    .expect("Failed to insert test movie");
    let movie_id = db.last_insert_rowid();
    drop(db);

    let response = app
        .server()
        .get(&format!("/api/movies/{}", movie_id))
        .add_header(name, value)
        .await;

    response.assert_status_ok();
    let body: serde_json::Value = response.json();
    assert_eq!(body["id"], movie_id);
    assert_eq!(body["tmdb_id"], 550);
    assert_eq!(body["imdb_id"], "tt0137523");
    assert_eq!(body["title"], "Fight Club");
    assert_eq!(body["year"], 1999);
    assert_eq!(body["status"], "available");
    assert_eq!(body["monitored"], true);
    assert_eq!(body["quality_limit"], "1080p");
}

#[tokio::test]
async fn test_get_nonexistent_movie() {
    let app = TestApp::new().await;
    let (_user_id, token) = app.create_user().await;
    let (name, value) = app.auth_header(&token);

    let response = app
        .server()
        .get("/api/movies/999")
        .add_header(name, value)
        .await;

    // Should return 404 for nonexistent movie
    response.assert_status(axum::http::StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn test_get_movie_unauthenticated() {
    let app = TestApp::new().await;

    let response = app.server().get("/api/movies/1").await;

    response.assert_status_unauthorized();
}

// =============================================================================
// Add Movie Tests
// =============================================================================

#[tokio::test]
async fn test_add_movie_invalid_tmdb_id() {
    let app = TestApp::new().await;
    let (_user_id, token) = app.create_user().await;
    let (name, value) = app.auth_header(&token);

    // Attempt to add movie with invalid TMDB ID
    let response = app
        .server()
        .post("/api/movies")
        .add_header(name.clone(), value.clone())
        .json(&serde_json::json!({
            "tmdb_id": 0,
            "monitored": true,
            "quality_limit": "1080p"
        }))
        .await;

    response.assert_status(axum::http::StatusCode::BAD_REQUEST);

    // Negative TMDB ID
    let response = app
        .server()
        .post("/api/movies")
        .add_header(name, value)
        .json(&serde_json::json!({
            "tmdb_id": -1,
            "monitored": true,
            "quality_limit": "1080p"
        }))
        .await;

    response.assert_status(axum::http::StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn test_add_movie_unauthenticated() {
    let app = TestApp::new().await;

    let response = app
        .server()
        .post("/api/movies")
        .json(&serde_json::json!({
            "tmdb_id": 550,
            "monitored": true,
            "quality_limit": "1080p"
        }))
        .await;

    response.assert_status_unauthorized();
}

#[tokio::test]
async fn test_add_movie_no_tmdb_client() {
    let app = TestApp::new().await;
    let (_user_id, token) = app.create_user().await;
    let (name, value) = app.auth_header(&token);

    // Since TestApp doesn't configure TMDB client, this should fail
    let response = app
        .server()
        .post("/api/movies")
        .add_header(name, value)
        .json(&serde_json::json!({
            "tmdb_id": 550,
            "monitored": true,
            "quality_limit": "1080p"
        }))
        .await;

    // Should return internal server error when TMDB client is not configured
    response.assert_status(axum::http::StatusCode::INTERNAL_SERVER_ERROR);
}

#[tokio::test]
async fn test_add_movie_duplicate() {
    let app = TestApp::new().await;
    let (user_id, token) = app.create_user().await;
    let (_name, _value) = app.auth_header(&token);

    // Insert a movie directly into DB
    let db = app.db().lock().await;
    db.execute(
        r#"
        INSERT INTO movies (tmdb_id, title, year, status, monitored, quality_limit, added_by)
        VALUES (550, 'Fight Club', 1999, 'available', 1, '1080p', ?1)
        "#,
        rusqlite::params![user_id],
    )
    .expect("Failed to insert test movie");
    drop(db);

    // Note: Cannot fully test duplicate detection without TMDB client configured
    // The API would check for duplicates after fetching from TMDB
    // This test just documents the limitation in test environment
}

// =============================================================================
// Update Movie Tests
// =============================================================================

#[tokio::test]
async fn test_update_movie() {
    let app = TestApp::new().await;
    let (user_id, token) = app.create_user().await;
    let (name, value) = app.auth_header(&token);

    // Insert a test movie
    let db = app.db().lock().await;
    db.execute(
        r#"
        INSERT INTO movies (tmdb_id, title, year, status, monitored, quality_limit, added_by)
        VALUES (550, 'Fight Club', 1999, 'missing', 1, '1080p', ?1)
        "#,
        rusqlite::params![user_id],
    )
    .expect("Failed to insert test movie");
    let movie_id = db.last_insert_rowid();
    drop(db);

    // Update the movie
    let response = app
        .server()
        .put(&format!("/api/movies/{}", movie_id))
        .add_header(name.clone(), value.clone())
        .json(&serde_json::json!({
            "monitored": false,
            "quality_limit": "720p"
        }))
        .await;

    response.assert_status_ok();
    let body: serde_json::Value = response.json();
    assert_eq!(body["id"], movie_id);
    assert_eq!(body["monitored"], false);
    assert_eq!(body["quality_limit"], "720p");

    // Verify the update persisted
    let response = app
        .server()
        .get(&format!("/api/movies/{}", movie_id))
        .add_header(name, value)
        .await;

    response.assert_status_ok();
    let body: serde_json::Value = response.json();
    assert_eq!(body["monitored"], false);
    assert_eq!(body["quality_limit"], "720p");
}

#[tokio::test]
async fn test_update_movie_partial() {
    let app = TestApp::new().await;
    let (user_id, token) = app.create_user().await;
    let (name, value) = app.auth_header(&token);

    // Insert a test movie
    let db = app.db().lock().await;
    db.execute(
        r#"
        INSERT INTO movies (tmdb_id, title, year, status, monitored, quality_limit, added_by)
        VALUES (550, 'Fight Club', 1999, 'missing', 1, '1080p', ?1)
        "#,
        rusqlite::params![user_id],
    )
    .expect("Failed to insert test movie");
    let movie_id = db.last_insert_rowid();
    drop(db);

    // Update only monitored field
    let response = app
        .server()
        .put(&format!("/api/movies/{}", movie_id))
        .add_header(name.clone(), value.clone())
        .json(&serde_json::json!({
            "monitored": false
        }))
        .await;

    response.assert_status_ok();
    let body: serde_json::Value = response.json();
    assert_eq!(body["monitored"], false);
    assert_eq!(body["quality_limit"], "1080p"); // Should remain unchanged

    // Update only quality_limit field
    let response = app
        .server()
        .put(&format!("/api/movies/{}", movie_id))
        .add_header(name, value)
        .json(&serde_json::json!({
            "quality_limit": "4K"
        }))
        .await;

    response.assert_status_ok();
    let body: serde_json::Value = response.json();
    assert_eq!(body["monitored"], false); // Should remain from previous update
    assert_eq!(body["quality_limit"], "4K");
}

#[tokio::test]
async fn test_update_movie_nonexistent() {
    let app = TestApp::new().await;
    let (_user_id, token) = app.create_user().await;
    let (name, value) = app.auth_header(&token);

    let response = app
        .server()
        .put("/api/movies/999")
        .add_header(name, value)
        .json(&serde_json::json!({
            "monitored": false
        }))
        .await;

    response.assert_status(axum::http::StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn test_update_movie_no_fields() {
    let app = TestApp::new().await;
    let (user_id, token) = app.create_user().await;
    let (name, value) = app.auth_header(&token);

    // Insert a test movie
    let db = app.db().lock().await;
    db.execute(
        r#"
        INSERT INTO movies (tmdb_id, title, year, status, monitored, quality_limit, added_by)
        VALUES (550, 'Fight Club', 1999, 'missing', 1, '1080p', ?1)
        "#,
        rusqlite::params![user_id],
    )
    .expect("Failed to insert test movie");
    let movie_id = db.last_insert_rowid();
    drop(db);

    // Update with empty body
    let response = app
        .server()
        .put(&format!("/api/movies/{}", movie_id))
        .add_header(name, value)
        .json(&serde_json::json!({}))
        .await;

    response.assert_status(axum::http::StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn test_update_movie_unauthenticated() {
    let app = TestApp::new().await;

    let response = app
        .server()
        .put("/api/movies/1")
        .json(&serde_json::json!({
            "monitored": false
        }))
        .await;

    response.assert_status_unauthorized();
}

// =============================================================================
// Delete Movie Tests
// =============================================================================

#[tokio::test]
async fn test_delete_movie() {
    let app = TestApp::new().await;
    let (user_id, token) = app.create_user().await;
    let (name, value) = app.auth_header(&token);

    // Insert a test movie
    let db = app.db().lock().await;
    db.execute(
        r#"
        INSERT INTO movies (tmdb_id, title, year, status, monitored, quality_limit, added_by)
        VALUES (550, 'Fight Club', 1999, 'missing', 1, '1080p', ?1)
        "#,
        rusqlite::params![user_id],
    )
    .expect("Failed to insert test movie");
    let movie_id = db.last_insert_rowid();
    drop(db);

    // Delete the movie
    let response = app
        .server()
        .delete(&format!("/api/movies/{}", movie_id))
        .add_header(name.clone(), value.clone())
        .await;

    response.assert_status_ok();
    let body: serde_json::Value = response.json();
    assert_eq!(body["success"], true);
    assert!(body.get("message").is_some());

    // Verify movie is deleted
    let response = app
        .server()
        .get(&format!("/api/movies/{}", movie_id))
        .add_header(name, value)
        .await;

    response.assert_status(axum::http::StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn test_delete_movie_nonexistent() {
    let app = TestApp::new().await;
    let (_user_id, token) = app.create_user().await;
    let (name, value) = app.auth_header(&token);

    let response = app
        .server()
        .delete("/api/movies/999")
        .add_header(name, value)
        .await;

    response.assert_status(axum::http::StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn test_delete_movie_with_files() {
    let app = TestApp::new().await;
    let (user_id, token) = app.create_user().await;
    let (name, value) = app.auth_header(&token);

    // Create a temporary file to simulate movie file
    let temp_dir = std::env::temp_dir();
    let file_path = temp_dir.join("test_movie_delete.mkv");
    std::fs::write(&file_path, b"test content").expect("Failed to create test file");

    // Insert a test movie with file path
    let db = app.db().lock().await;
    db.execute(
        r#"
        INSERT INTO movies (tmdb_id, title, year, status, monitored, quality_limit, file_path, added_by)
        VALUES (550, 'Fight Club', 1999, 'available', 1, '1080p', ?1, ?2)
        "#,
        rusqlite::params![file_path.to_str().unwrap(), user_id],
    )
    .expect("Failed to insert test movie");
    let movie_id = db.last_insert_rowid();
    drop(db);

    // Delete the movie with delete_files=true
    let response = app
        .server()
        .delete(&format!("/api/movies/{}?delete_files=true", movie_id))
        .add_header(name, value)
        .await;

    response.assert_status_ok();

    // Verify file is deleted
    assert!(!file_path.exists());
}

#[tokio::test]
async fn test_delete_movie_unauthenticated() {
    let app = TestApp::new().await;

    let response = app.server().delete("/api/movies/1").await;

    response.assert_status_unauthorized();
}

// =============================================================================
// Search Releases Tests
// =============================================================================

#[tokio::test]
async fn test_search_releases_movie_not_found() {
    let app = TestApp::new().await;
    let (_user_id, token) = app.create_user().await;
    let (name, value) = app.auth_header(&token);

    let response = app
        .server()
        .post("/api/movies/999/search")
        .add_header(name, value)
        .await;

    response.assert_status(axum::http::StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn test_search_releases_unauthenticated() {
    let app = TestApp::new().await;

    let response = app.server().post("/api/movies/1/search").await;

    response.assert_status_unauthorized();
}

#[tokio::test]
async fn test_search_releases_no_indexers() {
    let app = TestApp::new().await;
    let (user_id, token) = app.create_user().await;
    let (name, value) = app.auth_header(&token);

    // Insert a test movie
    let db = app.db().lock().await;
    db.execute(
        r#"
        INSERT INTO movies (tmdb_id, imdb_id, title, year, status, monitored, quality_limit, added_by)
        VALUES (550, 'tt0137523', 'Fight Club', 1999, 'missing', 1, '1080p', ?1)
        "#,
        rusqlite::params![user_id],
    )
    .expect("Failed to insert test movie");
    let movie_id = db.last_insert_rowid();
    drop(db);

    // Search should succeed but return empty results (no indexers configured)
    let response = app
        .server()
        .post(&format!("/api/movies/{}/search", movie_id))
        .add_header(name, value)
        .await;

    response.assert_status_ok();
    let body: serde_json::Value = response.json();
    assert!(body.is_array());
    assert_eq!(body.as_array().unwrap().len(), 0);
}

// =============================================================================
// Download Release Tests
// =============================================================================

#[tokio::test]
async fn test_download_release_invalid_magnet() {
    let app = TestApp::new().await;
    let (user_id, token) = app.create_user().await;
    let (name, value) = app.auth_header(&token);

    // Insert a test movie
    let db = app.db().lock().await;
    db.execute(
        r#"
        INSERT INTO movies (tmdb_id, title, year, status, monitored, quality_limit, added_by)
        VALUES (550, 'Fight Club', 1999, 'missing', 1, '1080p', ?1)
        "#,
        rusqlite::params![user_id],
    )
    .expect("Failed to insert test movie");
    let movie_id = db.last_insert_rowid();
    drop(db);

    // Try to download with invalid magnet link
    let response = app
        .server()
        .post(&format!("/api/movies/{}/download", movie_id))
        .add_header(name.clone(), value.clone())
        .json(&serde_json::json!({
            "magnet": "not-a-valid-magnet-link"
        }))
        .await;

    response.assert_status(axum::http::StatusCode::BAD_REQUEST);

    // Try with empty magnet
    let response = app
        .server()
        .post(&format!("/api/movies/{}/download", movie_id))
        .add_header(name, value)
        .json(&serde_json::json!({
            "magnet": ""
        }))
        .await;

    response.assert_status(axum::http::StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn test_download_release_movie_not_found() {
    let app = TestApp::new().await;
    let (_user_id, token) = app.create_user().await;
    let (name, value) = app.auth_header(&token);

    let response = app
        .server()
        .post("/api/movies/999/download")
        .add_header(name, value)
        .json(&serde_json::json!({
            "magnet": "magnet:?xt=urn:btih:abc123"
        }))
        .await;

    // Note: Returns 500 in test environment because torrent engine is checked before movie existence
    // In production with torrent engine configured, this would return 404 for nonexistent movie
    response.assert_status(axum::http::StatusCode::INTERNAL_SERVER_ERROR);
}

#[tokio::test]
async fn test_download_release_unauthenticated() {
    let app = TestApp::new().await;

    let response = app
        .server()
        .post("/api/movies/1/download")
        .json(&serde_json::json!({
            "magnet": "magnet:?xt=urn:btih:abc123"
        }))
        .await;

    response.assert_status_unauthorized();
}

#[tokio::test]
async fn test_download_release_no_torrent_engine() {
    let app = TestApp::new().await;
    let (user_id, token) = app.create_user().await;
    let (name, value) = app.auth_header(&token);

    // Insert a test movie
    let db = app.db().lock().await;
    db.execute(
        r#"
        INSERT INTO movies (tmdb_id, title, year, status, monitored, quality_limit, added_by)
        VALUES (550, 'Fight Club', 1999, 'missing', 1, '1080p', ?1)
        "#,
        rusqlite::params![user_id],
    )
    .expect("Failed to insert test movie");
    let movie_id = db.last_insert_rowid();
    drop(db);

    // Since TestApp doesn't configure torrent engine, this should fail
    let response = app
        .server()
        .post(&format!("/api/movies/{}/download", movie_id))
        .add_header(name, value)
        .json(&serde_json::json!({
            "magnet": "magnet:?xt=urn:btih:abc123&dn=Fight.Club.1999.1080p"
        }))
        .await;

    // Should return internal server error when torrent engine is not available
    response.assert_status(axum::http::StatusCode::INTERNAL_SERVER_ERROR);
}

// =============================================================================
// Refresh Metadata Tests
// =============================================================================

#[tokio::test]
async fn test_refresh_metadata_movie_not_found() {
    let app = TestApp::new().await;
    let (_user_id, token) = app.create_user().await;
    let (name, value) = app.auth_header(&token);

    let response = app
        .server()
        .post("/api/movies/999/refresh")
        .add_header(name, value)
        .await;

    // Note: Returns 500 in test environment because TMDB client is checked before movie existence
    // In production with TMDB client configured, this would return 404 for nonexistent movie
    response.assert_status(axum::http::StatusCode::INTERNAL_SERVER_ERROR);
}

#[tokio::test]
async fn test_refresh_metadata_unauthenticated() {
    let app = TestApp::new().await;

    let response = app.server().post("/api/movies/1/refresh").await;

    response.assert_status_unauthorized();
}

#[tokio::test]
async fn test_refresh_metadata_no_tmdb_client() {
    let app = TestApp::new().await;
    let (user_id, token) = app.create_user().await;
    let (name, value) = app.auth_header(&token);

    // Insert a test movie
    let db = app.db().lock().await;
    db.execute(
        r#"
        INSERT INTO movies (tmdb_id, title, year, status, monitored, quality_limit, added_by)
        VALUES (550, 'Fight Club', 1999, 'missing', 1, '1080p', ?1)
        "#,
        rusqlite::params![user_id],
    )
    .expect("Failed to insert test movie");
    let movie_id = db.last_insert_rowid();
    drop(db);

    // Since TestApp doesn't configure TMDB client, this should fail
    let response = app
        .server()
        .post(&format!("/api/movies/{}/refresh", movie_id))
        .add_header(name, value)
        .await;

    // Should return internal server error when TMDB client is not configured
    response.assert_status(axum::http::StatusCode::INTERNAL_SERVER_ERROR);
}
