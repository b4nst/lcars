//! Test infrastructure for LCARS backend integration tests.
//!
//! Provides a `TestApp` wrapper around `axum_test::TestServer` with helper methods
//! for creating users, generating auth tokens, and making authenticated requests.

use axum::{
    middleware as axum_mw,
    routing::{get, post, put},
    Router,
};
use axum_test::TestServer;
use rusqlite::Connection;
use std::sync::Arc;
use tokio::sync::Mutex;

use backend::services::{AuthService, IndexerManager};
use backend::{config::Config, db, AppState};

/// Test application wrapper around axum_test::TestServer.
///
/// Provides helper methods for creating test users, generating auth tokens,
/// and making authenticated HTTP requests.
pub struct TestApp {
    server: TestServer,
    db: Arc<Mutex<Connection>>,
    auth_service: Arc<AuthService>,
}

impl TestApp {
    /// Create a new test application with in-memory database.
    ///
    /// Sets up a complete LCARS backend with:
    /// - In-memory SQLite database with all migrations applied
    /// - Test configuration with default values
    /// - AuthService with test JWT secret
    /// - Complete router identical to production
    ///
    /// Optional services (TMDB, MusicBrainz, torrent engine, scheduler, storage)
    /// are initialized as None for test isolation.
    pub async fn new() -> Self {
        // Initialize in-memory database
        let conn = db::init_db_memory().expect("Failed to initialize test database");
        let db = Arc::new(Mutex::new(conn));

        // Create test configuration
        let config = Config {
            server: backend::config::ServerConfig {
                host: "127.0.0.1".to_string(),
                port: 0,
                jwt_secret: Some("test-jwt-secret-for-integration-tests".to_string()),
            },
            database: backend::config::DatabaseConfig {
                path: ":memory:".into(),
            },
            tmdb: Default::default(),
            musicbrainz: Default::default(),
            torrent: Default::default(),
            soulseek: Default::default(),
            storage: Default::default(),
            scheduler: Default::default(),
        };

        // Create auth service with test secret
        let auth_service = Arc::new(AuthService::new(
            "test-jwt-secret-for-integration-tests".to_string(),
        ));

        // Create indexer manager
        let indexer_manager = IndexerManager::new_shared();

        // Create application state (without optional services for test isolation)
        let state = AppState {
            config: Arc::new(config),
            db: Arc::clone(&db),
            auth_service: Arc::clone(&auth_service),
            tmdb_client: None,
            musicbrainz_client: None,
            indexer_manager,
            torrent_engine: None,
            soulseek_engine: None,
            scheduler: None,
            start_time: std::time::Instant::now(),
            storage_manager: None,
        };

        // Build router identical to main.rs
        let app = Self::build_router(state);

        // Create test server
        let server = TestServer::new(app).expect("Failed to create test server");

        Self {
            server,
            db,
            auth_service,
        }
    }

    /// Build the complete application router.
    ///
    /// This mirrors the router construction in main.rs to ensure
    /// integration tests run against the actual production routes.
    ///
    /// Note: Path parameters use `:id` syntax instead of `{id}` for compatibility
    /// with axum-test. Both syntaxes are valid in Axum 0.7, but axum-test requires
    /// the colon syntax for proper route matching in test environments.
    fn build_router(state: AppState) -> Router {
        // Build auth routes (public)
        let auth_routes = Router::new()
            .route("/login", post(backend::api::auth::login))
            .route(
                "/logout",
                post(backend::api::auth::logout).layer(axum_mw::from_fn_with_state(
                    state.clone(),
                    backend::middleware::auth_middleware,
                )),
            )
            .route(
                "/me",
                get(backend::api::auth::me).layer(axum_mw::from_fn_with_state(
                    state.clone(),
                    backend::middleware::auth_middleware,
                )),
            );

        // Build user routes (admin only)
        let user_routes = Router::new()
            .route(
                "/",
                get(backend::api::users::list_users).post(backend::api::users::create_user),
            )
            .route(
                "/:id",
                put(backend::api::users::update_user).delete(backend::api::users::delete_user),
            )
            .layer(axum_mw::from_fn(backend::middleware::require_admin))
            .layer(axum_mw::from_fn_with_state(
                state.clone(),
                backend::middleware::auth_middleware,
            ));

        // Build system routes - some authenticated, some admin only
        let system_auth_routes = Router::new()
            .route("/status", get(backend::api::system::get_system_status))
            .route("/activity", get(backend::api::system::get_activity))
            .layer(axum_mw::from_fn_with_state(
                state.clone(),
                backend::middleware::auth_middleware,
            ));

        let system_admin_routes = Router::new()
            .route("/jobs", get(backend::api::system::list_jobs))
            .route("/jobs/:name/run", post(backend::api::system::trigger_job))
            .route(
                "/indexers",
                get(backend::api::system::list_indexers).post(backend::api::system::create_indexer),
            )
            .route(
                "/indexers/:id",
                put(backend::api::system::update_indexer)
                    .delete(backend::api::system::delete_indexer),
            )
            .route(
                "/indexers/:id/test",
                post(backend::api::system::test_indexer),
            )
            .route("/storage/mounts", get(backend::api::system::list_mounts))
            .route(
                "/storage/mounts/:name/test",
                post(backend::api::system::test_mount),
            )
            .layer(axum_mw::from_fn(backend::middleware::require_admin))
            .layer(axum_mw::from_fn_with_state(
                state.clone(),
                backend::middleware::auth_middleware,
            ));

        let system_routes = Router::new()
            .merge(system_auth_routes)
            .merge(system_admin_routes);

        // Build movies routes (authenticated)
        let movies_routes = Router::new()
            .route(
                "/",
                get(backend::api::movies::list_movies).post(backend::api::movies::add_movie),
            )
            .route(
                "/:id",
                get(backend::api::movies::get_movie)
                    .put(backend::api::movies::update_movie)
                    .delete(backend::api::movies::delete_movie),
            )
            .route("/:id/search", post(backend::api::movies::search_releases))
            .route(
                "/:id/download",
                post(backend::api::movies::download_release),
            )
            .route("/:id/refresh", post(backend::api::movies::refresh_metadata))
            .layer(axum_mw::from_fn_with_state(
                state.clone(),
                backend::middleware::auth_middleware,
            ));

        // Build TV shows routes (authenticated)
        // Note: Using :id syntax instead of {id} for axum-test compatibility
        let tv_routes = Router::new()
            .route(
                "/",
                get(backend::api::tv::list_shows).post(backend::api::tv::add_show),
            )
            .route(
                "/:id",
                get(backend::api::tv::get_show)
                    .put(backend::api::tv::update_show)
                    .delete(backend::api::tv::delete_show),
            )
            .route("/:id/refresh", post(backend::api::tv::refresh_metadata))
            .route(
                "/:id/season/:season",
                get(backend::api::tv::get_season).put(backend::api::tv::update_season),
            )
            .route(
                "/:id/season/:season/episode/:episode",
                put(backend::api::tv::update_episode),
            )
            .route(
                "/:id/season/:season/episode/:episode/search",
                post(backend::api::tv::search_episode),
            )
            .route(
                "/:id/season/:season/episode/:episode/download",
                post(backend::api::tv::download_episode),
            )
            .layer(axum_mw::from_fn_with_state(
                state.clone(),
                backend::middleware::auth_middleware,
            ));

        // Build music routes (authenticated)
        let music_routes = backend::api::music::router(state.clone());

        // Build downloads routes (authenticated)
        let downloads_routes = backend::api::downloads::router(state.clone());

        // Build search routes (authenticated)
        let search_routes = Router::new()
            .route(
                "/musicbrainz/artists",
                get(backend::api::search::search_mb_artists),
            )
            .route(
                "/musicbrainz/albums",
                get(backend::api::search::search_mb_albums),
            )
            .layer(axum_mw::from_fn_with_state(
                state.clone(),
                backend::middleware::auth_middleware,
            ));

        // Build main router with state
        Router::new()
            .route("/health", get(backend::health_check))
            .nest("/api/auth", auth_routes)
            .nest("/api/users", user_routes)
            .nest("/api/movies", movies_routes)
            .nest("/api/tv", tv_routes)
            .nest("/api/music", music_routes)
            .nest("/api/downloads", downloads_routes)
            .nest("/api/search", search_routes)
            .nest("/api/system", system_routes)
            .route("/api/ws", get(backend::api::ws::ws_handler))
            .with_state(state)
    }

    /// Get a reference to the test server.
    ///
    /// Use this to make HTTP requests:
    /// ```ignore
    /// let response = app.server().get("/health").await;
    /// ```
    pub fn server(&self) -> &TestServer {
        &self.server
    }

    /// Get a reference to the database connection.
    ///
    /// Useful for seeding test data or verifying database state.
    #[allow(dead_code)]
    pub fn db(&self) -> &Arc<Mutex<Connection>> {
        &self.db
    }

    /// Get a reference to the auth service.
    ///
    /// Useful for generating tokens or verifying passwords.
    #[allow(dead_code)]
    pub fn auth_service(&self) -> &Arc<AuthService> {
        &self.auth_service
    }

    /// Create a test user in the database.
    ///
    /// Returns the user_id of the created user.
    ///
    /// # Arguments
    /// * `username` - The username for the user
    /// * `password` - The plain-text password (will be hashed)
    /// * `role` - The user role ("admin" or "user")
    ///
    /// # Example
    /// ```ignore
    /// let user_id = app.create_test_user("testuser", "password123", "user").await;
    /// ```
    pub async fn create_test_user(&self, username: &str, password: &str, role: &str) -> i64 {
        let password_hash = self
            .auth_service
            .hash_password(password)
            .expect("Failed to hash password");

        let db = self.db.lock().await;
        db.execute(
            "INSERT INTO users (username, password_hash, role) VALUES (?1, ?2, ?3)",
            rusqlite::params![username, password_hash, role],
        )
        .expect("Failed to create test user");

        db.last_insert_rowid()
    }

    /// Generate a JWT token for the given user.
    ///
    /// # Arguments
    /// * `user_id` - The user ID to encode in the token
    /// * `role` - The user role to encode in the token
    ///
    /// # Example
    /// ```ignore
    /// let token = app.get_auth_token(1, "admin");
    /// ```
    pub fn get_auth_token(&self, user_id: i64, role: &str) -> String {
        self.auth_service
            .create_token(user_id, role)
            .expect("Failed to create token")
    }

    /// Create an Authorization header tuple for use with HTTP requests.
    ///
    /// Returns a tuple of (HeaderName, HeaderValue) suitable for passing to
    /// axum_test request builders.
    ///
    /// # Arguments
    /// * `token` - The JWT token to use
    ///
    /// # Example
    /// ```ignore
    /// let token = app.get_auth_token(user_id, "user");
    /// let (name, value) = app.auth_header(&token);
    /// let response = app.server().get("/api/movies").add_header(name, value).await;
    /// ```
    pub fn auth_header(&self, token: &str) -> (axum::http::HeaderName, axum::http::HeaderValue) {
        use axum::http::{header::AUTHORIZATION, HeaderValue};
        (
            AUTHORIZATION,
            HeaderValue::from_str(&format!("Bearer {}", token)).expect("Invalid token format"),
        )
    }

    /// Create a test admin user and return their ID and auth token.
    ///
    /// Convenience method that combines user creation and token generation.
    ///
    /// # Example
    /// ```ignore
    /// let (admin_id, admin_token) = app.create_admin().await;
    /// ```
    pub async fn create_admin(&self) -> (i64, String) {
        let user_id = self.create_test_user("admin", "adminpass", "admin").await;
        let token = self.get_auth_token(user_id, "admin");
        (user_id, token)
    }

    /// Create a test regular user and return their ID and auth token.
    ///
    /// Convenience method that combines user creation and token generation.
    ///
    /// # Example
    /// ```ignore
    /// let (user_id, user_token) = app.create_user().await;
    /// ```
    pub async fn create_user(&self) -> (i64, String) {
        let user_id = self.create_test_user("testuser", "userpass", "user").await;
        let token = self.get_auth_token(user_id, "user");
        (user_id, token)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_app_creation() {
        let app = TestApp::new().await;
        assert!(app.db.lock().await.is_autocommit());
    }

    #[tokio::test]
    async fn test_create_test_user() {
        let app = TestApp::new().await;
        let user_id = app.create_test_user("testuser", "pass123", "user").await;
        assert!(user_id > 0);

        // Verify user exists in database
        let db = app.db.lock().await;
        let username: String = db
            .query_row(
                "SELECT username FROM users WHERE id = ?1",
                [user_id],
                |row| row.get(0),
            )
            .expect("User not found");
        assert_eq!(username, "testuser");
    }

    #[tokio::test]
    async fn test_get_auth_token() {
        let app = TestApp::new().await;
        let token = app.get_auth_token(1, "admin");
        assert!(!token.is_empty());

        // Verify token is valid
        let claims = app
            .auth_service
            .verify_token(&token)
            .expect("Token should be valid");
        assert_eq!(claims.sub, 1);
        assert_eq!(claims.role, "admin");
    }

    #[tokio::test]
    async fn test_auth_header() {
        let app = TestApp::new().await;
        let token = app.get_auth_token(1, "user");
        let (name, value) = app.auth_header(&token);

        assert_eq!(name, axum::http::header::AUTHORIZATION);
        assert_eq!(
            value.to_str().unwrap(),
            format!("Bearer {}", token).as_str()
        );
    }

    #[tokio::test]
    async fn test_create_admin_helper() {
        let app = TestApp::new().await;
        let (admin_id, admin_token) = app.create_admin().await;

        assert!(admin_id > 0);
        assert!(!admin_token.is_empty());

        // Verify admin role
        let claims = app
            .auth_service
            .verify_token(&admin_token)
            .expect("Token should be valid");
        assert_eq!(claims.role, "admin");
    }

    #[tokio::test]
    async fn test_create_user_helper() {
        let app = TestApp::new().await;
        let (user_id, user_token) = app.create_user().await;

        assert!(user_id > 0);
        assert!(!user_token.is_empty());

        // Verify user role
        let claims = app
            .auth_service
            .verify_token(&user_token)
            .expect("Token should be valid");
        assert_eq!(claims.role, "user");
    }

    #[tokio::test]
    async fn test_health_check_endpoint() {
        let app = TestApp::new().await;
        let response = app.server().get("/health").await;

        response.assert_status_ok();
        response.assert_json_contains(&serde_json::json!({
            "message": "LCARS Backend is running"
        }));
    }
}
