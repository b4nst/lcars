//! Integration tests for music API endpoints.

mod common;

use common::TestApp;

// =============================================================================
// Artist Tests - List & Search
// =============================================================================

#[tokio::test]
async fn test_list_artists() {
    let app = TestApp::new().await;
    let (_user_id, token) = app.create_user().await;

    // Seed some test data
    seed_test_artist(&app, "Pink Floyd", "5441c29d-3602-4898-b1a1-b77fa23b8e50").await;
    seed_test_artist(&app, "Radiohead", "a74b1b7f-71a5-4011-9441-d0b5e4122711").await;

    let (name, value) = app.auth_header(&token);
    let response = app
        .server()
        .get("/api/music/artists")
        .add_header(name, value)
        .await;

    response.assert_status_ok();
    let body: serde_json::Value = response.json();

    assert!(body.get("items").is_some());
    assert!(body.get("total").is_some());
    assert!(body.get("page").is_some());
    assert!(body.get("pages").is_some());

    let items = body["items"].as_array().expect("items should be an array");
    assert!(items.len() >= 2, "Should have at least 2 artists");
}

#[tokio::test]
async fn test_list_artists_unauthenticated() {
    let app = TestApp::new().await;

    let response = app.server().get("/api/music/artists").await;
    response.assert_status_unauthorized();
}

#[tokio::test]
async fn test_list_artists_with_search() {
    let app = TestApp::new().await;
    let (_user_id, token) = app.create_user().await;

    // Seed test data with distinct names
    seed_test_artist(&app, "Pink Floyd", "5441c29d-3602-4898-b1a1-b77fa23b8e50").await;
    seed_test_artist(&app, "Radiohead", "a74b1b7f-71a5-4011-9441-d0b5e4122711").await;
    seed_test_artist(&app, "The Beatles", "b10bbbfc-cf9e-42e0-be17-e2c3e1d2600d").await;

    let (name, value) = app.auth_header(&token);

    // Search for "Pink"
    let response = app
        .server()
        .get("/api/music/artists?search=Pink")
        .add_header(name.clone(), value.clone())
        .await;

    response.assert_status_ok();
    let body: serde_json::Value = response.json();
    let items = body["items"].as_array().expect("items should be an array");

    // Should find Pink Floyd
    assert!(
        items
            .iter()
            .any(|item| item["name"].as_str().unwrap().contains("Pink")),
        "Should find Pink Floyd"
    );
}

#[tokio::test]
async fn test_list_artists_with_monitored_filter() {
    let app = TestApp::new().await;
    let (_user_id, token) = app.create_user().await;

    // Seed artists with different monitored status
    let _monitored_id = seed_test_artist(
        &app,
        "Monitored Artist",
        "5441c29d-3602-4898-b1a1-b77fa23b8e50",
    )
    .await;
    let unmonitored_id = seed_test_artist(
        &app,
        "Unmonitored Artist",
        "a74b1b7f-71a5-4011-9441-d0b5e4122711",
    )
    .await;

    // Set one as unmonitored
    update_artist_monitored(&app, unmonitored_id, false).await;

    let (name, value) = app.auth_header(&token);

    // Filter by monitored=true
    let response = app
        .server()
        .get("/api/music/artists?monitored=true")
        .add_header(name.clone(), value.clone())
        .await;

    response.assert_status_ok();
    let body: serde_json::Value = response.json();
    let items = body["items"].as_array().expect("items should be an array");

    // Should only find monitored artist
    assert!(
        items
            .iter()
            .all(|item| item["monitored"].as_bool().unwrap() == true),
        "All artists should be monitored"
    );

    // Filter by monitored=false
    let response = app
        .server()
        .get("/api/music/artists?monitored=false")
        .add_header(name, value)
        .await;

    response.assert_status_ok();
    let body: serde_json::Value = response.json();
    let items = body["items"].as_array().expect("items should be an array");

    // Should only find unmonitored artist
    assert!(
        items
            .iter()
            .all(|item| item["monitored"].as_bool().unwrap() == false),
        "All artists should be unmonitored"
    );
}

#[tokio::test]
async fn test_list_artists_pagination() {
    let app = TestApp::new().await;
    let (_user_id, token) = app.create_user().await;

    // Seed enough artists for pagination
    for i in 0..25 {
        seed_test_artist(
            &app,
            &format!("Artist {}", i),
            &format!("12345678-1234-1234-1234-123456789{:03}", i),
        )
        .await;
    }

    let (name, value) = app.auth_header(&token);

    // Get first page
    let response = app
        .server()
        .get("/api/music/artists?page=1&limit=10")
        .add_header(name.clone(), value.clone())
        .await;

    response.assert_status_ok();
    let body: serde_json::Value = response.json();
    assert_eq!(body["page"], 1);
    assert_eq!(body["items"].as_array().unwrap().len(), 10);

    // Get second page
    let response = app
        .server()
        .get("/api/music/artists?page=2&limit=10")
        .add_header(name, value)
        .await;

    response.assert_status_ok();
    let body: serde_json::Value = response.json();
    assert_eq!(body["page"], 2);
    assert!(body["items"].as_array().unwrap().len() > 0);
}

// =============================================================================
// Artist Tests - CRUD Operations
// =============================================================================

#[tokio::test]
async fn test_get_artist() {
    let app = TestApp::new().await;
    let (_user_id, token) = app.create_user().await;

    let artist_id =
        seed_test_artist(&app, "Pink Floyd", "5441c29d-3602-4898-b1a1-b77fa23b8e50").await;

    let (name, value) = app.auth_header(&token);
    let response = app
        .server()
        .get(&format!("/api/music/artists/{}", artist_id))
        .add_header(name, value)
        .await;

    response.assert_status_ok();
    let body: serde_json::Value = response.json();

    assert_eq!(body["id"], artist_id);
    assert_eq!(body["name"], "Pink Floyd");
    assert_eq!(body["mbid"], "5441c29d-3602-4898-b1a1-b77fa23b8e50");
    assert!(body.get("albums").is_some());
}

#[tokio::test]
async fn test_get_nonexistent_artist() {
    let app = TestApp::new().await;
    let (_user_id, token) = app.create_user().await;

    let (name, value) = app.auth_header(&token);
    let response = app
        .server()
        .get("/api/music/artists/99999")
        .add_header(name, value)
        .await;

    response.assert_status_not_found();
}

#[tokio::test]
async fn test_update_artist() {
    let app = TestApp::new().await;
    let (_user_id, token) = app.create_user().await;

    let artist_id =
        seed_test_artist(&app, "Pink Floyd", "5441c29d-3602-4898-b1a1-b77fa23b8e50").await;

    let (name, value) = app.auth_header(&token);
    let response = app
        .server()
        .put(&format!("/api/music/artists/{}", artist_id))
        .add_header(name.clone(), value.clone())
        .json(&serde_json::json!({
            "monitored": false,
            "quality_limit": "mp3"
        }))
        .await;

    response.assert_status_ok();
    let body: serde_json::Value = response.json();

    assert_eq!(body["id"], artist_id);
    assert_eq!(body["monitored"], false);
    assert_eq!(body["quality_limit"], "mp3");
}

#[tokio::test]
async fn test_update_artist_no_fields() {
    let app = TestApp::new().await;
    let (_user_id, token) = app.create_user().await;

    let artist_id =
        seed_test_artist(&app, "Pink Floyd", "5441c29d-3602-4898-b1a1-b77fa23b8e50").await;

    let (name, value) = app.auth_header(&token);
    let response = app
        .server()
        .put(&format!("/api/music/artists/{}", artist_id))
        .add_header(name, value)
        .json(&serde_json::json!({}))
        .await;

    response.assert_status_bad_request();
}

#[tokio::test]
async fn test_update_nonexistent_artist() {
    let app = TestApp::new().await;
    let (_user_id, token) = app.create_user().await;

    let (name, value) = app.auth_header(&token);
    let response = app
        .server()
        .put("/api/music/artists/99999")
        .add_header(name, value)
        .json(&serde_json::json!({
            "monitored": false
        }))
        .await;

    response.assert_status_not_found();
}

#[tokio::test]
async fn test_delete_artist() {
    let app = TestApp::new().await;
    let (_user_id, token) = app.create_user().await;

    let artist_id =
        seed_test_artist(&app, "Pink Floyd", "5441c29d-3602-4898-b1a1-b77fa23b8e50").await;

    let (name, value) = app.auth_header(&token);
    let response = app
        .server()
        .delete(&format!("/api/music/artists/{}", artist_id))
        .add_header(name.clone(), value.clone())
        .await;

    response.assert_status_ok();

    // Verify artist is deleted
    let db = app.db().lock().await;
    let exists: bool = db
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM artists WHERE id = ?1)",
            [artist_id],
            |row| row.get(0),
        )
        .unwrap_or(false);
    assert!(!exists, "Artist should be deleted");
}

#[tokio::test]
async fn test_delete_nonexistent_artist() {
    let app = TestApp::new().await;
    let (_user_id, token) = app.create_user().await;

    let (name, value) = app.auth_header(&token);
    let response = app
        .server()
        .delete("/api/music/artists/99999")
        .add_header(name, value)
        .await;

    response.assert_status_not_found();
}

// =============================================================================
// Album Tests
// =============================================================================

#[tokio::test]
async fn test_list_albums() {
    let app = TestApp::new().await;
    let (_user_id, token) = app.create_user().await;

    let artist_id =
        seed_test_artist(&app, "Pink Floyd", "5441c29d-3602-4898-b1a1-b77fa23b8e50").await;
    seed_test_album(&app, artist_id, "The Dark Side of the Moon").await;
    seed_test_album(&app, artist_id, "The Wall").await;

    let (name, value) = app.auth_header(&token);
    let response = app
        .server()
        .get("/api/music/albums")
        .add_header(name, value)
        .await;

    response.assert_status_ok();
    let body: serde_json::Value = response.json();

    assert!(body.get("items").is_some());
    assert!(body.get("total").is_some());
    let items = body["items"].as_array().expect("items should be an array");
    assert!(items.len() >= 2, "Should have at least 2 albums");
}

#[tokio::test]
async fn test_list_albums_with_artist_filter() {
    let app = TestApp::new().await;
    let (_user_id, token) = app.create_user().await;

    let artist1_id =
        seed_test_artist(&app, "Pink Floyd", "5441c29d-3602-4898-b1a1-b77fa23b8e50").await;
    let artist2_id =
        seed_test_artist(&app, "Radiohead", "a74b1b7f-71a5-4011-9441-d0b5e4122711").await;

    seed_test_album(&app, artist1_id, "The Dark Side of the Moon").await;
    seed_test_album(&app, artist2_id, "OK Computer").await;

    let (name, value) = app.auth_header(&token);
    let response = app
        .server()
        .get(&format!("/api/music/albums?artist_id={}", artist1_id))
        .add_header(name, value)
        .await;

    response.assert_status_ok();
    let body: serde_json::Value = response.json();
    let items = body["items"].as_array().expect("items should be an array");

    // All albums should belong to artist1
    assert!(
        items.iter().all(|item| item["artist_id"] == artist1_id),
        "All albums should belong to the filtered artist"
    );
}

#[tokio::test]
async fn test_list_albums_with_status_filter() {
    let app = TestApp::new().await;
    let (_user_id, token) = app.create_user().await;

    let artist_id =
        seed_test_artist(&app, "Pink Floyd", "5441c29d-3602-4898-b1a1-b77fa23b8e50").await;
    let album_id = seed_test_album(&app, artist_id, "The Dark Side of the Moon").await;

    // Update album status
    update_album_status(&app, album_id, "available").await;

    let (name, value) = app.auth_header(&token);
    let response = app
        .server()
        .get("/api/music/albums?status=available")
        .add_header(name, value)
        .await;

    response.assert_status_ok();
    let body: serde_json::Value = response.json();
    let items = body["items"].as_array().expect("items should be an array");

    // All albums should have status=available
    assert!(
        items.iter().all(|item| item["status"] == "available"),
        "All albums should have status=available"
    );
}

#[tokio::test]
async fn test_get_album() {
    let app = TestApp::new().await;
    let (_user_id, token) = app.create_user().await;

    let artist_id =
        seed_test_artist(&app, "Pink Floyd", "5441c29d-3602-4898-b1a1-b77fa23b8e50").await;
    let album_id = seed_test_album(&app, artist_id, "The Dark Side of the Moon").await;

    let (name, value) = app.auth_header(&token);
    let response = app
        .server()
        .get(&format!("/api/music/albums/{}", album_id))
        .add_header(name, value)
        .await;

    response.assert_status_ok();
    let body: serde_json::Value = response.json();

    assert_eq!(body["id"], album_id);
    assert_eq!(body["title"], "The Dark Side of the Moon");
    assert_eq!(body["artist_id"], artist_id);
    assert!(body.get("tracks").is_some());
}

#[tokio::test]
async fn test_get_nonexistent_album() {
    let app = TestApp::new().await;
    let (_user_id, token) = app.create_user().await;

    let (name, value) = app.auth_header(&token);
    let response = app
        .server()
        .get("/api/music/albums/99999")
        .add_header(name, value)
        .await;

    response.assert_status_not_found();
}

#[tokio::test]
async fn test_update_album() {
    let app = TestApp::new().await;
    let (_user_id, token) = app.create_user().await;

    let artist_id =
        seed_test_artist(&app, "Pink Floyd", "5441c29d-3602-4898-b1a1-b77fa23b8e50").await;
    let album_id = seed_test_album(&app, artist_id, "The Dark Side of the Moon").await;

    let (name, value) = app.auth_header(&token);
    let response = app
        .server()
        .put(&format!("/api/music/albums/{}", album_id))
        .add_header(name, value)
        .json(&serde_json::json!({
            "monitored": false,
            "quality_limit": "mp3"
        }))
        .await;

    response.assert_status_ok();
    let body: serde_json::Value = response.json();

    assert_eq!(body["id"], album_id);
    assert_eq!(body["monitored"], false);
    assert_eq!(body["quality_limit"], "mp3");
}

#[tokio::test]
async fn test_update_album_no_fields() {
    let app = TestApp::new().await;
    let (_user_id, token) = app.create_user().await;

    let artist_id =
        seed_test_artist(&app, "Pink Floyd", "5441c29d-3602-4898-b1a1-b77fa23b8e50").await;
    let album_id = seed_test_album(&app, artist_id, "The Dark Side of the Moon").await;

    let (name, value) = app.auth_header(&token);
    let response = app
        .server()
        .put(&format!("/api/music/albums/{}", album_id))
        .add_header(name, value)
        .json(&serde_json::json!({}))
        .await;

    response.assert_status_bad_request();
}

// =============================================================================
// Track Tests
// =============================================================================

#[tokio::test]
async fn test_list_tracks() {
    let app = TestApp::new().await;
    let (_user_id, token) = app.create_user().await;

    let artist_id =
        seed_test_artist(&app, "Pink Floyd", "5441c29d-3602-4898-b1a1-b77fa23b8e50").await;
    let album_id = seed_test_album(&app, artist_id, "The Dark Side of the Moon").await;
    seed_test_track(&app, album_id, "Speak to Me", 1).await;
    seed_test_track(&app, album_id, "Breathe", 2).await;

    let (name, value) = app.auth_header(&token);
    let response = app
        .server()
        .get("/api/music/tracks")
        .add_header(name, value)
        .await;

    response.assert_status_ok();
    let body: serde_json::Value = response.json();

    assert!(body.get("items").is_some());
    assert!(body.get("total").is_some());
    let items = body["items"].as_array().expect("items should be an array");
    assert!(items.len() >= 2, "Should have at least 2 tracks");
}

#[tokio::test]
async fn test_list_tracks_with_album_filter() {
    let app = TestApp::new().await;
    let (_user_id, token) = app.create_user().await;

    let artist_id =
        seed_test_artist(&app, "Pink Floyd", "5441c29d-3602-4898-b1a1-b77fa23b8e50").await;
    let album1_id = seed_test_album(&app, artist_id, "The Dark Side of the Moon").await;
    let album2_id = seed_test_album(&app, artist_id, "The Wall").await;

    seed_test_track(&app, album1_id, "Speak to Me", 1).await;
    seed_test_track(&app, album2_id, "In The Flesh?", 1).await;

    let (name, value) = app.auth_header(&token);
    let response = app
        .server()
        .get(&format!("/api/music/tracks?album_id={}", album1_id))
        .add_header(name, value)
        .await;

    response.assert_status_ok();
    let body: serde_json::Value = response.json();
    let items = body["items"].as_array().expect("items should be an array");

    // All tracks should belong to album1
    assert!(
        items.iter().all(|item| item["album_id"] == album1_id),
        "All tracks should belong to the filtered album"
    );
}

#[tokio::test]
async fn test_update_track() {
    let app = TestApp::new().await;
    let (_user_id, token) = app.create_user().await;

    let artist_id =
        seed_test_artist(&app, "Pink Floyd", "5441c29d-3602-4898-b1a1-b77fa23b8e50").await;
    let album_id = seed_test_album(&app, artist_id, "The Dark Side of the Moon").await;
    let track_id = seed_test_track(&app, album_id, "Speak to Me", 1).await;

    let (name, value) = app.auth_header(&token);
    let response = app
        .server()
        .put(&format!("/api/music/tracks/{}", track_id))
        .add_header(name, value)
        .json(&serde_json::json!({
            "monitored": false
        }))
        .await;

    response.assert_status_ok();
    let body: serde_json::Value = response.json();

    assert_eq!(body["id"], track_id);
    assert_eq!(body["monitored"], false);
}

#[tokio::test]
async fn test_update_track_no_fields() {
    let app = TestApp::new().await;
    let (_user_id, token) = app.create_user().await;

    let artist_id =
        seed_test_artist(&app, "Pink Floyd", "5441c29d-3602-4898-b1a1-b77fa23b8e50").await;
    let album_id = seed_test_album(&app, artist_id, "The Dark Side of the Moon").await;
    let track_id = seed_test_track(&app, album_id, "Speak to Me", 1).await;

    let (name, value) = app.auth_header(&token);
    let response = app
        .server()
        .put(&format!("/api/music/tracks/{}", track_id))
        .add_header(name, value)
        .json(&serde_json::json!({}))
        .await;

    response.assert_status_bad_request();
}

#[tokio::test]
async fn test_update_nonexistent_track() {
    let app = TestApp::new().await;
    let (_user_id, token) = app.create_user().await;

    let (name, value) = app.auth_header(&token);
    let response = app
        .server()
        .put("/api/music/tracks/99999")
        .add_header(name, value)
        .json(&serde_json::json!({
            "monitored": false
        }))
        .await;

    response.assert_status_not_found();
}

// =============================================================================
// Helper Functions
// =============================================================================

/// Seeds a test artist in the database and returns its ID.
async fn seed_test_artist(app: &TestApp, name: &str, mbid: &str) -> i64 {
    let db = app.db().lock().await;
    db.execute(
        r#"
        INSERT INTO artists (mbid, name, sort_name, monitored, quality_limit, added_by)
        VALUES (?1, ?2, ?3, 1, 'flac', 1)
        "#,
        rusqlite::params![mbid, name, name],
    )
    .expect("Failed to insert test artist");

    db.last_insert_rowid()
}

/// Seeds a test album in the database and returns its ID.
async fn seed_test_album(app: &TestApp, artist_id: i64, title: &str) -> i64 {
    let db = app.db().lock().await;
    let mbid = format!("album-{}-{}", artist_id, title.replace(' ', "-"));
    db.execute(
        r#"
        INSERT INTO albums (mbid, artist_id, title, album_type, status, monitored, quality_limit)
        VALUES (?1, ?2, ?3, 'Album', 'missing', 1, 'flac')
        "#,
        rusqlite::params![mbid, artist_id, title],
    )
    .expect("Failed to insert test album");

    db.last_insert_rowid()
}

/// Seeds a test track in the database and returns its ID.
async fn seed_test_track(app: &TestApp, album_id: i64, title: &str, track_number: i32) -> i64 {
    let db = app.db().lock().await;
    let mbid = format!("track-{}-{}", album_id, track_number);
    db.execute(
        r#"
        INSERT INTO tracks (mbid, album_id, title, track_number, disc_number, status, monitored)
        VALUES (?1, ?2, ?3, ?4, 1, 'missing', 1)
        "#,
        rusqlite::params![mbid, album_id, title, track_number],
    )
    .expect("Failed to insert test track");

    db.last_insert_rowid()
}

/// Updates an artist's monitored status.
async fn update_artist_monitored(app: &TestApp, artist_id: i64, monitored: bool) {
    let db = app.db().lock().await;
    db.execute(
        "UPDATE artists SET monitored = ?1 WHERE id = ?2",
        rusqlite::params![monitored, artist_id],
    )
    .expect("Failed to update artist monitored status");
}

/// Updates an album's status.
async fn update_album_status(app: &TestApp, album_id: i64, status: &str) {
    let db = app.db().lock().await;
    db.execute(
        "UPDATE albums SET status = ?1 WHERE id = ?2",
        rusqlite::params![status, album_id],
    )
    .expect("Failed to update album status");
}
