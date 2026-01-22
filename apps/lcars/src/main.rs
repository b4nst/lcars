use axum::{
    http::{header, Method},
    middleware as axum_mw,
    routing::{get, post, put},
    Router,
};
use rand::Rng;
use rusqlite::Connection;
use std::sync::Arc;
use tokio::sync::Mutex;
use tower_http::cors::CorsLayer;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

use lcars::{api, config, db, middleware, services, static_files, views, AppState};

use config::Config;
use services::{
    AuthService, IndexerManager, JobContext, MusicBrainzClient, Scheduler, SoulseekEngine,
    StorageManager, TmdbClient, TorrentEngine, WireGuardService,
};

fn init_tracing() {
    // Initialize tracing with env-filter
    // RUST_LOG environment variable controls log levels
    // Default: debug for our crate, info for axum, warn for dependencies
    let filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new("lcars=debug,tower_http=debug,axum=info,warn"));

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

    // Create WireGuard service (optional - requires configuration)
    let wireguard_service = match &config.wireguard {
        Some(wg_config) if wg_config.enabled => {
            match WireGuardService::new_shared(wg_config.clone()) {
                Ok(service) => {
                    tracing::info!(
                        interface = %service.interface_name(),
                        "WireGuard service initialized"
                    );
                    // Auto-connect on startup
                    match service.connect().await {
                        Ok(()) => {
                            tracing::info!("WireGuard connected successfully");
                        }
                        Err(e) => {
                            tracing::warn!("Failed to connect WireGuard: {}", e);
                            if wg_config.kill_switch {
                                tracing::warn!(
                                    "Kill switch is enabled - torrent downloads will be blocked until VPN connects"
                                );
                            }
                        }
                    }
                    Some(service)
                }
                Err(e) => {
                    tracing::error!("Failed to create WireGuard service: {}", e);
                    tracing::warn!("VPN features will be unavailable");
                    None
                }
            }
        }
        _ => {
            tracing::info!("WireGuard not configured - VPN features disabled");
            None
        }
    };

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

    // Enable VPN kill switch if both services are available and kill switch is configured
    if let (Some(ref torrent), Some(ref wg)) = (&torrent_engine, &wireguard_service) {
        if config.wireguard.as_ref().is_some_and(|c| c.kill_switch) {
            torrent.enable_vpn_kill_switch(Arc::clone(wg));
            tracing::info!("VPN kill switch enabled for torrent engine");
        }
    }

    // Create Soulseek engine (optional - requires credentials)
    let soulseek_engine =
        if config.soulseek.username.is_some() && config.soulseek.password.is_some() {
            match SoulseekEngine::new_shared(config.soulseek.clone()).await {
                Ok(engine) => {
                    tracing::info!(
                        server = %config.soulseek.server_host,
                        "Soulseek engine initialized"
                    );
                    // Auto-connect to Soulseek server
                    match engine.connect().await {
                        Ok(()) => {
                            tracing::info!("Soulseek connected successfully");
                        }
                        Err(e) => {
                            tracing::warn!("Failed to connect to Soulseek: {}", e);
                            tracing::info!("Auto-reconnect is enabled, will retry in background");
                        }
                    }
                    Some(engine)
                }
                Err(e) => {
                    tracing::error!("Failed to create Soulseek engine: {}", e);
                    tracing::warn!("Soulseek features will be unavailable");
                    None
                }
            }
        } else {
            tracing::info!("Soulseek credentials not configured - Soulseek features disabled");
            None
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

    // Create storage manager
    let storage_manager = match StorageManager::new(config.storage.clone()) {
        Ok(manager) => {
            tracing::info!(
                mounts = manager.list_mounts().len(),
                "Storage manager initialized"
            );
            Some(Arc::new(manager))
        }
        Err(e) => {
            tracing::error!("Failed to create storage manager: {}", e);
            tracing::warn!("File organization will be unavailable");
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
        soulseek_engine,
        scheduler,
        start_time: std::time::Instant::now(),
        storage_manager,
        wireguard_service,
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

    // Build system routes - some authenticated, some admin only
    let system_auth_routes = Router::new()
        .route("/status", get(api::system::get_system_status))
        .route("/activity", get(api::system::get_activity))
        .layer(axum_mw::from_fn_with_state(
            state.clone(),
            middleware::auth_middleware,
        ));

    let system_admin_routes = Router::new()
        .route("/jobs", get(api::system::list_jobs))
        .route("/jobs/{name}/run", post(api::system::trigger_job))
        .route(
            "/indexers",
            get(api::system::list_indexers).post(api::system::create_indexer),
        )
        .route(
            "/indexers/{id}",
            put(api::system::update_indexer).delete(api::system::delete_indexer),
        )
        .route("/indexers/{id}/test", post(api::system::test_indexer))
        .route("/storage/mounts", get(api::system::list_mounts))
        .route("/storage/mounts/{name}/test", post(api::system::test_mount))
        .layer(axum_mw::from_fn(middleware::require_admin))
        .layer(axum_mw::from_fn_with_state(
            state.clone(),
            middleware::auth_middleware,
        ));

    let system_routes = Router::new()
        .merge(system_auth_routes)
        .merge(system_admin_routes);

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

    // Build music routes (authenticated)
    let music_routes = api::music::router(state.clone());

    // Build downloads routes (authenticated)
    let downloads_routes = api::downloads::router(state.clone());

    // Build search routes (authenticated)
    let search_routes = Router::new()
        .route("/musicbrainz/artists", get(api::search::search_mb_artists))
        .route("/musicbrainz/albums", get(api::search::search_mb_albums))
        .route("/tmdb/movies", get(api::search::search_tmdb_movies))
        .route("/tmdb/tv", get(api::search::search_tmdb_tv))
        .layer(axum_mw::from_fn_with_state(
            state.clone(),
            middleware::auth_middleware,
        ));

    // Configure CORS based on allowed origins from config
    // If no origins configured, only same-origin requests are allowed
    let cors = if config.server.cors_origins.is_empty() {
        tracing::info!("CORS: No origins configured, same-origin only");
        CorsLayer::new()
            .allow_methods([
                Method::GET,
                Method::POST,
                Method::PUT,
                Method::DELETE,
                Method::OPTIONS,
            ])
            .allow_headers([header::AUTHORIZATION, header::CONTENT_TYPE, header::ACCEPT])
            .max_age(std::time::Duration::from_secs(3600))
    } else {
        use tower_http::cors::AllowOrigin;
        let origins: Vec<_> = config
            .server
            .cors_origins
            .iter()
            .filter_map(|o| o.parse().ok())
            .collect();
        tracing::info!("CORS: Allowing origins {:?}", config.server.cors_origins);
        CorsLayer::new()
            .allow_origin(AllowOrigin::list(origins))
            .allow_methods([
                Method::GET,
                Method::POST,
                Method::PUT,
                Method::DELETE,
                Method::OPTIONS,
            ])
            .allow_headers([header::AUTHORIZATION, header::CONTENT_TYPE, header::ACCEPT])
            .allow_credentials(true)
            .max_age(std::time::Duration::from_secs(3600))
    };

    // Build Soulseek routes (authenticated)
    let soulseek_routes = api::soulseek::router(state.clone());

    // Build VPN routes - status/stats for authenticated users, connect/disconnect for admin
    let vpn_auth_routes = Router::new()
        .route("/status", get(api::vpn::get_status))
        .route("/stats", get(api::vpn::get_stats))
        .layer(axum_mw::from_fn_with_state(
            state.clone(),
            middleware::auth_middleware,
        ));

    let vpn_admin_routes = Router::new()
        .route("/connect", post(api::vpn::connect))
        .route("/disconnect", post(api::vpn::disconnect))
        .layer(axum_mw::from_fn(middleware::require_admin))
        .layer(axum_mw::from_fn_with_state(
            state.clone(),
            middleware::auth_middleware,
        ));

    let vpn_routes = Router::new()
        .merge(vpn_auth_routes)
        .merge(vpn_admin_routes);

    // Build HTML views routes for HTMX frontend
    let html_routes = views::routes();

    // Build main router with state
    let app = Router::new()
        // Static assets (CSS, JS, fonts)
        .route("/static/*path", get(static_files::serve_static))
        // Health check
        .route("/health", get(lcars::health_check))
        // HTMX HTML routes (served at root)
        .merge(html_routes)
        // JSON API routes (under /api)
        .nest("/api/auth", auth_routes)
        .nest("/api/users", user_routes)
        .nest("/api/movies", movies_routes)
        .nest("/api/tv", tv_routes)
        .nest("/api/music", music_routes)
        .nest("/api/downloads", downloads_routes)
        .nest("/api/search", search_routes)
        .nest("/api/soulseek", soulseek_routes)
        .nest("/api/system", system_routes)
        .nest("/api/vpn", vpn_routes)
        .route("/api/ws", get(api::ws::ws_handler))
        // 404 fallback
        .fallback(views::not_found)
        .layer(cors)
        .with_state(state);

    let addr = config.server_addr();
    tracing::info!("LCARS Backend listening on {}", addr);

    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}
