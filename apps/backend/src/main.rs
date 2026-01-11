use axum::{
    middleware as axum_mw,
    response::Json,
    routing::{get, post, put},
    Router,
};
use rand::Rng;
use rusqlite::Connection;
use serde::Serialize;
use std::sync::Arc;
use tokio::sync::Mutex;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

mod api;
mod config;
mod db;
mod error;
mod middleware;
mod services;

use config::Config;
use services::{
    AuthService, IndexerManager, JobContext, MusicBrainzClient, Scheduler, TmdbClient,
    TorrentEngine,
};

/// Application state shared across handlers
#[derive(Clone)]
pub struct AppState {
    pub config: Arc<Config>,
    pub db: Arc<Mutex<Connection>>,
    auth_service: Arc<AuthService>,
    tmdb_client: Option<Arc<TmdbClient>>,
    musicbrainz_client: Option<Arc<MusicBrainzClient>>,
    indexer_manager: Arc<IndexerManager>,
    torrent_engine: Option<Arc<TorrentEngine>>,
    scheduler: Option<Arc<Scheduler>>,
}

impl AppState {
    /// Get a reference to the auth service.
    pub fn auth_service(&self) -> &AuthService {
        &self.auth_service
    }

    /// Get a reference to the TMDB client, if configured.
    pub fn tmdb_client(&self) -> Option<&TmdbClient> {
        self.tmdb_client.as_deref()
    }

    /// Get a reference to the MusicBrainz client, if configured.
    pub fn musicbrainz_client(&self) -> Option<&MusicBrainzClient> {
        self.musicbrainz_client.as_deref()
    }

    /// Get a reference to the indexer manager.
    pub fn indexer_manager(&self) -> &IndexerManager {
        &self.indexer_manager
    }

    /// Get a reference to the torrent engine, if initialized.
    pub fn torrent_engine(&self) -> Option<&TorrentEngine> {
        self.torrent_engine.as_deref()
    }

    /// Get a reference to the scheduler, if initialized.
    pub fn scheduler(&self) -> Option<&Scheduler> {
        self.scheduler.as_deref()
    }

    /// Create a job context for manual job execution.
    pub fn job_context(&self) -> JobContext {
        JobContext {
            db: Arc::clone(&self.db),
            tmdb_client: self.tmdb_client.clone(),
            musicbrainz_client: self.musicbrainz_client.clone(),
            indexer_manager: Arc::clone(&self.indexer_manager),
            torrent_engine: self.torrent_engine.clone(),
        }
    }
}

#[derive(Serialize)]
struct ApiResponse {
    message: String,
    version: String,
}

async fn health_check() -> Json<ApiResponse> {
    Json(ApiResponse {
        message: "LCARS Backend is running".to_string(),
        version: env!("CARGO_PKG_VERSION").to_string(),
    })
}

fn init_tracing() {
    // Initialize tracing with env-filter
    // RUST_LOG environment variable controls log levels
    // Default: debug for our crate, info for axum, warn for dependencies
    let filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new("backend=debug,tower_http=debug,axum=info,warn"));

    tracing_subscriber::registry()
        .with(filter)
        .with(tracing_subscriber::fmt::layer())
        .init();
}

/// Ensure a default admin user exists.
fn ensure_admin_user(conn: &Connection, auth_service: &AuthService) {
    let admin_exists: bool = conn
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM users WHERE role = 'admin')",
            [],
            |row| row.get(0),
        )
        .unwrap_or(false);

    if admin_exists {
        tracing::debug!("Admin user already exists");
        return;
    }

    // Generate a random password if not provided via environment
    let admin_password = std::env::var("LCARS_ADMIN_PASSWORD").unwrap_or_else(|_| {
        let password: String = rand::thread_rng()
            .sample_iter(&rand::distributions::Alphanumeric)
            .take(16)
            .map(char::from)
            .collect();
        tracing::warn!("Generated admin password: {}", password);
        tracing::warn!("Set LCARS_ADMIN_PASSWORD environment variable to use a fixed password");
        password
    });

    let password_hash = match auth_service.hash_password(&admin_password) {
        Ok(hash) => hash,
        Err(e) => {
            tracing::error!("Failed to hash admin password: {}", e);
            return;
        }
    };

    match conn.execute(
        "INSERT INTO users (username, password_hash, role) VALUES ('admin', ?1, 'admin')",
        [&password_hash],
    ) {
        Ok(_) => tracing::info!("Created default admin user"),
        Err(e) => tracing::error!("Failed to create admin user: {}", e),
    }
}

/// Clean up expired sessions.
fn cleanup_expired_sessions(conn: &Connection) {
    match conn.execute(
        "DELETE FROM sessions WHERE expires_at < datetime('now')",
        [],
    ) {
        Ok(count) => {
            if count > 0 {
                tracing::debug!("Cleaned up {} expired sessions", count);
            }
        }
        Err(e) => tracing::warn!("Failed to cleanup expired sessions: {}", e),
    }
}

#[tokio::main]
async fn main() {
    // Initialize tracing first so we can log configuration loading
    init_tracing();

    tracing::info!("Starting LCARS Backend v{}", env!("CARGO_PKG_VERSION"));

    // Load configuration
    let config = match Config::load() {
        Ok(cfg) => {
            tracing::info!("Configuration loaded successfully");
            tracing::debug!("Server: {}:{}", cfg.server.host, cfg.server.port);
            tracing::debug!("Database: {:?}", cfg.database.path);
            cfg
        }
        Err(e) => {
            tracing::error!("Failed to load configuration: {}", e);
            std::process::exit(1);
        }
    };

    // Ensure database directory exists
    if let Some(parent) = config.database.path.parent() {
        if !parent.exists() {
            if let Err(e) = std::fs::create_dir_all(parent) {
                tracing::error!("Failed to create database directory: {}", e);
                std::process::exit(1);
            }
        }
    }

    // Initialize database
    let conn = match db::init_db(&config.database.path) {
        Ok(conn) => {
            tracing::info!("Database initialized at {:?}", config.database.path);
            conn
        }
        Err(e) => {
            tracing::error!("Failed to initialize database: {}", e);
            std::process::exit(1);
        }
    };

    // Get JWT secret, generating one if not configured (development mode)
    let jwt_secret = config.server.jwt_secret.clone().unwrap_or_else(|| {
        let secret: String = rand::thread_rng()
            .sample_iter(&rand::distributions::Alphanumeric)
            .take(32)
            .map(char::from)
            .collect();
        tracing::warn!("No JWT secret configured, using random secret");
        tracing::warn!("Set LCARS_SERVER__JWT_SECRET for production use");
        secret
    });

    // Create auth service
    let auth_service = AuthService::new(jwt_secret);

    // Ensure admin user exists
    ensure_admin_user(&conn, &auth_service);

    // Clean up expired sessions on startup
    cleanup_expired_sessions(&conn);

    // Create TMDB client if API key is configured
    let tmdb_client = match &config.tmdb.api_key {
        Some(api_key) if !api_key.is_empty() => match TmdbClient::new_shared(api_key.clone()) {
            Ok(client) => {
                tracing::info!("TMDB client initialized");
                Some(client)
            }
            Err(e) => {
                tracing::error!("Failed to create TMDB client: {}", e);
                None
            }
        },
        _ => {
            tracing::warn!("TMDB API key not configured - metadata lookups will be unavailable");
            None
        }
    };

    // Create MusicBrainz client (no API key required, just rate limiting)
    let musicbrainz_client = match MusicBrainzClient::new_shared(
        env!("CARGO_PKG_NAME"),
        env!("CARGO_PKG_VERSION"),
        "https://github.com/b4nst/lcars",
        config.musicbrainz.rate_limit_ms,
    ) {
        Ok(client) => {
            tracing::info!("MusicBrainz client initialized");
            Some(client)
        }
        Err(e) => {
            tracing::error!("Failed to create MusicBrainz client: {}", e);
            None
        }
    };

    // Create indexer manager
    let indexer_manager = IndexerManager::new_shared();
    tracing::info!(
        "Indexer manager initialized with {} providers",
        indexer_manager.providers().len()
    );

    // Create torrent engine
    let torrent_engine = match TorrentEngine::new_shared(config.torrent.clone()).await {
        Ok(engine) => {
            tracing::info!(
                download_dir = ?config.torrent.download_dir,
                "Torrent engine initialized"
            );
            Some(engine)
        }
        Err(e) => {
            tracing::error!("Failed to create torrent engine: {}", e);
            tracing::warn!("Downloads will be unavailable until torrent engine is fixed");
            None
        }
    };

    // Create job context for scheduler
    let job_ctx = JobContext {
        db: Arc::new(Mutex::new(conn)),
        tmdb_client: tmdb_client.clone(),
        musicbrainz_client: musicbrainz_client.clone(),
        indexer_manager: indexer_manager.clone(),
        torrent_engine: torrent_engine.clone(),
    };

    // Create and start scheduler
    let scheduler = match Scheduler::new_shared(&config.scheduler, job_ctx.clone()).await {
        Ok(sched) => {
            if let Err(e) = sched.start().await {
                tracing::error!("Failed to start scheduler: {}", e);
                None
            } else {
                tracing::info!("Scheduler started with configured jobs");
                Some(sched)
            }
        }
        Err(e) => {
            tracing::error!("Failed to create scheduler: {}", e);
            tracing::warn!("Background jobs will not run automatically");
            None
        }
    };

    // Create application state
    let state = AppState {
        config: Arc::new(config.clone()),
        db: job_ctx.db,
        auth_service: Arc::new(auth_service),
        tmdb_client,
        musicbrainz_client,
        indexer_manager,
        torrent_engine,
        scheduler,
    };

    // Build auth routes (public)
    let auth_routes = Router::new()
        .route("/login", post(api::auth::login))
        .route(
            "/logout",
            post(api::auth::logout).layer(axum_mw::from_fn_with_state(
                state.clone(),
                middleware::auth_middleware,
            )),
        )
        .route(
            "/me",
            get(api::auth::me).layer(axum_mw::from_fn_with_state(
                state.clone(),
                middleware::auth_middleware,
            )),
        );

    // Build user routes (admin only)
    let user_routes = Router::new()
        .route(
            "/",
            get(api::users::list_users).post(api::users::create_user),
        )
        .route(
            "/{id}",
            put(api::users::update_user).delete(api::users::delete_user),
        )
        .layer(axum_mw::from_fn(middleware::require_admin))
        .layer(axum_mw::from_fn_with_state(
            state.clone(),
            middleware::auth_middleware,
        ));

    // Build system routes (admin only)
    let system_routes = Router::new()
        .route("/jobs", get(api::system::list_jobs))
        .route("/jobs/{name}/run", post(api::system::trigger_job))
        .layer(axum_mw::from_fn(middleware::require_admin))
        .layer(axum_mw::from_fn_with_state(
            state.clone(),
            middleware::auth_middleware,
        ));

    // Build movies routes (authenticated)
    let movies_routes = Router::new()
        .route(
            "/",
            get(api::movies::list_movies).post(api::movies::add_movie),
        )
        .route(
            "/{id}",
            get(api::movies::get_movie)
                .put(api::movies::update_movie)
                .delete(api::movies::delete_movie),
        )
        .route("/{id}/search", post(api::movies::search_releases))
        .route("/{id}/download", post(api::movies::download_release))
        .route("/{id}/refresh", post(api::movies::refresh_metadata))
        .layer(axum_mw::from_fn_with_state(
            state.clone(),
            middleware::auth_middleware,
        ));

    // Build TV shows routes (authenticated)
    let tv_routes = api::tv::router(state.clone());

    // Build main router with state
    let app = Router::new()
        .route("/health", get(health_check))
        .nest("/api/auth", auth_routes)
        .nest("/api/users", user_routes)
        .nest("/api/movies", movies_routes)
        .nest("/api/tv", tv_routes)
        .nest("/api/system", system_routes)
        .with_state(state);

    let addr = config.server_addr();
    tracing::info!("LCARS Backend listening on {}", addr);

    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}
