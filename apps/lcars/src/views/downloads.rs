//! Downloads views - thin wrappers around API handlers
//!
//! The page handler calls the API to get downloads, then renders templates.
//! Mutation handlers (pause/resume/cancel) call API handlers and return
//! HTML fragments for HTMX to swap.

use askama::Template;
use axum::{
    extract::{Path, State},
    response::{Html, IntoResponse, Redirect},
};
use axum_extra::extract::CookieJar;

use crate::api::downloads::{
    delete_download as api_delete, list_downloads as api_list, pause_download as api_pause,
    resume_download as api_resume, DeleteDownloadQuery, ListDownloadsQuery,
};
use crate::db::models::DownloadStatus;
use crate::services::wireguard::ConnectionStatus;
use crate::AppState;

use super::{
    auth,
    utils::{format_size, format_speed},
};

// =============================================================================
// Templates
// =============================================================================

#[derive(Template)]
#[template(path = "pages/downloads.html")]
pub struct DownloadsTemplate {
    pub active: Vec<DownloadView>,
    pub queued: Vec<DownloadView>,
    pub seeding: Vec<DownloadView>,
    pub paused: Vec<DownloadView>,
    pub completed: Vec<DownloadView>,
    pub failed: Vec<DownloadView>,
    pub vpn_warning: Option<VpnWarning>,
}

/// VPN warning to show on downloads page
pub struct VpnWarning {
    pub message: String,
    pub show_connect_button: bool,
}

#[derive(Template)]
#[template(path = "components/download_item.html")]
pub struct DownloadItemTemplate {
    pub download: DownloadView,
}

// =============================================================================
// View Models
// =============================================================================

#[derive(Clone)]
pub struct DownloadView {
    pub id: i64,
    pub name: String,
    pub media_type: String,
    pub media_id: i64,
    pub status: String,
    pub progress: f64,
    pub progress_percent: String,
    pub download_speed: String,
    pub upload_speed: String,
    pub size_display: String,
    pub downloaded_display: String,
    pub peers: i32,
    pub error_message: Option<String>,
}

// =============================================================================
// View Handlers
// =============================================================================

/// Downloads page - calls API handler
pub async fn page(State(state): State<AppState>, cookies: CookieJar) -> impl IntoResponse {
    if auth::get_current_user(&state, &cookies).await.is_none() {
        return Redirect::to("/login").into_response();
    }

    // Check VPN status for warning banner
    let vpn_warning = get_vpn_warning(&state).await;

    // Call API handler
    let response = api_list(
        State(state),
        axum::extract::Query(ListDownloadsQuery {
            status: None,
            source: None,
        }),
    )
    .await;

    match response {
        Ok(json) => {
            let downloads = json.0;

            let mut active = vec![];
            let mut queued = vec![];
            let mut seeding = vec![];
            let mut paused = vec![];
            let mut completed = vec![];
            let mut failed = vec![];

            for d in downloads {
                let view = DownloadView {
                    id: d.id,
                    name: d.name.clone(),
                    media_type: d.media_type.to_string(),
                    media_id: d.media_id,
                    status: d.status.to_string(),
                    progress: d.progress,
                    progress_percent: format!("{:.1}%", d.progress * 100.0),
                    download_speed: format_speed(d.download_speed as u64),
                    upload_speed: format_speed(d.upload_speed as u64),
                    size_display: format_size(d.size_bytes.unwrap_or(0) as u64),
                    downloaded_display: format_size(d.downloaded_bytes as u64),
                    peers: d.peers,
                    error_message: d.error_message.clone(),
                };

                match d.status {
                    DownloadStatus::Downloading => active.push(view),
                    DownloadStatus::Queued => queued.push(view),
                    DownloadStatus::Seeding => seeding.push(view),
                    DownloadStatus::Paused => paused.push(view),
                    DownloadStatus::Completed => completed.push(view),
                    DownloadStatus::Failed => failed.push(view),
                    DownloadStatus::Processing => completed.push(view),
                }
            }

            DownloadsTemplate {
                active,
                queued,
                seeding,
                paused,
                completed,
                failed,
                vpn_warning,
            }
            .into_response()
        }
        Err(_) => Html("<div class='lcars-error'>Failed to load downloads</div>").into_response(),
    }
}

/// Check if VPN warning should be shown
async fn get_vpn_warning(state: &AppState) -> Option<VpnWarning> {
    let wg_config = state.config.wireguard.as_ref()?;

    let kill_switch_enabled = wg_config.kill_switch;

    if let Some(wg_service) = state.wireguard_service() {
        let wg_state = wg_service.get_status().await;

        let is_disconnected = matches!(
            wg_state.status,
            ConnectionStatus::Disconnected | ConnectionStatus::Error(_)
        );

        if is_disconnected && kill_switch_enabled {
            // Kill switch active - show critical warning
            Some(VpnWarning {
                message: "VPN is disconnected. Kill switch is active - all downloads are paused."
                    .to_string(),
                show_connect_button: true,
            })
        } else if is_disconnected {
            // VPN disconnected but no kill switch - show warning
            Some(VpnWarning {
                message: "VPN is disconnected. Your IP may be exposed during downloads."
                    .to_string(),
                show_connect_button: true,
            })
        } else {
            None
        }
    } else {
        None
    }
}

/// Pause a download - calls API handler
pub async fn pause(
    State(state): State<AppState>,
    cookies: CookieJar,
    Path(id): Path<i64>,
) -> impl IntoResponse {
    if auth::get_current_user(&state, &cookies).await.is_none() {
        return Html("<div class='lcars-error'>Unauthorized</div>").into_response();
    }

    // Call API handler
    let response = api_pause(State(state.clone()), Path(id)).await;

    match response {
        Ok(_) => {
            // Fetch updated download to return HTML fragment
            let list_response = api_list(
                State(state),
                axum::extract::Query(ListDownloadsQuery {
                    status: None,
                    source: None,
                }),
            )
            .await;

            if let Ok(json) = list_response {
                if let Some(d) = json.0.into_iter().find(|d| d.id == id) {
                    return DownloadItemTemplate {
                        download: DownloadView {
                            id: d.id,
                            name: d.name,
                            media_type: d.media_type.to_string(),
                            media_id: d.media_id,
                            status: d.status.to_string(),
                            progress: d.progress,
                            progress_percent: format!("{:.1}%", d.progress * 100.0),
                            download_speed: format_speed(d.download_speed as u64),
                            upload_speed: format_speed(d.upload_speed as u64),
                            size_display: format_size(d.size_bytes.unwrap_or(0) as u64),
                            downloaded_display: format_size(d.downloaded_bytes as u64),
                            peers: d.peers,
                            error_message: d.error_message,
                        },
                    }
                    .into_response();
                }
            }
            Html("<div class='lcars-success'>Paused</div>").into_response()
        }
        Err(e) => Html(format!(
            "<div class='lcars-error'>Failed to pause: {}</div>",
            e
        ))
        .into_response(),
    }
}

/// Resume a download - calls API handler
pub async fn resume(
    State(state): State<AppState>,
    cookies: CookieJar,
    Path(id): Path<i64>,
) -> impl IntoResponse {
    if auth::get_current_user(&state, &cookies).await.is_none() {
        return Html("<div class='lcars-error'>Unauthorized</div>").into_response();
    }

    // Call API handler
    let response = api_resume(State(state.clone()), Path(id)).await;

    match response {
        Ok(_) => {
            // Fetch updated download to return HTML fragment
            let list_response = api_list(
                State(state),
                axum::extract::Query(ListDownloadsQuery {
                    status: None,
                    source: None,
                }),
            )
            .await;

            if let Ok(json) = list_response {
                if let Some(d) = json.0.into_iter().find(|d| d.id == id) {
                    return DownloadItemTemplate {
                        download: DownloadView {
                            id: d.id,
                            name: d.name,
                            media_type: d.media_type.to_string(),
                            media_id: d.media_id,
                            status: d.status.to_string(),
                            progress: d.progress,
                            progress_percent: format!("{:.1}%", d.progress * 100.0),
                            download_speed: format_speed(d.download_speed as u64),
                            upload_speed: format_speed(d.upload_speed as u64),
                            size_display: format_size(d.size_bytes.unwrap_or(0) as u64),
                            downloaded_display: format_size(d.downloaded_bytes as u64),
                            peers: d.peers,
                            error_message: d.error_message,
                        },
                    }
                    .into_response();
                }
            }
            Html("<div class='lcars-success'>Resumed</div>").into_response()
        }
        Err(e) => Html(format!(
            "<div class='lcars-error'>Failed to resume: {}</div>",
            e
        ))
        .into_response(),
    }
}

/// Cancel/delete a download - calls API handler
pub async fn cancel(
    State(state): State<AppState>,
    cookies: CookieJar,
    Path(id): Path<i64>,
) -> impl IntoResponse {
    if auth::get_current_user(&state, &cookies).await.is_none() {
        return Html("<div class='lcars-error'>Unauthorized</div>").into_response();
    }

    // Call API handler
    let response = api_delete(
        State(state),
        Path(id),
        axum::extract::Query(DeleteDownloadQuery {
            delete_files: Some(false),
        }),
    )
    .await;

    match response {
        Ok(_) => {
            // Return empty to remove the element via HTMX swap
            Html("").into_response()
        }
        Err(e) => Html(format!(
            "<div class='lcars-error'>Failed to cancel: {}</div>",
            e
        ))
        .into_response(),
    }
}
