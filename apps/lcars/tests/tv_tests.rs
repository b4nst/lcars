//! Integration tests for TV shows endpoints.

mod common;

use common::TestApp;

// =============================================================================
// List TV Shows Tests
// =============================================================================

#[tokio::test]
async fn test_list_tv_shows() {
    let app = TestApp::new().await;
    let (_user_id, user_token) = app.create_user().await;

    // Seed some test TV shows
    let db = app.db().lock().await;
    db.execute(
        r#"
        INSERT INTO tv_shows (tmdb_id, title, status, monitored, quality_limit, added_by)
        VALUES (1399, 'Game of Thrones', 'ended', 1, '1080p', 1)
        "#,
        [],
    )
    .unwrap();
    db.execute(
        r#"
        INSERT INTO tv_shows (tmdb_id, title, status, monitored, quality_limit, added_by)
        VALUES (82856, 'The Mandalorian', 'continuing', 1, '1080p', 1)
        "#,
        [],
    )
    .unwrap();
    drop(db);

    let (name, value) = app.auth_header(&user_token);

    // List TV shows
    let response = app.server().get("/api/tv").add_header(name, value).await;

    response.assert_status_ok();
    let body: serde_json::Value = response.json();
    assert!(body["items"].is_array());
    let items = body["items"].as_array().unwrap();
    assert_eq!(items.len(), 2);
    assert!(body["total"].as_u64().unwrap() >= 2);
    assert_eq!(body["page"], 1);
}

#[tokio::test]
async fn test_list_tv_shows_unauthenticated() {
    let app = TestApp::new().await;

    // Attempt to list TV shows without authentication
    let response = app.server().get("/api/tv").await;

    response.assert_status_unauthorized();
}

#[tokio::test]
async fn test_list_tv_shows_with_status_filter() {
    let app = TestApp::new().await;
    let (_user_id, user_token) = app.create_user().await;

    // Seed test shows with different statuses
    let db = app.db().lock().await;
    db.execute(
        r#"
        INSERT INTO tv_shows (tmdb_id, title, status, monitored, quality_limit, added_by)
        VALUES (1399, 'Game of Thrones', 'ended', 1, '1080p', 1)
        "#,
        [],
    )
    .unwrap();
    db.execute(
        r#"
        INSERT INTO tv_shows (tmdb_id, title, status, monitored, quality_limit, added_by)
        VALUES (82856, 'The Mandalorian', 'continuing', 1, '1080p', 1)
        "#,
        [],
    )
    .unwrap();
    db.execute(
        r#"
        INSERT INTO tv_shows (tmdb_id, title, status, monitored, quality_limit, added_by)
        VALUES (12345, 'Canceled Show', 'canceled', 1, '1080p', 1)
        "#,
        [],
    )
    .unwrap();
    drop(db);

    let (name, value) = app.auth_header(&user_token);

    // Filter by 'ended' status
    let response = app
        .server()
        .get("/api/tv?status=ended")
        .add_header(name.clone(), value.clone())
        .await;

    response.assert_status_ok();
    let body: serde_json::Value = response.json();
    let items = body["items"].as_array().unwrap();
    assert_eq!(items.len(), 1);
    assert_eq!(items[0]["title"], "Game of Thrones");
    assert_eq!(items[0]["status"], "ended");

    // Filter by 'continuing' status
    let response = app
        .server()
        .get("/api/tv?status=continuing")
        .add_header(name, value)
        .await;

    response.assert_status_ok();
    let body: serde_json::Value = response.json();
    let items = body["items"].as_array().unwrap();
    assert_eq!(items.len(), 1);
    assert_eq!(items[0]["title"], "The Mandalorian");
    assert_eq!(items[0]["status"], "continuing");
}

#[tokio::test]
async fn test_list_tv_shows_with_monitored_filter() {
    let app = TestApp::new().await;
    let (_user_id, user_token) = app.create_user().await;

    // Seed test shows with different monitored states
    let db = app.db().lock().await;
    db.execute(
        r#"
        INSERT INTO tv_shows (tmdb_id, title, status, monitored, quality_limit, added_by)
        VALUES (1399, 'Monitored Show', 'continuing', 1, '1080p', 1)
        "#,
        [],
    )
    .unwrap();
    db.execute(
        r#"
        INSERT INTO tv_shows (tmdb_id, title, status, monitored, quality_limit, added_by)
        VALUES (82856, 'Unmonitored Show', 'continuing', 0, '1080p', 1)
        "#,
        [],
    )
    .unwrap();
    drop(db);

    let (name, value) = app.auth_header(&user_token);

    // Filter by monitored=true
    let response = app
        .server()
        .get("/api/tv?monitored=true")
        .add_header(name.clone(), value.clone())
        .await;

    response.assert_status_ok();
    let body: serde_json::Value = response.json();
    let items = body["items"].as_array().unwrap();
    assert_eq!(items.len(), 1);
    assert_eq!(items[0]["title"], "Monitored Show");
    assert_eq!(items[0]["monitored"], true);

    // Filter by monitored=false
    let response = app
        .server()
        .get("/api/tv?monitored=false")
        .add_header(name, value)
        .await;

    response.assert_status_ok();
    let body: serde_json::Value = response.json();
    let items = body["items"].as_array().unwrap();
    assert_eq!(items.len(), 1);
    assert_eq!(items[0]["title"], "Unmonitored Show");
    assert_eq!(items[0]["monitored"], false);
}

#[tokio::test]
async fn test_list_tv_shows_with_pagination() {
    let app = TestApp::new().await;
    let (_user_id, user_token) = app.create_user().await;

    // Seed multiple shows
    let db = app.db().lock().await;
    for i in 1..=5 {
        db.execute(
            r#"
            INSERT INTO tv_shows (tmdb_id, title, status, monitored, quality_limit, added_by)
            VALUES (?, ?, 'continuing', 1, '1080p', 1)
            "#,
            rusqlite::params![1000 + i, format!("Show {}", i)],
        )
        .unwrap();
    }
    drop(db);

    let (name, value) = app.auth_header(&user_token);

    // Request page 1 with limit of 2
    let response = app
        .server()
        .get("/api/tv?page=1&limit=2")
        .add_header(name.clone(), value.clone())
        .await;

    response.assert_status_ok();
    let body: serde_json::Value = response.json();
    assert_eq!(body["items"].as_array().unwrap().len(), 2);
    assert_eq!(body["page"], 1);
    assert_eq!(body["total"], 5);
    assert_eq!(body["pages"], 3);

    // Request page 2
    let response = app
        .server()
        .get("/api/tv?page=2&limit=2")
        .add_header(name, value)
        .await;

    response.assert_status_ok();
    let body: serde_json::Value = response.json();
    assert_eq!(body["items"].as_array().unwrap().len(), 2);
    assert_eq!(body["page"], 2);
}

// =============================================================================
// Add TV Show Tests
// =============================================================================

#[tokio::test]
async fn test_add_tv_show_without_tmdb() {
    let app = TestApp::new().await;
    let (_user_id, user_token) = app.create_user().await;

    let (name, value) = app.auth_header(&user_token);

    // Attempt to add a show (will fail without TMDB client)
    let response = app
        .server()
        .post("/api/tv")
        .add_header(name, value)
        .json(&serde_json::json!({
            "tmdb_id": 1399,
            "monitored": true,
            "quality_limit": "1080p"
        }))
        .await;

    // Should fail because TMDB client is not configured in tests
    response.assert_status(axum::http::StatusCode::INTERNAL_SERVER_ERROR);
}

#[tokio::test]
async fn test_add_tv_show_invalid_tmdb_id() {
    let app = TestApp::new().await;
    let (_user_id, user_token) = app.create_user().await;

    let (name, value) = app.auth_header(&user_token);

    // Attempt to add a show with invalid TMDB ID
    let response = app
        .server()
        .post("/api/tv")
        .add_header(name, value)
        .json(&serde_json::json!({
            "tmdb_id": -1,
            "monitored": true
        }))
        .await;

    response.assert_status_bad_request();
}

// =============================================================================
// Get TV Show Tests
// =============================================================================

#[tokio::test]
async fn test_get_tv_show() {
    let app = TestApp::new().await;
    let (_user_id, user_token) = app.create_user().await;

    // Seed a test show
    let db = app.db().lock().await;
    db.execute(
        r#"
        INSERT INTO tv_shows (tmdb_id, title, original_title, year_start, status, monitored, quality_limit, added_by)
        VALUES (1399, 'Game of Thrones', 'Game of Thrones', 2011, 'ended', 1, '1080p', 1)
        "#,
        [],
    )
    .unwrap();
    let show_id = db.last_insert_rowid();

    // Add some episodes
    db.execute(
        r#"
        INSERT INTO episodes (show_id, season_number, episode_number, title, status, monitored)
        VALUES (?, 1, 1, 'Winter Is Coming', 'missing', 1)
        "#,
        [show_id],
    )
    .unwrap();
    db.execute(
        r#"
        INSERT INTO episodes (show_id, season_number, episode_number, title, status, monitored)
        VALUES (?, 1, 2, 'The Kingsroad', 'available', 1)
        "#,
        [show_id],
    )
    .unwrap();
    drop(db);

    let (name, value) = app.auth_header(&user_token);

    // Get the show
    let response = app
        .server()
        .get(&format!("/api/tv/{}", show_id))
        .add_header(name, value)
        .await;

    response.assert_status_ok();
    let body: serde_json::Value = response.json();
    assert_eq!(body["id"], show_id);
    assert_eq!(body["title"], "Game of Thrones");
    assert_eq!(body["tmdb_id"], 1399);
    assert_eq!(body["status"], "ended");
    assert_eq!(body["monitored"], true);

    // Verify seasons are included
    assert!(body["seasons"].is_array());
    let seasons = body["seasons"].as_array().unwrap();
    assert_eq!(seasons.len(), 1);
    assert_eq!(seasons[0]["season_number"], 1);
    assert_eq!(seasons[0]["total_count"], 2);
    assert_eq!(seasons[0]["available_count"], 1);
    assert_eq!(seasons[0]["episodes"].as_array().unwrap().len(), 2);
}

#[tokio::test]
async fn test_get_nonexistent_show() {
    let app = TestApp::new().await;
    let (_user_id, user_token) = app.create_user().await;

    let (name, value) = app.auth_header(&user_token);

    // Attempt to get non-existent show
    let response = app
        .server()
        .get("/api/tv/99999")
        .add_header(name, value)
        .await;

    response.assert_status_not_found();
}

// =============================================================================
// Update TV Show Tests
// =============================================================================

#[tokio::test]
async fn test_update_tv_show() {
    let app = TestApp::new().await;
    let (_user_id, user_token) = app.create_user().await;

    // Seed a test show
    let db = app.db().lock().await;
    db.execute(
        r#"
        INSERT INTO tv_shows (tmdb_id, title, status, monitored, quality_limit, added_by)
        VALUES (1399, 'Game of Thrones', 'continuing', 1, '1080p', 1)
        "#,
        [],
    )
    .unwrap();
    let show_id = db.last_insert_rowid();
    drop(db);

    let (name, value) = app.auth_header(&user_token);

    // Update the show
    let response = app
        .server()
        .put(&format!("/api/tv/{}", show_id))
        .add_header(name, value)
        .json(&serde_json::json!({
            "monitored": false,
            "quality_limit": "720p"
        }))
        .await;

    response.assert_status_ok();
    let body: serde_json::Value = response.json();
    assert_eq!(body["id"], show_id);
    assert_eq!(body["monitored"], false);
    assert_eq!(body["quality_limit"], "720p");

    // Verify in database
    let db = app.db().lock().await;
    let (monitored, quality_limit): (bool, String) = db
        .query_row(
            "SELECT monitored, quality_limit FROM tv_shows WHERE id = ?1",
            [show_id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .unwrap();
    assert!(!monitored);
    assert_eq!(quality_limit, "720p");
}

#[tokio::test]
async fn test_update_nonexistent_show() {
    let app = TestApp::new().await;
    let (_user_id, user_token) = app.create_user().await;

    let (name, value) = app.auth_header(&user_token);

    // Attempt to update non-existent show
    let response = app
        .server()
        .put("/api/tv/99999")
        .add_header(name, value)
        .json(&serde_json::json!({
            "monitored": false
        }))
        .await;

    response.assert_status_not_found();
}

#[tokio::test]
async fn test_update_tv_show_no_fields() {
    let app = TestApp::new().await;
    let (_user_id, user_token) = app.create_user().await;

    // Seed a test show
    let db = app.db().lock().await;
    db.execute(
        r#"
        INSERT INTO tv_shows (tmdb_id, title, status, monitored, quality_limit, added_by)
        VALUES (1399, 'Game of Thrones', 'continuing', 1, '1080p', 1)
        "#,
        [],
    )
    .unwrap();
    let show_id = db.last_insert_rowid();
    drop(db);

    let (name, value) = app.auth_header(&user_token);

    // Attempt to update with no fields
    let response = app
        .server()
        .put(&format!("/api/tv/{}", show_id))
        .add_header(name, value)
        .json(&serde_json::json!({}))
        .await;

    response.assert_status_bad_request();
}

// =============================================================================
// Delete TV Show Tests
// =============================================================================

#[tokio::test]
async fn test_delete_tv_show() {
    let app = TestApp::new().await;
    let (_user_id, user_token) = app.create_user().await;

    // Seed a test show
    let db = app.db().lock().await;
    db.execute(
        r#"
        INSERT INTO tv_shows (tmdb_id, title, status, monitored, quality_limit, added_by)
        VALUES (1399, 'Game of Thrones', 'ended', 1, '1080p', 1)
        "#,
        [],
    )
    .unwrap();
    let show_id = db.last_insert_rowid();

    // Add episodes
    db.execute(
        r#"
        INSERT INTO episodes (show_id, season_number, episode_number, title, status, monitored)
        VALUES (?, 1, 1, 'Winter Is Coming', 'missing', 1)
        "#,
        [show_id],
    )
    .unwrap();
    drop(db);

    let (name, value) = app.auth_header(&user_token);

    // Delete the show
    let response = app
        .server()
        .delete(&format!("/api/tv/{}", show_id))
        .add_header(name, value)
        .await;

    response.assert_status_ok();
    let body: serde_json::Value = response.json();
    assert_eq!(body["success"], true);

    // Verify show is deleted
    let db = app.db().lock().await;
    let exists: bool = db
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM tv_shows WHERE id = ?1)",
            [show_id],
            |row| row.get(0),
        )
        .unwrap_or(false);
    assert!(!exists);

    // Verify episodes are also deleted (CASCADE)
    let episode_count: i64 = db
        .query_row(
            "SELECT COUNT(*) FROM episodes WHERE show_id = ?1",
            [show_id],
            |row| row.get(0),
        )
        .unwrap_or(0);
    assert_eq!(episode_count, 0);
}

#[tokio::test]
async fn test_delete_nonexistent_show() {
    let app = TestApp::new().await;
    let (_user_id, user_token) = app.create_user().await;

    let (name, value) = app.auth_header(&user_token);

    // Attempt to delete non-existent show
    let response = app
        .server()
        .delete("/api/tv/99999")
        .add_header(name, value)
        .await;

    response.assert_status_not_found();
}

// =============================================================================
// Season Operations Tests
// =============================================================================

#[tokio::test]
async fn test_get_season_episodes() {
    let app = TestApp::new().await;
    let (_user_id, user_token) = app.create_user().await;

    // Seed a test show with episodes
    let db = app.db().lock().await;
    db.execute(
        r#"
        INSERT INTO tv_shows (tmdb_id, title, status, monitored, quality_limit, added_by)
        VALUES (1399, 'Game of Thrones', 'ended', 1, '1080p', 1)
        "#,
        [],
    )
    .unwrap();
    let show_id = db.last_insert_rowid();

    // Add season 1 episodes
    db.execute(
        r#"
        INSERT INTO episodes (show_id, season_number, episode_number, title, status, monitored)
        VALUES (?, 1, 1, 'Winter Is Coming', 'available', 1)
        "#,
        [show_id],
    )
    .unwrap();
    db.execute(
        r#"
        INSERT INTO episodes (show_id, season_number, episode_number, title, status, monitored)
        VALUES (?, 1, 2, 'The Kingsroad', 'missing', 1)
        "#,
        [show_id],
    )
    .unwrap();

    // Add season 2 episode
    db.execute(
        r#"
        INSERT INTO episodes (show_id, season_number, episode_number, title, status, monitored)
        VALUES (?, 2, 1, 'The North Remembers', 'missing', 1)
        "#,
        [show_id],
    )
    .unwrap();
    drop(db);

    let (name, value) = app.auth_header(&user_token);

    // Get season 1 episodes
    let response = app
        .server()
        .get(&format!("/api/tv/{}/season/1", show_id))
        .add_header(name, value)
        .await;

    response.assert_status_ok();
    let body: serde_json::Value = response.json();
    assert_eq!(body["season_number"], 1);
    assert_eq!(body["total_count"], 2);
    assert_eq!(body["available_count"], 1);

    let episodes = body["episodes"].as_array().unwrap();
    assert_eq!(episodes.len(), 2);
    assert_eq!(episodes[0]["episode_number"], 1);
    assert_eq!(episodes[0]["title"], "Winter Is Coming");
    assert_eq!(episodes[1]["episode_number"], 2);
}

#[tokio::test]
async fn test_get_nonexistent_season() {
    let app = TestApp::new().await;
    let (_user_id, user_token) = app.create_user().await;

    // Seed a test show without season 5
    let db = app.db().lock().await;
    db.execute(
        r#"
        INSERT INTO tv_shows (tmdb_id, title, status, monitored, quality_limit, added_by)
        VALUES (1399, 'Game of Thrones', 'ended', 1, '1080p', 1)
        "#,
        [],
    )
    .unwrap();
    let show_id = db.last_insert_rowid();
    drop(db);

    let (name, value) = app.auth_header(&user_token);

    // Attempt to get non-existent season
    let response = app
        .server()
        .get(&format!("/api/tv/{}/season/5", show_id))
        .add_header(name, value)
        .await;

    response.assert_status_not_found();
}

#[tokio::test]
async fn test_update_season() {
    let app = TestApp::new().await;
    let (_user_id, user_token) = app.create_user().await;

    // Seed a test show with episodes
    let db = app.db().lock().await;
    db.execute(
        r#"
        INSERT INTO tv_shows (tmdb_id, title, status, monitored, quality_limit, added_by)
        VALUES (1399, 'Game of Thrones', 'ended', 1, '1080p', 1)
        "#,
        [],
    )
    .unwrap();
    let show_id = db.last_insert_rowid();

    // Add season 1 episodes (monitored by default)
    db.execute(
        r#"
        INSERT INTO episodes (show_id, season_number, episode_number, title, status, monitored)
        VALUES (?, 1, 1, 'Winter Is Coming', 'missing', 1)
        "#,
        [show_id],
    )
    .unwrap();
    db.execute(
        r#"
        INSERT INTO episodes (show_id, season_number, episode_number, title, status, monitored)
        VALUES (?, 1, 2, 'The Kingsroad', 'missing', 1)
        "#,
        [show_id],
    )
    .unwrap();
    drop(db);

    let (name, value) = app.auth_header(&user_token);

    // Update season to unmonitored
    let response = app
        .server()
        .put(&format!("/api/tv/{}/season/1", show_id))
        .add_header(name, value)
        .json(&serde_json::json!({
            "monitored": false
        }))
        .await;

    response.assert_status_ok();
    let body: serde_json::Value = response.json();
    assert_eq!(body["season_number"], 1);

    let episodes = body["episodes"].as_array().unwrap();
    assert_eq!(episodes.len(), 2);
    // All episodes should now be unmonitored
    assert_eq!(episodes[0]["monitored"], false);
    assert_eq!(episodes[1]["monitored"], false);

    // Verify in database
    let db = app.db().lock().await;
    let monitored_count: i64 = db
        .query_row(
            "SELECT COUNT(*) FROM episodes WHERE show_id = ?1 AND season_number = 1 AND monitored = 1",
            [show_id],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(monitored_count, 0);
}

// =============================================================================
// Episode Operations Tests
// =============================================================================

#[tokio::test]
async fn test_update_episode() {
    let app = TestApp::new().await;
    let (_user_id, user_token) = app.create_user().await;

    // Seed a test show with an episode
    let db = app.db().lock().await;
    db.execute(
        r#"
        INSERT INTO tv_shows (tmdb_id, title, status, monitored, quality_limit, added_by)
        VALUES (1399, 'Game of Thrones', 'ended', 1, '1080p', 1)
        "#,
        [],
    )
    .unwrap();
    let show_id = db.last_insert_rowid();

    db.execute(
        r#"
        INSERT INTO episodes (show_id, season_number, episode_number, title, status, monitored)
        VALUES (?, 1, 1, 'Winter Is Coming', 'missing', 1)
        "#,
        [show_id],
    )
    .unwrap();
    let episode_id = db.last_insert_rowid();
    drop(db);

    let (name, value) = app.auth_header(&user_token);

    // Update the episode
    let response = app
        .server()
        .put(&format!("/api/tv/{}/season/1/episode/1", show_id))
        .add_header(name, value)
        .json(&serde_json::json!({
            "monitored": false
        }))
        .await;

    response.assert_status_ok();
    let body: serde_json::Value = response.json();
    assert_eq!(body["id"], episode_id);
    assert_eq!(body["monitored"], false);
    assert_eq!(body["season_number"], 1);
    assert_eq!(body["episode_number"], 1);

    // Verify in database
    let db = app.db().lock().await;
    let monitored: bool = db
        .query_row(
            "SELECT monitored FROM episodes WHERE id = ?1",
            [episode_id],
            |row| row.get(0),
        )
        .unwrap();
    assert!(!monitored);
}

#[tokio::test]
async fn test_update_nonexistent_episode() {
    let app = TestApp::new().await;
    let (_user_id, user_token) = app.create_user().await;

    // Seed a test show without episodes
    let db = app.db().lock().await;
    db.execute(
        r#"
        INSERT INTO tv_shows (tmdb_id, title, status, monitored, quality_limit, added_by)
        VALUES (1399, 'Game of Thrones', 'ended', 1, '1080p', 1)
        "#,
        [],
    )
    .unwrap();
    let show_id = db.last_insert_rowid();
    drop(db);

    let (name, value) = app.auth_header(&user_token);

    // Attempt to update non-existent episode
    let response = app
        .server()
        .put(&format!("/api/tv/{}/season/1/episode/1", show_id))
        .add_header(name, value)
        .json(&serde_json::json!({
            "monitored": false
        }))
        .await;

    response.assert_status_not_found();
}

// =============================================================================
// Search and Download Tests (require external services)
// =============================================================================

#[tokio::test]
async fn test_search_episode_without_indexer() {
    let app = TestApp::new().await;
    let (_user_id, user_token) = app.create_user().await;

    // Seed a test show with an episode
    let db = app.db().lock().await;
    db.execute(
        r#"
        INSERT INTO tv_shows (tmdb_id, title, status, monitored, quality_limit, added_by)
        VALUES (1399, 'Game of Thrones', 'ended', 1, '1080p', 1)
        "#,
        [],
    )
    .unwrap();
    let show_id = db.last_insert_rowid();

    db.execute(
        r#"
        INSERT INTO episodes (show_id, season_number, episode_number, title, status, monitored)
        VALUES (?, 1, 1, 'Winter Is Coming', 'missing', 1)
        "#,
        [show_id],
    )
    .unwrap();
    drop(db);

    let (name, value) = app.auth_header(&user_token);

    // Search for episode (will return empty results without configured indexers)
    let response = app
        .server()
        .post(&format!("/api/tv/{}/season/1/episode/1/search", show_id))
        .add_header(name, value)
        .await;

    response.assert_status_ok();
    let body: serde_json::Value = response.json();
    assert!(body.is_array());
    // Should be empty array without configured indexers
    assert_eq!(body.as_array().unwrap().len(), 0);
}

#[tokio::test]
async fn test_download_episode_without_torrent_engine() {
    let app = TestApp::new().await;
    let (_user_id, user_token) = app.create_user().await;

    // Seed a test show with an episode
    let db = app.db().lock().await;
    db.execute(
        r#"
        INSERT INTO tv_shows (tmdb_id, title, status, monitored, quality_limit, added_by)
        VALUES (1399, 'Game of Thrones', 'ended', 1, '1080p', 1)
        "#,
        [],
    )
    .unwrap();
    let show_id = db.last_insert_rowid();

    db.execute(
        r#"
        INSERT INTO episodes (show_id, season_number, episode_number, title, status, monitored)
        VALUES (?, 1, 1, 'Winter Is Coming', 'missing', 1)
        "#,
        [show_id],
    )
    .unwrap();
    drop(db);

    let (name, value) = app.auth_header(&user_token);

    // Attempt to download episode (will fail without torrent engine)
    let response = app
        .server()
        .post(&format!("/api/tv/{}/season/1/episode/1/download", show_id))
        .add_header(name, value)
        .json(&serde_json::json!({
            "magnet": "magnet:?xt=urn:btih:1234567890abcdef1234567890abcdef12345678"
        }))
        .await;

    // Should fail because torrent engine is not available in tests
    response.assert_status(axum::http::StatusCode::INTERNAL_SERVER_ERROR);
}

#[tokio::test]
async fn test_download_episode_invalid_magnet() {
    let app = TestApp::new().await;
    let (_user_id, user_token) = app.create_user().await;

    // Seed a test show with an episode
    let db = app.db().lock().await;
    db.execute(
        r#"
        INSERT INTO tv_shows (tmdb_id, title, status, monitored, quality_limit, added_by)
        VALUES (1399, 'Game of Thrones', 'ended', 1, '1080p', 1)
        "#,
        [],
    )
    .unwrap();
    let show_id = db.last_insert_rowid();

    db.execute(
        r#"
        INSERT INTO episodes (show_id, season_number, episode_number, title, status, monitored)
        VALUES (?, 1, 1, 'Winter Is Coming', 'missing', 1)
        "#,
        [show_id],
    )
    .unwrap();
    drop(db);

    let (name, value) = app.auth_header(&user_token);

    // Attempt to download with invalid magnet link
    let response = app
        .server()
        .post(&format!("/api/tv/{}/season/1/episode/1/download", show_id))
        .add_header(name, value)
        .json(&serde_json::json!({
            "magnet": "not-a-valid-magnet-link"
        }))
        .await;

    response.assert_status_bad_request();
}
