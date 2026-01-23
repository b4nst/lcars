//! Dashboard view

use askama::Template;
use axum::{
    extract::State,
    response::{IntoResponse, Redirect},
};
use axum_extra::extract::CookieJar;

use crate::api::movies::{list_movies, ListMoviesQuery};
use crate::api::system::get_system_status;
use crate::services::wireguard::ConnectionStatus;
use crate::AppState;

use super::auth;
use super::settings::VpnStatusView;

#[derive(Template)]
#[template(path = "pages/dashboard.html")]
pub struct DashboardTemplate {
    pub version: String,
    pub uptime_seconds: u64,
    pub active_downloads: i64,
    pub total_movies: i64,
    pub total_shows: i64,
    pub total_artists: i64,
    pub recent_movies: Vec<MovieSummary>,
    pub vpn_status: VpnStatusView,
    pub is_admin: bool,
}

pub struct MovieSummary {
    pub id: i64,
    pub title: String,
    pub year: Option<i32>,
    pub poster_path: Option<String>,
    pub status: String,
}

/// Render the dashboard page
pub async fn page(State(state): State<AppState>, cookies: CookieJar) -> impl IntoResponse {
    // Check authentication
    let user = auth::get_current_user(&state, &cookies).await;
    if user.is_none() {
        return Redirect::to("/login").into_response();
    }
    let is_admin = user.is_some_and(|u| u.role == "admin");

    // Get system status from API
    let status = get_system_status(State(state.clone()))
        .await
        .map(|r| r.0)
        .ok();

    // Get recent movies from API
    let recent_movies_response = list_movies(
        State(state.clone()),
        axum::extract::Query(ListMoviesQuery {
            status: None,
            monitored: None,
            search: None,
            page: Some(1),
            limit: Some(6),
        }),
    )
    .await
    .ok();

    // Get counts from database (these aren't exposed via API, keeping direct query for now)
    let (total_movies, total_shows, total_artists) = {
        let db = state.db.lock().await;
        let total_movies: i64 = db
            .query_row("SELECT COUNT(*) FROM movies", [], |row| row.get(0))
            .unwrap_or(0);
        let total_shows: i64 = db
            .query_row("SELECT COUNT(*) FROM tv_shows", [], |row| row.get(0))
            .unwrap_or(0);
        let total_artists: i64 = db
            .query_row("SELECT COUNT(*) FROM artists", [], |row| row.get(0))
            .unwrap_or(0);
        (total_movies, total_shows, total_artists)
    };

    let recent_movies: Vec<MovieSummary> = recent_movies_response
        .map(|r| {
            r.0.items
                .into_iter()
                .map(|m| MovieSummary {
                    id: m.id,
                    title: m.title,
                    year: Some(m.year),
                    poster_path: m.poster_path,
                    status: m.status.to_string(),
                })
                .collect()
        })
        .unwrap_or_default();

    // Get VPN status
    let vpn_status = get_vpn_status(&state).await;

    DashboardTemplate {
        version: status
            .as_ref()
            .map(|s| s.version.clone())
            .unwrap_or_else(|| env!("CARGO_PKG_VERSION").to_string()),
        uptime_seconds: status.as_ref().map(|s| s.uptime_seconds).unwrap_or(0),
        active_downloads: status.as_ref().map(|s| s.downloads.active).unwrap_or(0),
        total_movies,
        total_shows,
        total_artists,
        recent_movies,
        vpn_status,
        is_admin,
    }
    .into_response()
}

/// Get VPN status from WireGuard service
async fn get_vpn_status(state: &AppState) -> VpnStatusView {
    let wg_config = state.config.wireguard.as_ref();
    let configured = wg_config.is_some();
    let enabled = wg_config.is_some_and(|c| c.enabled);
    let kill_switch_enabled = wg_config.is_some_and(|c| c.kill_switch);

    if let Some(wg_service) = state.wireguard_service() {
        let wg_state = wg_service.get_status().await;

        let (status, status_class) = match &wg_state.status {
            ConnectionStatus::Disconnected => ("Disconnected".to_string(), "disconnected"),
            ConnectionStatus::Connecting => ("Connecting...".to_string(), "connecting"),
            ConnectionStatus::Connected => ("Connected".to_string(), "connected"),
            ConnectionStatus::Reconnecting { attempt } => {
                (format!("Reconnecting (attempt {})", attempt), "connecting")
            }
            ConnectionStatus::Error(msg) => (format!("Error: {}", msg), "error"),
        };

        let is_disconnected = matches!(
            wg_state.status,
            ConnectionStatus::Disconnected | ConnectionStatus::Error(_)
        );
        let kill_switch_active = kill_switch_enabled && is_disconnected;

        let connected_since = wg_state.connected_since.map(|dt| {
            let now = chrono::Utc::now();
            let duration = now - dt;
            format_duration(duration.num_seconds().max(0) as u64)
        });

        VpnStatusView {
            configured,
            enabled,
            status,
            status_class: status_class.to_string(),
            interface: Some(wg_service.interface_name().to_string()),
            endpoint: wg_state.stats.endpoint.clone(),
            connected_since,
            kill_switch_enabled,
            kill_switch_active,
        }
    } else {
        VpnStatusView {
            configured,
            enabled,
            status: if configured {
                "Not initialized".to_string()
            } else {
                "Not configured".to_string()
            },
            status_class: "disconnected".to_string(),
            interface: None,
            endpoint: None,
            connected_since: None,
            kill_switch_enabled,
            kill_switch_active: false,
        }
    }
}

fn format_duration(seconds: u64) -> String {
    let days = seconds / 86400;
    let hours = (seconds % 86400) / 3600;
    let minutes = (seconds % 3600) / 60;

    if days > 0 {
        format!("{}d {}h {}m", days, hours, minutes)
    } else if hours > 0 {
        format!("{}h {}m", hours, minutes)
    } else {
        format!("{}m", minutes)
    }
}
