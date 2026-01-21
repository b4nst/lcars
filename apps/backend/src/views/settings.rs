//! Settings views

use askama::Template;
use axum::{
    extract::State,
    response::{IntoResponse, Redirect},
};
use axum_extra::extract::CookieJar;

use crate::AppState;

use super::auth;

#[derive(Template)]
#[template(path = "pages/settings.html")]
pub struct SettingsTemplate {
    pub version: String,
    pub uptime: String,
    pub db_size: String,
    pub vpn_status: VpnStatus,
    pub torrent_status: ServiceStatus,
    pub soulseek_status: ServiceStatus,
    pub storage_mounts: Vec<StorageMount>,
    pub indexers: Vec<IndexerInfo>,
}

pub struct VpnStatus {
    pub connected: bool,
    pub ip: Option<String>,
    pub country: Option<String>,
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

    // Check VPN status
    let vpn_status = check_vpn_status().await;

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
                    mount
                        .free_space()
                        .await
                        .ok()
                        .map(|bytes| format_size(bytes as i64))
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
    format_size(size)
}

async fn check_vpn_status() -> VpnStatus {
    // Try to get external IP and check if it's different from local
    // This is a simple check; production would use VPN provider API
    VpnStatus {
        connected: false,
        ip: None,
        country: None,
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

fn format_size(bytes: i64) -> String {
    const KB: i64 = 1024;
    const MB: i64 = KB * 1024;
    const GB: i64 = MB * 1024;

    if bytes >= GB {
        format!("{:.2} GB", bytes as f64 / GB as f64)
    } else if bytes >= MB {
        format!("{:.2} MB", bytes as f64 / MB as f64)
    } else if bytes >= KB {
        format!("{:.2} KB", bytes as f64 / KB as f64)
    } else {
        format!("{} B", bytes)
    }
}
