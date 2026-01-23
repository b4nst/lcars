//! Settings views

use askama::Template;
use axum::{
    extract::State,
    response::{Html, IntoResponse, Redirect},
};
use axum_extra::extract::CookieJar;

use crate::services::wireguard::ConnectionStatus;
use crate::AppState;

use super::auth;
use super::utils::format_size;

#[derive(Template)]
#[template(path = "pages/settings.html")]
pub struct SettingsTemplate {
    pub version: String,
    pub uptime: String,
    pub db_size: String,
    pub vpn_status: VpnStatusView,
    pub torrent_status: ServiceStatus,
    pub soulseek_status: ServiceStatus,
    pub storage_mounts: Vec<StorageMount>,
    pub indexers: Vec<IndexerInfo>,
}

/// VPN status view model for templates
pub struct VpnStatusView {
    /// Whether WireGuard is configured
    pub configured: bool,
    /// Whether WireGuard is enabled
    pub enabled: bool,
    /// Current connection status string
    pub status: String,
    /// CSS class for status indicator (connected, connecting, disconnected, error)
    pub status_class: String,
    /// Interface name
    pub interface: Option<String>,
    /// Connected peer endpoint
    pub endpoint: Option<String>,
    /// Human-readable connection duration
    pub connected_since: Option<String>,
    /// Whether kill switch is enabled
    pub kill_switch_enabled: bool,
    /// Whether kill switch is currently active
    pub kill_switch_active: bool,
}

pub struct ServiceStatus {
    pub available: bool,
    pub status: String,
}

pub struct StorageMount {
    pub name: String,
    pub path: String,
    pub available: bool,
    pub free_space: Option<String>,
}

pub struct IndexerInfo {
    pub id: i64,
    pub name: String,
    pub enabled: bool,
    pub priority: i32,
}

/// Settings page
pub async fn page(State(state): State<AppState>, cookies: CookieJar) -> impl IntoResponse {
    if auth::get_current_user(&state, &cookies).await.is_none() {
        return Redirect::to("/login").into_response();
    }

    let uptime = state.start_time().elapsed();
    let uptime_str = format_duration(uptime.as_secs());

    // Get database size
    let db = state.db.lock().await;
    let db_size = get_db_size(&db);

    // Get indexers
    let indexers = {
        let mut stmt = db
            .prepare("SELECT id, name, enabled, priority FROM indexers ORDER BY priority DESC")
            .unwrap_or_else(|_| panic!("Failed to prepare query"));

        stmt.query_map([], |row| {
            Ok(IndexerInfo {
                id: row.get(0)?,
                name: row.get(1)?,
                enabled: row.get(2)?,
                priority: row.get(3)?,
            })
        })
        .unwrap_or_else(|_| panic!("Failed to query indexers"))
        .collect::<Result<Vec<_>, _>>()
        .unwrap_or_default()
    };
    drop(db);

    // Check VPN status from WireGuard service
    let vpn_status = get_vpn_status(&state).await;

    // Torrent engine status
    let torrent_status = if let Some(engine) = state.torrent_engine() {
        let torrents = engine.list_all().await;
        let active_count = torrents
            .iter()
            .filter(|t| {
                matches!(
                    t.status,
                    crate::db::models::DownloadStatus::Downloading
                        | crate::db::models::DownloadStatus::Queued
                )
            })
            .count();
        ServiceStatus {
            available: true,
            status: format!("{} active downloads", active_count),
        }
    } else {
        ServiceStatus {
            available: false,
            status: "Not configured".to_string(),
        }
    };

    // Soulseek status
    let soulseek_status = if let Some(engine) = state.soulseek_engine() {
        ServiceStatus {
            available: true,
            status: if engine.is_connected().await {
                "Connected".to_string()
            } else {
                "Disconnected".to_string()
            },
        }
    } else {
        ServiceStatus {
            available: false,
            status: "Not configured".to_string(),
        }
    };

    // Storage mounts
    let storage_mounts = if let Some(storage) = state.storage_manager() {
        let mut mounts = Vec::new();
        for mount_name in storage.list_mounts() {
            if let Some(mount) = storage.get_mount(mount_name) {
                let root = mount.root();
                let path = root.to_string_lossy().to_string();
                let available = mount.available().await;
                let free_space = if available {
                    mount.free_space().await.ok().map(format_size)
                } else {
                    None
                };

                mounts.push(StorageMount {
                    name: mount_name.to_string(),
                    path,
                    available,
                    free_space,
                });
            }
        }
        mounts
    } else {
        vec![]
    };

    SettingsTemplate {
        version: env!("CARGO_PKG_VERSION").to_string(),
        uptime: uptime_str,
        db_size,
        vpn_status,
        torrent_status,
        soulseek_status,
        storage_mounts,
        indexers,
    }
    .into_response()
}

fn get_db_size(conn: &rusqlite::Connection) -> String {
    let size: i64 = conn
        .query_row(
            "SELECT page_count * page_size FROM pragma_page_count(), pragma_page_size()",
            [],
            |row| row.get(0),
        )
        .unwrap_or(0);
    format_size(size as u64)
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

/// VPN status partial template for HTMX updates
#[derive(Template)]
#[template(path = "partials/vpn_status.html")]
pub struct VpnStatusPartial {
    pub vpn_status: VpnStatusView,
    pub is_admin: bool,
}

/// GET /vpn/status - Returns VPN status HTML partial for HTMX polling
pub async fn vpn_status_partial(
    State(state): State<AppState>,
    cookies: CookieJar,
) -> impl IntoResponse {
    let user = auth::get_current_user(&state, &cookies).await;
    if user.is_none() {
        return Html("<div class='lcars-error'>Unauthorized</div>").into_response();
    }

    let is_admin = user.is_some_and(|u| u.role == "admin");
    let vpn_status = get_vpn_status(&state).await;

    VpnStatusPartial {
        vpn_status,
        is_admin,
    }
    .into_response()
}

/// POST /vpn/connect - Connect VPN and return status partial
pub async fn vpn_connect(State(state): State<AppState>, cookies: CookieJar) -> impl IntoResponse {
    let user = auth::get_current_user(&state, &cookies).await;
    if user.is_none() {
        return Html("<div class='lcars-error'>Unauthorized</div>").into_response();
    }

    // Check admin role
    if user.is_none_or(|u| u.role != "admin") {
        return Html("<div class='lcars-error'>Admin access required</div>").into_response();
    }

    // Connect VPN
    if let Some(wg_service) = state.wireguard_service() {
        if let Err(e) = wg_service.connect().await {
            return Html(format!(
                "<div class='lcars-error'>Failed to connect: {}</div>",
                e
            ))
            .into_response();
        }
    } else {
        return Html("<div class='lcars-error'>VPN not configured</div>").into_response();
    }

    // Return updated status
    let vpn_status = get_vpn_status(&state).await;
    VpnStatusPartial {
        vpn_status,
        is_admin: true,
    }
    .into_response()
}

/// POST /vpn/disconnect - Disconnect VPN and return status partial
pub async fn vpn_disconnect(
    State(state): State<AppState>,
    cookies: CookieJar,
) -> impl IntoResponse {
    let user = auth::get_current_user(&state, &cookies).await;
    if user.is_none() {
        return Html("<div class='lcars-error'>Unauthorized</div>").into_response();
    }

    // Check admin role
    if user.is_none_or(|u| u.role != "admin") {
        return Html("<div class='lcars-error'>Admin access required</div>").into_response();
    }

    // Disconnect VPN
    if let Some(wg_service) = state.wireguard_service() {
        if let Err(e) = wg_service.disconnect().await {
            return Html(format!(
                "<div class='lcars-error'>Failed to disconnect: {}</div>",
                e
            ))
            .into_response();
        }
    } else {
        return Html("<div class='lcars-error'>VPN not configured</div>").into_response();
    }

    // Return updated status
    let vpn_status = get_vpn_status(&state).await;
    VpnStatusPartial {
        vpn_status,
        is_admin: true,
    }
    .into_response()
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
