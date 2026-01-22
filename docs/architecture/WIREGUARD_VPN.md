# WireGuard VPN Integration Architecture

## Overview

This document describes the architecture for integrating WireGuard VPN directly into LCARS to protect torrent traffic. The goal is to ensure **all** torrent-related traffic (TCP, UDP, DNS) flows through an encrypted VPN tunnel, preventing IP leaks.

## Problem Statement

The current `bind_interface` configuration in the torrent service is not implemented. Using a SOCKS5 proxy (which librqbit supports) has significant privacy issues:

| Traffic Type | SOCKS5 Proxy | WireGuard Tunnel |
|--------------|--------------|------------------|
| TCP peer connections | ✅ Proxied | ✅ Protected |
| UDP trackers | ❌ **Leaks** | ✅ Protected |
| DHT (UDP) | ❌ **Leaks** | ✅ Protected |
| DNS resolution | ❌ **Leaks** | ✅ Protected |

See: https://github.com/ikatson/rqbit/issues/493

## Solution Architecture

### High-Level Design

```
┌─────────────────────────────────────────────────────────────────┐
│                         LCARS Application                        │
├─────────────────────────────────────────────────────────────────┤
│                                                                  │
│  ┌──────────────────┐      ┌──────────────────────────────────┐ │
│  │  WireGuard       │      │  Torrent Engine (librqbit)       │ │
│  │  Service         │      │                                  │ │
│  │                  │      │  - Bound to wg0 interface        │ │
│  │  - Create wg0    │─────▶│  - All traffic via tunnel        │ │
│  │  - Configure     │      │  - Kill switch on disconnect     │ │
│  │  - Monitor       │      │                                  │ │
│  └──────────────────┘      └──────────────────────────────────┘ │
│           │                              │                       │
│           │ Events                       │ Events                │
│           ▼                              ▼                       │
│  ┌─────────────────────────────────────────────────────────────┐│
│  │                    Event Bus (broadcast)                    ││
│  └─────────────────────────────────────────────────────────────┘│
│                              │                                   │
└──────────────────────────────┼───────────────────────────────────┘
                               │
                               ▼
┌─────────────────────────────────────────────────────────────────┐
│                        Linux Kernel                              │
│  ┌─────────────┐    ┌─────────────┐    ┌─────────────────────┐  │
│  │   wg0       │    │  Routing    │    │  Network Namespace  │  │
│  │  interface  │───▶│  Tables     │───▶│  (optional)         │  │
│  └─────────────┘    └─────────────┘    └─────────────────────┘  │
└─────────────────────────────────────────────────────────────────┘
```

### Components

#### 1. WireGuard Service (`services/wireguard.rs`)

Manages the WireGuard interface lifecycle using `defguard_wireguard_rs`.

```rust
pub struct WireGuardService {
    config: WireGuardConfig,
    wgapi: WGApi<Kernel>,
    event_tx: broadcast::Sender<WireGuardEvent>,
    state: Arc<RwLock<WireGuardState>>,
    monitor_handle: RwLock<Option<JoinHandle<()>>>,
}

pub struct WireGuardState {
    pub status: ConnectionStatus,
    pub interface_name: String,
    pub connected_since: Option<DateTime<Utc>>,
    pub stats: Option<InterfaceStats>,
    pub last_handshake: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ConnectionStatus {
    Disconnected,
    Connecting,
    Connected,
    Reconnecting,
    Error(String),
}

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum WireGuardEvent {
    Connecting { interface: String },
    Connected { interface: String, endpoint: String },
    Disconnected { interface: String, reason: String },
    Reconnecting { interface: String, attempt: u32 },
    StatsUpdate { rx_bytes: u64, tx_bytes: u64, last_handshake: Option<i64> },
    Error { message: String },
}
```

**Key Methods:**

```rust
impl WireGuardService {
    /// Create a new WireGuard service
    pub async fn new(config: WireGuardConfig) -> Result<Self>;

    /// Create wrapped in Arc for shared access
    pub async fn new_shared(config: WireGuardConfig) -> Result<Arc<Self>>;

    /// Bring up the WireGuard interface and establish connection
    pub async fn connect(&self) -> Result<()>;

    /// Tear down the WireGuard interface
    pub async fn disconnect(&self) -> Result<()>;

    /// Get current connection status and stats
    pub async fn get_status(&self) -> WireGuardState;

    /// Subscribe to connection events
    pub fn subscribe(&self) -> broadcast::Receiver<WireGuardEvent>;

    /// Check if VPN is healthy (recent handshake)
    pub async fn is_healthy(&self) -> bool;

    /// Get the interface name for binding
    pub fn interface_name(&self) -> &str;
}
```

#### 2. Configuration (`config.rs`)

```rust
#[derive(Debug, Clone, Deserialize)]
pub struct WireGuardConfig {
    /// Enable WireGuard VPN integration
    pub enabled: bool,

    /// Interface name (default: "wg0" on Linux, "utun3" on macOS)
    pub interface_name: Option<String>,

    /// Path to WireGuard config file (standard wg-quick format)
    /// If provided, overrides inline configuration
    pub config_file: Option<PathBuf>,

    /// Inline configuration (alternative to config_file)
    pub inline: Option<WireGuardInlineConfig>,

    /// How often to check connection health (seconds)
    #[serde(default = "default_health_check_interval")]
    pub health_check_interval_secs: u64,

    /// Auto-reconnect on connection loss
    #[serde(default = "default_auto_reconnect")]
    pub auto_reconnect: bool,

    /// Maximum reconnect delay (exponential backoff cap)
    #[serde(default = "default_reconnect_delay_max")]
    pub reconnect_delay_max_secs: u64,

    /// Kill switch: pause torrents if VPN disconnects
    #[serde(default = "default_kill_switch")]
    pub kill_switch: bool,
}

#[derive(Debug, Clone, Deserialize)]
pub struct WireGuardInlineConfig {
    /// Interface private key (base64)
    pub private_key: String,

    /// Interface address(es) with CIDR
    pub addresses: Vec<String>,

    /// Listen port (optional, random if not specified)
    pub listen_port: Option<u16>,

    /// DNS servers to use when connected
    pub dns: Option<Vec<String>>,

    /// MTU (optional)
    pub mtu: Option<u16>,

    /// Peer configuration
    pub peer: WireGuardPeerConfig,
}

#[derive(Debug, Clone, Deserialize)]
pub struct WireGuardPeerConfig {
    /// Peer public key (base64)
    pub public_key: String,

    /// Pre-shared key for additional security (optional, base64)
    pub preshared_key: Option<String>,

    /// Peer endpoint (host:port)
    pub endpoint: String,

    /// Allowed IPs (typically "0.0.0.0/0" for full tunnel)
    pub allowed_ips: Vec<String>,

    /// Persistent keepalive interval (seconds)
    #[serde(default = "default_keepalive")]
    pub persistent_keepalive: u16,
}

// Defaults
fn default_health_check_interval() -> u64 { 30 }
fn default_auto_reconnect() -> bool { true }
fn default_reconnect_delay_max() -> u64 { 300 }
fn default_kill_switch() -> bool { true }
fn default_keepalive() -> u16 { 25 }
```

**Example configuration (`config.toml`):**

```toml
[wireguard]
enabled = true
kill_switch = true
auto_reconnect = true
health_check_interval_secs = 30

# Option 1: Use existing WireGuard config file
config_file = "/etc/wireguard/mullvad.conf"

# Option 2: Inline configuration
[wireguard.inline]
private_key = "your-private-key-base64"
addresses = ["10.66.66.2/32"]
dns = ["10.64.0.1"]

[wireguard.inline.peer]
public_key = "server-public-key-base64"
endpoint = "vpn.example.com:51820"
allowed_ips = ["0.0.0.0/0", "::/0"]
persistent_keepalive = 25
```

**Environment variables:**

```bash
LCARS_WIREGUARD__ENABLED=true
LCARS_WIREGUARD__CONFIG_FILE=/etc/wireguard/mullvad.conf
LCARS_WIREGUARD__KILL_SWITCH=true
LCARS_WIREGUARD__INLINE__PRIVATE_KEY=your-key
LCARS_WIREGUARD__INLINE__PEER__PUBLIC_KEY=server-key
LCARS_WIREGUARD__INLINE__PEER__ENDPOINT=vpn.example.com:51820
```

#### 3. Traffic Binding Strategy

Two approaches, configurable:

##### Option A: Policy-Based Routing (Recommended for Linux)

Uses `fwmark` to tag packets and route them through the WireGuard interface:

```rust
impl WireGuardService {
    async fn setup_routing(&self) -> Result<()> {
        // 1. Mark packets from torrent process
        // 2. Add routing rule: packets with mark -> wg0
        // 3. Add route in table for marked packets

        let fwmark = 51820; // Configurable

        // ip rule add fwmark $fwmark table $table
        // ip route add default dev wg0 table $table
    }
}
```

##### Option B: Network Namespace (Alternative)

Run the torrent engine in an isolated network namespace:

```rust
// More complex but provides stronger isolation
// Requires spawning torrent engine in separate process/namespace
```

**Decision:** Start with policy-based routing (Option A) as it's simpler and works well with the existing in-process architecture.

#### 4. Kill Switch Integration

The torrent engine listens for WireGuard events:

```rust
// In TorrentEngine
pub async fn enable_vpn_kill_switch(&self, wireguard: Arc<WireGuardService>) {
    let mut rx = wireguard.subscribe();
    let engine = self.clone();

    tokio::spawn(async move {
        while let Ok(event) = rx.recv().await {
            match event {
                WireGuardEvent::Disconnected { .. } |
                WireGuardEvent::Error { .. } => {
                    tracing::warn!("VPN disconnected, pausing all torrents (kill switch)");
                    engine.pause_all().await;
                }
                WireGuardEvent::Connected { .. } => {
                    tracing::info!("VPN connected, resuming torrents");
                    engine.resume_all().await;
                }
                _ => {}
            }
        }
    });
}
```

#### 5. API Endpoints

```rust
// GET /api/vpn/status
pub async fn get_vpn_status(State(state): State<AppState>) -> Result<Json<VpnStatusResponse>>;

// POST /api/vpn/connect
pub async fn connect_vpn(State(state): State<AppState>) -> Result<Json<SuccessResponse>>;

// POST /api/vpn/disconnect
pub async fn disconnect_vpn(State(state): State<AppState>) -> Result<Json<SuccessResponse>>;

// GET /api/vpn/stats
pub async fn get_vpn_stats(State(state): State<AppState>) -> Result<Json<VpnStatsResponse>>;

#[derive(Serialize)]
pub struct VpnStatusResponse {
    pub enabled: bool,
    pub status: String,  // "connected", "disconnected", "connecting", "error"
    pub interface: Option<String>,
    pub endpoint: Option<String>,
    pub connected_since: Option<String>,
    pub last_handshake: Option<String>,
    pub kill_switch_active: bool,
}

#[derive(Serialize)]
pub struct VpnStatsResponse {
    pub rx_bytes: u64,
    pub tx_bytes: u64,
    pub rx_packets: u64,
    pub tx_packets: u64,
}
```

#### 6. UI Integration

Add VPN status to the dashboard and settings:

**Dashboard (`templates/pages/dashboard.html`):**
- VPN connection status indicator (green/red)
- Quick connect/disconnect toggle
- Current endpoint display

**Settings (`templates/pages/settings.html`):**
- WireGuard configuration form
- Import from config file
- Test connection button
- Kill switch toggle

**Downloads page:**
- Warning banner if VPN is disconnected and kill switch is off

### Initialization Flow

```rust
// main.rs
async fn main() -> Result<()> {
    // ... existing init ...

    // Initialize WireGuard service (before torrent engine)
    let wireguard_service = if config.wireguard.enabled {
        match WireGuardService::new_shared(config.wireguard.clone()).await {
            Ok(service) => {
                tracing::info!(
                    interface = %service.interface_name(),
                    "WireGuard service initialized"
                );

                // Auto-connect if configured
                if let Err(e) = service.connect().await {
                    tracing::error!("Failed to connect WireGuard: {}", e);
                    if config.wireguard.kill_switch {
                        tracing::warn!("Kill switch enabled - torrents will not start");
                    }
                }

                Some(service)
            }
            Err(e) => {
                tracing::error!("Failed to initialize WireGuard: {}", e);
                None
            }
        }
    } else {
        None
    };

    // Initialize torrent engine with VPN dependency
    let torrent_engine = TorrentEngine::new_shared(
        config.torrent.clone(),
        wireguard_service.clone(),  // Pass WireGuard reference
    ).await.ok();

    // Enable kill switch if configured
    if let (Some(ref torrent), Some(ref wg)) = (&torrent_engine, &wireguard_service) {
        if config.wireguard.kill_switch {
            torrent.enable_vpn_kill_switch(wg.clone()).await;
        }
    }

    // ... rest of init ...
}
```

### Error Handling

```rust
// New error variants in error.rs
pub enum AppError {
    // ... existing variants ...

    #[error("VPN error: {0}")]
    Vpn(String),

    #[error("VPN not configured")]
    VpnNotConfigured,

    #[error("VPN disconnected - operation blocked by kill switch")]
    VpnKillSwitch,
}
```

### Permissions & Capabilities

**Linux:**
```bash
# Grant network admin capability
sudo setcap cap_net_admin+ep /usr/local/bin/lcars

# Or run with sudo (not recommended for production)
sudo lcars
```

**macOS (development):**
```bash
# Run as root for userspace WireGuard
sudo cargo run
```

### Dependencies

Add to `Cargo.toml`:

```toml
[dependencies]
defguard_wireguard_rs = "0.7"
```

### Security Considerations

1. **Private key storage**: The WireGuard private key is sensitive. Consider:
   - Environment variable injection (recommended)
   - Encrypted config file
   - Secret management system integration

2. **Config file permissions**: If using `config_file`, ensure it's readable only by the lcars user.

3. **Capability scope**: `CAP_NET_ADMIN` allows creating/modifying network interfaces. Run as a dedicated user.

4. **DNS leaks**: When WireGuard is connected, DNS queries should go through the tunnel. The service should configure system DNS or use the VPN provider's DNS.

### Testing Strategy

1. **Unit tests**: Mock `WGApi` trait for interface operations
2. **Integration tests**:
   - Requires `CAP_NET_ADMIN` or root
   - Create/destroy test interfaces
   - Skip in CI without capabilities
3. **Manual testing**:
   - Verify no traffic leaks (tcpdump on physical interface)
   - Test kill switch by disconnecting VPN
   - Test reconnection logic

### Future Enhancements

1. **Multiple VPN providers**: Support for different WireGuard configs (Mullvad, ProtonVPN, etc.)
2. **Port forwarding**: Some VPN providers offer port forwarding - integrate with torrent listen port
3. **Split tunneling**: Allow some traffic (web UI) to bypass VPN
4. **Connection quality metrics**: Latency, packet loss monitoring
5. **Automatic server selection**: Choose best endpoint based on latency

## Implementation Phases

### Phase 1: Core WireGuard Service
- [ ] Add `defguard_wireguard_rs` dependency
- [ ] Implement `WireGuardConfig` in config.rs
- [ ] Implement `WireGuardService` with connect/disconnect
- [ ] Add to AppState and initialization
- [ ] Basic API endpoints (status, connect, disconnect)

### Phase 2: Kill Switch & Torrent Integration
- [ ] Event subscription system
- [ ] Kill switch in torrent engine
- [ ] Policy-based routing setup
- [ ] Health monitoring and auto-reconnect

### Phase 3: UI Integration
- [ ] Dashboard VPN status widget
- [ ] Settings page VPN configuration
- [ ] Downloads page warning banner

### Phase 4: Hardening
- [ ] DNS leak prevention
- [ ] Connection quality monitoring
- [ ] Comprehensive error handling
- [ ] Documentation
