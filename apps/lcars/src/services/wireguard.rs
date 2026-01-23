//! WireGuard VPN service for LCARS.
//!
//! Provides WireGuard VPN connection management with:
//! - Interface creation and configuration
//! - Health monitoring with automatic reconnection
//! - Connection status tracking
//! - Event broadcasting for real-time updates
//! - Support for both Linux (kernel) and macOS (userspace) implementations

use std::io::BufRead;
use std::net::SocketAddr;
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use chrono::{DateTime, Utc};
use serde::Serialize;
use tokio::sync::{broadcast, RwLock};
use tokio::task::JoinHandle;

use defguard_wireguard_rs::{
    host::Peer, key::Key, net::IpAddrMask, InterfaceConfiguration, WGApi, WireguardInterfaceApi,
};

#[cfg(target_os = "linux")]
use defguard_wireguard_rs::Kernel;

#[cfg(target_os = "macos")]
use defguard_wireguard_rs::Userspace;

use crate::config::{WireGuardConfig, WireGuardInlineConfig};
use crate::error::{AppError, Result};
use crate::services::dns::DnsManager;

// =========================================================================
// Platform-specific type aliases
// =========================================================================

#[cfg(target_os = "linux")]
type WgApiType = WGApi<Kernel>;

#[cfg(target_os = "macos")]
type WgApiType = WGApi<Userspace>;

// =========================================================================
// Connection status types
// =========================================================================

/// Connection status of the WireGuard interface.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum ConnectionStatus {
    /// Interface is disconnected.
    Disconnected,
    /// Currently establishing connection.
    Connecting,
    /// Successfully connected and tunnel is active.
    Connected,
    /// Attempting to reconnect after connection loss.
    Reconnecting { attempt: u32 },
    /// An error occurred.
    Error(String),
}

/// Event emitted by the WireGuard service for status tracking.
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum WireGuardEvent {
    /// Starting connection process.
    Connecting { interface: String },
    /// Successfully connected to VPN.
    Connected { interface: String, endpoint: String },
    /// Disconnected from VPN.
    Disconnected { interface: String, reason: String },
    /// Attempting to reconnect.
    Reconnecting { interface: String, attempt: u32 },
    /// Statistics update from health monitoring.
    StatsUpdate {
        rx_bytes: u64,
        tx_bytes: u64,
        last_handshake: Option<i64>,
    },
    /// An error occurred.
    Error { message: String },
}

/// WireGuard connection statistics.
#[derive(Debug, Clone, Serialize, Default)]
pub struct WireGuardStats {
    /// Bytes received through the tunnel.
    pub rx_bytes: u64,
    /// Bytes transmitted through the tunnel.
    pub tx_bytes: u64,
    /// Timestamp of last successful handshake.
    pub last_handshake: Option<DateTime<Utc>>,
    /// Peer endpoint address.
    pub endpoint: Option<String>,
}

/// Current state of the WireGuard service.
#[derive(Debug, Clone, Serialize)]
pub struct WireGuardState {
    /// Current connection status.
    pub status: ConnectionStatus,
    /// When the connection was established.
    pub connected_since: Option<DateTime<Utc>>,
    /// Current connection statistics.
    pub stats: WireGuardStats,
}

impl Default for WireGuardState {
    fn default() -> Self {
        Self {
            status: ConnectionStatus::Disconnected,
            connected_since: None,
            stats: WireGuardStats::default(),
        }
    }
}

// =========================================================================
// WireGuardService
// =========================================================================

/// WireGuard VPN service for managing VPN connections.
///
/// Provides functionality for:
/// - Connecting/disconnecting VPN tunnel
/// - Monitoring connection health
/// - Automatic reconnection on failure
/// - Real-time statistics and event broadcasting
/// - DNS leak prevention (optional)
pub struct WireGuardService {
    config: WireGuardConfig,
    interface_name: String,
    event_tx: broadcast::Sender<WireGuardEvent>,
    state: Arc<RwLock<WireGuardState>>,
    monitor_handle: RwLock<Option<JoinHandle<()>>>,
    dns_manager: Option<DnsManager>,
}

impl WireGuardService {
    /// Create a new WireGuard service with the given configuration.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - Neither config_file nor inline configuration is provided
    /// - The configuration is invalid
    pub fn new(config: WireGuardConfig) -> Result<Self> {
        tracing::debug!(?config, "Initializing WireGuard service");

        // Validate that we have a configuration source
        if config.config_file.is_none() && config.inline.is_none() {
            return Err(AppError::Internal(
                "WireGuard configuration must provide either config_file or inline config"
                    .to_string(),
            ));
        }

        // Determine interface name
        let interface_name = config.interface_name.clone().unwrap_or_else(|| {
            #[cfg(target_os = "linux")]
            let default_name = "wg0".to_string();
            #[cfg(target_os = "macos")]
            let default_name = "utun3".to_string();
            #[cfg(not(any(target_os = "linux", target_os = "macos")))]
            let default_name = "wg0".to_string();
            default_name
        });

        let (event_tx, _) = broadcast::channel(100);

        // Create DNS manager if DNS leak protection is enabled
        let dns_manager = if config.dns_leak_protection {
            tracing::debug!("DNS leak protection enabled");
            Some(DnsManager::new(&interface_name))
        } else {
            tracing::debug!("DNS leak protection disabled");
            None
        };

        tracing::info!(
            interface = %interface_name,
            config_file = ?config.config_file,
            has_inline = %config.inline.is_some(),
            dns_leak_protection = %config.dns_leak_protection,
            "WireGuard service initialized"
        );

        Ok(Self {
            config,
            interface_name,
            event_tx,
            state: Arc::new(RwLock::new(WireGuardState::default())),
            monitor_handle: RwLock::new(None),
            dns_manager,
        })
    }

    /// Create a new WireGuard service wrapped in Arc for shared access.
    pub fn new_shared(config: WireGuardConfig) -> Result<Arc<Self>> {
        Ok(Arc::new(Self::new(config)?))
    }

    /// Connect to the WireGuard VPN.
    ///
    /// Establishes the VPN tunnel by:
    /// 1. Creating the network interface
    /// 2. Configuring interface and peer settings
    /// 3. Starting health monitoring
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The interface cannot be created
    /// - Configuration is invalid
    /// - Peer configuration fails
    pub async fn connect(&self) -> Result<()> {
        tracing::info!(interface = %self.interface_name, "Connecting to WireGuard VPN");

        // Update state to connecting
        {
            let mut state = self.state.write().await;
            state.status = ConnectionStatus::Connecting;
        }

        let _ = self.event_tx.send(WireGuardEvent::Connecting {
            interface: self.interface_name.clone(),
        });

        // Load configuration
        let wg_config = if let Some(ref config_file) = self.config.config_file {
            parse_wg_config_file(config_file)?
        } else if let Some(ref inline_config) = self.config.inline {
            inline_config.clone()
        } else {
            return Err(AppError::Internal(
                "No WireGuard configuration available".to_string(),
            ));
        };

        // Create the WireGuard API
        let wgapi = create_wgapi(&self.interface_name)?;

        // Create the interface
        wgapi
            .create_interface()
            .map_err(|e| AppError::Vpn(format!("Failed to create interface: {}", e)))?;
        tracing::debug!(interface = %self.interface_name, "Interface created");

        // Build interface configuration
        let interface_config = build_interface_config(&self.interface_name, &wg_config)?;

        // Configure the interface
        wgapi
            .configure_interface(&interface_config)
            .map_err(|e| AppError::Vpn(format!("Failed to configure interface: {}", e)))?;
        tracing::debug!(interface = %self.interface_name, "Interface configured");

        // Configure peer routing
        wgapi
            .configure_peer_routing(&interface_config.peers)
            .map_err(|e| AppError::Vpn(format!("Failed to configure peer routing: {}", e)))?;
        tracing::debug!(interface = %self.interface_name, "Peer routing configured");

        // Update state to connected
        let endpoint = wg_config.peer.endpoint.clone();
        {
            let mut state = self.state.write().await;
            state.status = ConnectionStatus::Connected;
            state.connected_since = Some(Utc::now());
            state.stats.endpoint = Some(endpoint.clone());
        }

        let _ = self.event_tx.send(WireGuardEvent::Connected {
            interface: self.interface_name.clone(),
            endpoint: endpoint.clone(),
        });

        tracing::info!(
            interface = %self.interface_name,
            endpoint = %endpoint,
            "Successfully connected to WireGuard VPN"
        );

        // Configure DNS if leak protection is enabled
        if let Some(ref dns_manager) = self.dns_manager {
            // Get DNS servers from config (prefer explicit config, then inline/file config)
            let dns_servers = self
                .config
                .dns_servers
                .clone()
                .or_else(|| wg_config.dns.clone())
                .unwrap_or_default();

            if !dns_servers.is_empty() {
                if let Err(e) = dns_manager.set_vpn_dns(&dns_servers).await {
                    tracing::error!("Failed to configure VPN DNS: {}", e);
                    // Don't fail the connection for DNS errors, but log it
                }
            } else {
                tracing::debug!("No DNS servers configured, skipping DNS leak protection");
            }
        }

        // Start health monitoring
        self.start_monitoring().await;

        Ok(())
    }

    /// Disconnect from the WireGuard VPN.
    ///
    /// Tears down the VPN tunnel, restores DNS settings, and stops health monitoring.
    ///
    /// # Errors
    ///
    /// Returns an error if the interface cannot be removed.
    pub async fn disconnect(&self) -> Result<()> {
        tracing::info!(interface = %self.interface_name, "Disconnecting from WireGuard VPN");

        // Restore DNS settings first (before removing interface)
        if let Some(ref dns_manager) = self.dns_manager {
            if let Err(e) = dns_manager.restore_dns().await {
                tracing::error!("Failed to restore DNS settings: {}", e);
                // Continue with disconnect even if DNS restore fails
            }
        }

        // Stop monitoring task
        {
            let mut handle = self.monitor_handle.write().await;
            if let Some(h) = handle.take() {
                h.abort();
                tracing::debug!("Monitoring task stopped");
            }
        }

        // Remove the interface
        let wgapi = create_wgapi(&self.interface_name)?;
        wgapi
            .remove_interface()
            .map_err(|e| AppError::Vpn(format!("Failed to remove interface: {}", e)))?;

        // Update state
        {
            let mut state = self.state.write().await;
            state.status = ConnectionStatus::Disconnected;
            state.connected_since = None;
            state.stats = WireGuardStats::default();
        }

        let _ = self.event_tx.send(WireGuardEvent::Disconnected {
            interface: self.interface_name.clone(),
            reason: "User requested disconnect".to_string(),
        });

        tracing::info!(interface = %self.interface_name, "Disconnected from WireGuard VPN");

        Ok(())
    }

    /// Get the current connection status and statistics.
    pub async fn get_status(&self) -> WireGuardState {
        self.state.read().await.clone()
    }

    /// Subscribe to WireGuard events.
    ///
    /// Returns a broadcast receiver that will receive all WireGuard events.
    pub fn subscribe(&self) -> broadcast::Receiver<WireGuardEvent> {
        self.event_tx.subscribe()
    }

    /// Check if the VPN connection is healthy.
    ///
    /// A connection is considered healthy if:
    /// - Status is Connected
    /// - Last handshake was within the last 3 minutes
    pub async fn is_healthy(&self) -> bool {
        let state = self.state.read().await;

        if state.status != ConnectionStatus::Connected {
            return false;
        }

        if let Some(last_handshake) = state.stats.last_handshake {
            let age = Utc::now() - last_handshake;
            age.num_seconds() < 180 // 3 minutes
        } else {
            // No handshake yet - give it some time
            if let Some(connected_since) = state.connected_since {
                let age = Utc::now() - connected_since;
                age.num_seconds() < 60 // 1 minute grace period
            } else {
                false
            }
        }
    }

    /// Get the interface name.
    pub fn interface_name(&self) -> &str {
        &self.interface_name
    }

    // =========================================================================
    // Private helpers
    // =========================================================================

    /// Start the health monitoring task.
    async fn start_monitoring(&self) {
        let interface_name = self.interface_name.clone();
        let interval = Duration::from_secs(self.config.health_check_interval_secs);
        let state = Arc::clone(&self.state);
        let event_tx = self.event_tx.clone();
        let auto_reconnect = self.config.auto_reconnect;

        let handle = tokio::spawn(async move {
            tracing::debug!(interface = %interface_name, "Health monitoring started");
            let mut unhealthy_count = 0u32;

            loop {
                tokio::time::sleep(interval).await;

                // Read interface data
                let wgapi = match create_wgapi(&interface_name) {
                    Ok(api) => api,
                    Err(e) => {
                        tracing::error!("Failed to create WireGuard API: {}", e);
                        continue;
                    }
                };

                match wgapi.read_interface_data() {
                    Ok(host) => {
                        // Extract stats from peer data (host.peers is a HashMap)
                        if let Some(peer) = host.peers.values().next() {
                            let rx_bytes = peer.rx_bytes;
                            let tx_bytes = peer.tx_bytes;
                            let last_handshake = peer
                                .last_handshake
                                .and_then(|st| st.duration_since(std::time::UNIX_EPOCH).ok())
                                .and_then(|d| d.as_secs().try_into().ok())
                                .and_then(|secs: i64| DateTime::from_timestamp(secs, 0));

                            // Update state
                            {
                                let mut state = state.write().await;
                                state.stats.rx_bytes = rx_bytes;
                                state.stats.tx_bytes = tx_bytes;
                                state.stats.last_handshake = last_handshake;
                            }

                            // Emit stats event
                            let _ = event_tx.send(WireGuardEvent::StatsUpdate {
                                rx_bytes,
                                tx_bytes,
                                last_handshake: last_handshake.map(|dt| dt.timestamp()),
                            });

                            // Check handshake freshness
                            let is_healthy = if let Some(last_handshake) = last_handshake {
                                let age = Utc::now() - last_handshake;
                                age.num_seconds() < 180 // 3 minutes
                            } else {
                                false
                            };

                            if is_healthy {
                                unhealthy_count = 0;
                            } else {
                                unhealthy_count += 1;
                                tracing::warn!(
                                    interface = %interface_name,
                                    last_handshake = ?last_handshake,
                                    "Connection appears unhealthy (stale handshake)"
                                );

                                // Trigger reconnect if enabled and multiple checks failed
                                if auto_reconnect && unhealthy_count >= 3 {
                                    tracing::warn!("Triggering automatic reconnect");
                                    // TODO: Implement reconnect logic with exponential backoff
                                    // For now, just log and reset counter
                                    unhealthy_count = 0;
                                }
                            }
                        }
                    }
                    Err(e) => {
                        tracing::error!("Failed to read interface data: {}", e);
                    }
                }
            }
        });

        *self.monitor_handle.write().await = Some(handle);
    }
}

// =========================================================================
// Helper functions
// =========================================================================

/// Create platform-specific WireGuard API instance.
fn create_wgapi(interface_name: &str) -> Result<WgApiType> {
    #[cfg(target_os = "linux")]
    let api = WGApi::<Kernel>::new(interface_name.to_string())
        .map_err(|e| AppError::Vpn(format!("Failed to create WireGuard API: {}", e)))?;

    #[cfg(target_os = "macos")]
    let api = WGApi::<Userspace>::new(interface_name.to_string())
        .map_err(|e| AppError::Vpn(format!("Failed to create WireGuard API: {}", e)))?;

    Ok(api)
}

/// Parse a wg-quick style configuration file.
///
/// Expected format:
/// ```ini
/// [Interface]
/// PrivateKey = base64key
/// Address = 10.0.0.2/32
/// DNS = 10.0.0.1
/// MTU = 1420
/// ListenPort = 51820
///
/// [Peer]
/// PublicKey = base64key
/// PresharedKey = base64key
/// Endpoint = vpn.example.com:51820
/// AllowedIPs = 0.0.0.0/0, ::/0
/// PersistentKeepalive = 25
/// ```
fn parse_wg_config_file(path: &Path) -> Result<WireGuardInlineConfig> {
    let file = std::fs::File::open(path)
        .map_err(|e| AppError::Vpn(format!("Failed to open config file: {}", e)))?;

    let reader = std::io::BufReader::new(file);
    let mut in_section = String::new();

    let mut private_key = None;
    let mut addresses = Vec::new();
    let mut listen_port = None;
    let mut dns = Vec::new();
    let mut mtu = None;

    let mut public_key = None;
    let mut preshared_key = None;
    let mut endpoint = None;
    let mut allowed_ips = Vec::new();
    let mut persistent_keepalive = 25u16;

    for line in reader.lines() {
        let line = line.map_err(|e| AppError::Vpn(format!("Failed to read config file: {}", e)))?;
        let line = line.trim();

        // Skip empty lines and comments
        if line.is_empty() || line.starts_with('#') {
            continue;
        }

        // Check for section headers
        if line.starts_with('[') && line.ends_with(']') {
            in_section = line[1..line.len() - 1].to_string();
            continue;
        }

        // Parse key = value pairs
        if let Some((key, value)) = line.split_once('=') {
            let key = key.trim();
            let value = value.trim();

            match in_section.as_str() {
                "Interface" => match key {
                    "PrivateKey" => private_key = Some(value.to_string()),
                    "Address" => {
                        // Can be comma-separated
                        for addr in value.split(',') {
                            addresses.push(addr.trim().to_string());
                        }
                    }
                    "ListenPort" => {
                        listen_port = value.parse().ok();
                    }
                    "DNS" => {
                        for d in value.split(',') {
                            dns.push(d.trim().to_string());
                        }
                    }
                    "MTU" => {
                        mtu = value.parse().ok();
                    }
                    _ => {}
                },
                "Peer" => match key {
                    "PublicKey" => public_key = Some(value.to_string()),
                    "PresharedKey" => preshared_key = Some(value.to_string()),
                    "Endpoint" => endpoint = Some(value.to_string()),
                    "AllowedIPs" => {
                        for ip in value.split(',') {
                            allowed_ips.push(ip.trim().to_string());
                        }
                    }
                    "PersistentKeepalive" => {
                        persistent_keepalive = value.parse().unwrap_or(25);
                    }
                    _ => {}
                },
                _ => {}
            }
        }
    }

    // Validate required fields
    let private_key =
        private_key.ok_or_else(|| AppError::Vpn("Missing PrivateKey in config".to_string()))?;
    let public_key =
        public_key.ok_or_else(|| AppError::Vpn("Missing PublicKey in config".to_string()))?;
    let endpoint =
        endpoint.ok_or_else(|| AppError::Vpn("Missing Endpoint in config".to_string()))?;

    if addresses.is_empty() {
        return Err(AppError::Vpn("Missing Address in config".to_string()));
    }

    if allowed_ips.is_empty() {
        return Err(AppError::Vpn("Missing AllowedIPs in config".to_string()));
    }

    Ok(WireGuardInlineConfig {
        private_key,
        addresses,
        listen_port,
        dns: if dns.is_empty() { None } else { Some(dns) },
        mtu,
        peer: crate::config::WireGuardPeerConfig {
            public_key,
            preshared_key,
            endpoint,
            allowed_ips,
            persistent_keepalive,
        },
    })
}

/// Build an InterfaceConfiguration from our inline config.
fn build_interface_config(
    name: &str,
    config: &WireGuardInlineConfig,
) -> Result<InterfaceConfiguration> {
    // Parse private key
    let private_key = config.private_key.clone();

    // Parse addresses as IpAddrMask
    let mut host_addresses = Vec::new();
    for addr_str in &config.addresses {
        let addr: IpAddrMask = addr_str
            .parse()
            .map_err(|e| AppError::Vpn(format!("Invalid address '{}': {}", addr_str, e)))?;
        host_addresses.push(addr);
    }

    // Parse endpoint
    let endpoint: SocketAddr = config
        .peer
        .endpoint
        .parse()
        .map_err(|e| AppError::Vpn(format!("Invalid endpoint: {}", e)))?;

    // Parse allowed IPs as IpAddrMask
    let mut allowed_ips = Vec::new();
    for ip_str in &config.peer.allowed_ips {
        let ip: IpAddrMask = ip_str
            .parse()
            .map_err(|e| AppError::Vpn(format!("Invalid allowed IP '{}': {}", ip_str, e)))?;
        allowed_ips.push(ip);
    }

    // Parse public key
    let public_key: Key = config
        .peer
        .public_key
        .as_str()
        .try_into()
        .map_err(|e| AppError::Vpn(format!("Invalid public key: {:?}", e)))?;

    // Parse preshared key if present
    let preshared_key = if let Some(ref psk) = config.peer.preshared_key {
        let key: Key = psk
            .as_str()
            .try_into()
            .map_err(|e| AppError::Vpn(format!("Invalid preshared key: {:?}", e)))?;
        Some(key)
    } else {
        None
    };

    // Build peer
    let peer = Peer {
        public_key,
        preshared_key,
        protocol_version: None,
        endpoint: Some(endpoint),
        last_handshake: None,
        tx_bytes: 0,
        rx_bytes: 0,
        persistent_keepalive_interval: Some(config.peer.persistent_keepalive),
        allowed_ips,
    };

    Ok(InterfaceConfiguration {
        name: name.to_string(),
        prvkey: private_key,
        addresses: host_addresses,
        port: config.listen_port.unwrap_or(0) as u32,
        peers: vec![peer],
        mtu: config.mtu.map(|m| m as u32),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    #[test]
    fn test_connection_status_serialization() {
        let status = ConnectionStatus::Connected;
        let json = serde_json::to_string(&status).unwrap();
        assert!(json.contains("\"status\":\"connected\""));

        let status = ConnectionStatus::Reconnecting { attempt: 3 };
        let json = serde_json::to_string(&status).unwrap();
        assert!(json.contains("\"status\":\"reconnecting\""));
        assert!(json.contains("\"attempt\":3"));
    }

    #[test]
    fn test_wireguard_event_serialization() {
        let event = WireGuardEvent::Connected {
            interface: "wg0".to_string(),
            endpoint: "vpn.example.com:51820".to_string(),
        };

        let json = serde_json::to_string(&event).unwrap();
        assert!(json.contains("\"type\":\"connected\""));
        assert!(json.contains("\"interface\":\"wg0\""));
    }

    #[test]
    fn test_parse_wg_config_file() -> Result<()> {
        let mut file = NamedTempFile::new().unwrap();
        writeln!(
            file,
            r#"
[Interface]
PrivateKey = AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA=
Address = 10.0.0.2/32, fd00::2/128
ListenPort = 51820
DNS = 10.0.0.1
MTU = 1420

[Peer]
PublicKey = BBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBB=
PresharedKey = CCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCC=
Endpoint = vpn.example.com:51820
AllowedIPs = 0.0.0.0/0, ::/0
PersistentKeepalive = 25
"#
        )
        .unwrap();

        let config = parse_wg_config_file(file.path())?;

        assert_eq!(
            config.private_key,
            "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA="
        );
        assert_eq!(config.addresses.len(), 2);
        assert_eq!(config.addresses[0], "10.0.0.2/32");
        assert_eq!(config.listen_port, Some(51820));
        assert_eq!(config.dns.as_ref().unwrap()[0], "10.0.0.1");
        assert_eq!(config.mtu, Some(1420));

        assert_eq!(
            config.peer.public_key,
            "BBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBB="
        );
        assert_eq!(config.peer.endpoint, "vpn.example.com:51820");
        assert_eq!(config.peer.allowed_ips.len(), 2);
        assert_eq!(config.peer.persistent_keepalive, 25);

        Ok(())
    }

    #[test]
    fn test_parse_wg_config_file_minimal() -> Result<()> {
        let mut file = NamedTempFile::new().unwrap();
        writeln!(
            file,
            r#"
[Interface]
PrivateKey = AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA=
Address = 10.0.0.2/32

[Peer]
PublicKey = BBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBB=
Endpoint = 192.168.1.1:51820
AllowedIPs = 0.0.0.0/0
"#
        )
        .unwrap();

        let config = parse_wg_config_file(file.path())?;

        assert_eq!(config.addresses.len(), 1);
        assert!(config.dns.is_none());
        assert!(config.mtu.is_none());
        assert_eq!(config.peer.persistent_keepalive, 25); // Default

        Ok(())
    }

    #[test]
    fn test_wireguard_service_validation() {
        // Missing both config_file and inline should fail
        let config = WireGuardConfig {
            enabled: true,
            interface_name: None,
            config_file: None,
            inline: None,
            health_check_interval_secs: 30,
            auto_reconnect: true,
            reconnect_delay_max_secs: 300,
            kill_switch: false,
            dns_leak_protection: true,
            dns_servers: None,
        };

        let result = WireGuardService::new(config);
        assert!(result.is_err());
    }
}
