//! VPN API endpoints for managing WireGuard VPN connections.
//!
//! Provides REST endpoints for:
//! - Checking VPN status
//! - Connecting/disconnecting the VPN
//! - Getting traffic statistics

use axum::{extract::State, Extension, Json};
use serde::Serialize;

use crate::error::{AppError, Result};
use crate::services::auth::Claims;
use crate::services::wireguard::ConnectionStatus;
use crate::AppState;

// =============================================================================
// Response Types
// =============================================================================

/// Response for GET /api/vpn/status
#[derive(Debug, Serialize)]
pub struct VpnStatusResponse {
    /// Whether WireGuard is configured in the application
    pub configured: bool,
    /// Whether WireGuard is enabled in configuration
    pub enabled: bool,
    /// Current connection status
    pub status: String,
    /// WireGuard interface name
    #[serde(skip_serializing_if = "Option::is_none")]
    pub interface: Option<String>,
    /// Connected peer endpoint
    #[serde(skip_serializing_if = "Option::is_none")]
    pub endpoint: Option<String>,
    /// ISO 8601 timestamp when connection was established
    #[serde(skip_serializing_if = "Option::is_none")]
    pub connected_since: Option<String>,
    /// ISO 8601 timestamp of last successful handshake
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_handshake: Option<String>,
    /// Whether kill switch is enabled in configuration
    pub kill_switch_enabled: bool,
    /// Whether kill switch is currently active (VPN disconnected)
    pub kill_switch_active: bool,
}

/// Response for POST /api/vpn/connect
#[derive(Debug, Serialize)]
pub struct VpnConnectResponse {
    /// Whether the connection attempt succeeded
    pub success: bool,
    /// Current status after connection attempt
    pub status: String,
    /// Optional message with details
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
}

/// Response for POST /api/vpn/disconnect
#[derive(Debug, Serialize)]
pub struct VpnDisconnectResponse {
    /// Whether the disconnect succeeded
    pub success: bool,
    /// Optional message with details
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
}

/// Response for GET /api/vpn/stats
#[derive(Debug, Serialize)]
pub struct VpnStatsResponse {
    /// Whether VPN is currently connected
    pub connected: bool,
    /// Total bytes received
    pub rx_bytes: u64,
    /// Total bytes transmitted
    pub tx_bytes: u64,
    /// ISO 8601 timestamp of latest handshake
    #[serde(skip_serializing_if = "Option::is_none")]
    pub latest_handshake: Option<String>,
    /// Peer endpoint address
    #[serde(skip_serializing_if = "Option::is_none")]
    pub peer_endpoint: Option<String>,
}

// =============================================================================
// Handlers
// =============================================================================

/// GET /api/vpn/status
///
/// Returns the current VPN connection status.
/// Requires authentication.
pub async fn get_status(
    State(state): State<AppState>,
    Extension(_claims): Extension<Claims>,
) -> Result<Json<VpnStatusResponse>> {
    // Check if WireGuard is configured
    let wg_config = state.config.wireguard.as_ref();
    let configured = wg_config.is_some();
    let enabled = wg_config.is_some_and(|c| c.enabled);
    let kill_switch_enabled = wg_config.is_some_and(|c| c.kill_switch);

    // Get current state from service if available
    let (status, interface, endpoint, connected_since, last_handshake, kill_switch_active) =
        if let Some(wg_service) = state.wireguard_service() {
            let wg_state = wg_service.get_status().await;

            let status_str = match &wg_state.status {
                ConnectionStatus::Disconnected => "disconnected",
                ConnectionStatus::Connecting => "connecting",
                ConnectionStatus::Connected => "connected",
                ConnectionStatus::Reconnecting { .. } => "reconnecting",
                ConnectionStatus::Error(_) => "error",
            };

            let is_disconnected = matches!(
                wg_state.status,
                ConnectionStatus::Disconnected | ConnectionStatus::Error(_)
            );
            let kill_switch_active = kill_switch_enabled && is_disconnected;

            (
                status_str.to_string(),
                Some(wg_service.interface_name().to_string()),
                wg_state.stats.endpoint.clone(),
                wg_state.connected_since.map(|dt| dt.to_rfc3339()),
                wg_state.stats.last_handshake.map(|dt| dt.to_rfc3339()),
                kill_switch_active,
            )
        } else {
            ("not_configured".to_string(), None, None, None, None, false)
        };

    Ok(Json(VpnStatusResponse {
        configured,
        enabled,
        status,
        interface,
        endpoint,
        connected_since,
        last_handshake,
        kill_switch_enabled,
        kill_switch_active,
    }))
}

/// POST /api/vpn/connect
///
/// Initiates a VPN connection.
/// Requires admin role.
pub async fn connect(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
) -> Result<Json<VpnConnectResponse>> {
    // Require admin role
    if claims.role != "admin" {
        return Err(AppError::Forbidden);
    }

    // Get WireGuard service
    let wg_service = state.wireguard_service().ok_or_else(|| {
        AppError::ServiceUnavailable("WireGuard VPN is not configured".to_string())
    })?;

    // Check if already connected
    let current_state = wg_service.get_status().await;
    if matches!(current_state.status, ConnectionStatus::Connected) {
        return Ok(Json(VpnConnectResponse {
            success: true,
            status: "connected".to_string(),
            message: Some("VPN is already connected".to_string()),
        }));
    }

    // Attempt connection
    match wg_service.connect().await {
        Ok(()) => Ok(Json(VpnConnectResponse {
            success: true,
            status: "connected".to_string(),
            message: Some("VPN connection established".to_string()),
        })),
        Err(e) => Ok(Json(VpnConnectResponse {
            success: false,
            status: "error".to_string(),
            message: Some(format!("Failed to connect: {}", e)),
        })),
    }
}

/// POST /api/vpn/disconnect
///
/// Disconnects the VPN.
/// Requires admin role.
pub async fn disconnect(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
) -> Result<Json<VpnDisconnectResponse>> {
    // Require admin role
    if claims.role != "admin" {
        return Err(AppError::Forbidden);
    }

    // Get WireGuard service
    let wg_service = state.wireguard_service().ok_or_else(|| {
        AppError::ServiceUnavailable("WireGuard VPN is not configured".to_string())
    })?;

    // Check if already disconnected
    let current_state = wg_service.get_status().await;
    if matches!(current_state.status, ConnectionStatus::Disconnected) {
        return Ok(Json(VpnDisconnectResponse {
            success: true,
            message: Some("VPN is already disconnected".to_string()),
        }));
    }

    // Attempt disconnect
    match wg_service.disconnect().await {
        Ok(()) => Ok(Json(VpnDisconnectResponse {
            success: true,
            message: Some("VPN disconnected".to_string()),
        })),
        Err(e) => Ok(Json(VpnDisconnectResponse {
            success: false,
            message: Some(format!("Failed to disconnect: {}", e)),
        })),
    }
}

/// GET /api/vpn/stats
///
/// Returns VPN traffic statistics.
/// Requires authentication.
pub async fn get_stats(
    State(state): State<AppState>,
    Extension(_claims): Extension<Claims>,
) -> Result<Json<VpnStatsResponse>> {
    // Get WireGuard service
    let wg_service = state.wireguard_service().ok_or_else(|| {
        AppError::ServiceUnavailable("WireGuard VPN is not configured".to_string())
    })?;

    let wg_state = wg_service.get_status().await;
    let connected = matches!(wg_state.status, ConnectionStatus::Connected);

    Ok(Json(VpnStatsResponse {
        connected,
        rx_bytes: wg_state.stats.rx_bytes,
        tx_bytes: wg_state.stats.tx_bytes,
        latest_handshake: wg_state.stats.last_handshake.map(|dt| dt.to_rfc3339()),
        peer_endpoint: wg_state.stats.endpoint,
    }))
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_vpn_status_response_serialization() {
        let response = VpnStatusResponse {
            configured: true,
            enabled: true,
            status: "connected".to_string(),
            interface: Some("wg0".to_string()),
            endpoint: Some("vpn.example.com:51820".to_string()),
            connected_since: Some("2024-01-15T10:30:00Z".to_string()),
            last_handshake: Some("2024-01-15T10:35:00Z".to_string()),
            kill_switch_enabled: true,
            kill_switch_active: false,
        };

        let json = serde_json::to_string(&response).unwrap();
        assert!(json.contains("\"configured\":true"));
        assert!(json.contains("\"status\":\"connected\""));
        assert!(json.contains("\"interface\":\"wg0\""));
    }

    #[test]
    fn test_vpn_status_response_skips_none() {
        let response = VpnStatusResponse {
            configured: false,
            enabled: false,
            status: "not_configured".to_string(),
            interface: None,
            endpoint: None,
            connected_since: None,
            last_handshake: None,
            kill_switch_enabled: false,
            kill_switch_active: false,
        };

        let json = serde_json::to_string(&response).unwrap();
        assert!(!json.contains("interface"));
        assert!(!json.contains("endpoint"));
        assert!(!json.contains("connected_since"));
    }

    #[test]
    fn test_vpn_connect_response_serialization() {
        let response = VpnConnectResponse {
            success: true,
            status: "connected".to_string(),
            message: Some("VPN connection established".to_string()),
        };

        let json = serde_json::to_string(&response).unwrap();
        assert!(json.contains("\"success\":true"));
        assert!(json.contains("\"status\":\"connected\""));
    }

    #[test]
    fn test_vpn_stats_response_serialization() {
        let response = VpnStatsResponse {
            connected: true,
            rx_bytes: 1024000,
            tx_bytes: 512000,
            latest_handshake: Some("2024-01-15T10:35:00Z".to_string()),
            peer_endpoint: Some("vpn.example.com:51820".to_string()),
        };

        let json = serde_json::to_string(&response).unwrap();
        assert!(json.contains("\"rx_bytes\":1024000"));
        assert!(json.contains("\"tx_bytes\":512000"));
    }
}
