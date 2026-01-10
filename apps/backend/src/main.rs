use axum::{response::Json, routing::get, Router};
use serde::Serialize;
use std::net::SocketAddr;

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

#[tokio::main]
async fn main() {
    let app = Router::new().route("/health", get(health_check));

    let addr = SocketAddr::from(([0, 0, 0, 0], 3001));
    println!("🚀 LCARS Backend listening on {}", addr);

    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}
