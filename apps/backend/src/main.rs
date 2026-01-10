use axum::{response::Json, routing::get, Router};
use rusqlite::Connection;
use serde::Serialize;
use std::sync::Arc;
use tokio::sync::Mutex;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

mod config;
mod db;
mod error;

use config::Config;

/// Application state shared across handlers
#[derive(Clone)]
pub struct AppState {
    pub config: Arc<Config>,
    pub db: Arc<Mutex<Connection>>,
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

    // Create application state
    let state = AppState {
        config: Arc::new(config.clone()),
        db: Arc::new(Mutex::new(conn)),
    };

    // Build router with state
    let app = Router::new()
        .route("/health", get(health_check))
        .with_state(state);

    let addr = config.server_addr();
    tracing::info!("LCARS Backend listening on {}", addr);

    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}
