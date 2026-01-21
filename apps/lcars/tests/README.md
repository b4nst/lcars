# LCARS Backend Integration Tests

This directory contains integration tests for the LCARS backend API.

## Structure

```
tests/
├── common/
│   └── mod.rs          # Test infrastructure and helper functions
├── auth_tests.rs       # Authentication endpoint tests
├── users_tests.rs      # User management tests (admin only)
├── movies_test.rs      # Movies endpoint tests
└── README.md           # This file
```

## Test Infrastructure

The `common/mod.rs` module provides a `TestApp` struct that sets up a complete test environment:

- **In-memory SQLite database** with all migrations applied
- **Test configuration** with default values
- **Complete API router** matching production routes
- **Helper methods** for creating users and generating auth tokens

### TestApp API

```rust
// Create a new test application
let app = TestApp::new().await;

// Create test users
let user_id = app.create_test_user("username", "password", "user").await;
let (user_id, token) = app.create_user().await;  // Convenience method
let (admin_id, token) = app.create_admin().await;  // Creates admin user

// Generate auth tokens
let token = app.get_auth_token(user_id, "user");
let (name, value) = app.auth_header(&token);

// Make HTTP requests
let response = app.server().get("/api/movies")
    .add_header(name, value)
    .await;

// Access database directly
let db = app.db().lock().await;
let count: i64 = db.query_row("SELECT COUNT(*) FROM users", [], |row| row.get(0))?;
```

## Writing Tests

### Basic Test Structure

```rust
mod common;
use common::TestApp;

#[tokio::test]
async fn test_my_endpoint() {
    // Setup
    let app = TestApp::new().await;
    let (_user_id, token) = app.create_user().await;
    let (name, value) = app.auth_header(&token);

    // Execute
    let response = app.server()
        .get("/api/endpoint")
        .add_header(name, value)
        .await;

    // Assert
    response.assert_status_ok();
    let body: serde_json::Value = response.json();
    assert_eq!(body["field"], "expected_value");
}
```

### Testing Authentication

```rust
#[tokio::test]
async fn test_requires_auth() {
    let app = TestApp::new().await;

    // Request without auth should fail
    let response = app.server().get("/api/protected").await;
    response.assert_status_unauthorized();
}

#[tokio::test]
async fn test_requires_admin() {
    let app = TestApp::new().await;

    // Regular user should be forbidden
    let (_user_id, user_token) = app.create_user().await;
    let (name, value) = app.auth_header(&user_token);

    let response = app.server()
        .get("/api/admin-only")
        .add_header(name, value)
        .await;

    response.assert_status_forbidden();
}
```

### Testing with Database State

```rust
#[tokio::test]
async fn test_with_existing_data() {
    let app = TestApp::new().await;

    // Seed database
    let db = app.db().lock().await;
    db.execute(
        "INSERT INTO movies (title, year) VALUES (?1, ?2)",
        ["Test Movie", "2024"]
    ).unwrap();
    drop(db);  // Release lock

    // Make request
    let (_user_id, token) = app.create_user().await;
    let (name, value) = app.auth_header(&token);
    let response = app.server()
        .get("/api/movies")
        .add_header(name, value)
        .await;

    // Verify
    response.assert_status_ok();
}
```

## Running Tests

```bash
# Run all tests
cargo test

# Run tests in a specific file
cargo test --test auth_tests

# Run a specific test
cargo test --test auth_tests test_login_success

# Run tests with output
cargo test -- --nocapture

# Run tests with logging
RUST_LOG=debug cargo test -- --nocapture
```

## Important Notes

### Path Parameter Syntax

The test router uses `:id` syntax for path parameters (e.g., `/:id`) instead of `{id}` syntax used in `main.rs`. This is required for compatibility with `axum-test`. Both syntaxes are valid in Axum 0.7, but axum-test requires the colon syntax for proper route matching.

```rust
// In main.rs (production)
.route("/{id}", get(handler))

// In tests/common/mod.rs
.route("/:id", get(handler))
```

### Test Isolation

Each test gets a fresh in-memory database, so tests are completely isolated from each other. There's no need to clean up state between tests.

### Optional Services

The test environment initializes `AppState` with:
- ✅ Database (in-memory)
- ✅ Config (test defaults)
- ✅ AuthService
- ✅ IndexerManager
- ❌ TmdbClient (None)
- ❌ MusicBrainzClient (None)
- ❌ TorrentEngine (None)
- ❌ Scheduler (None)
- ❌ StorageManager (None)

This provides isolation and faster test execution. Tests for endpoints requiring these services should mock or skip them.

## Examples

See the existing test files for examples:
- `auth_tests.rs` - Login, logout, token validation
- `users_tests.rs` - CRUD operations, role-based access control
- `movies_test.rs` - Basic endpoint access patterns
