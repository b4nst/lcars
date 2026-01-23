//! DNS manager for preventing DNS leaks when VPN is connected.
//!
//! This module provides platform-specific DNS management:
//! - **Linux**: Uses systemd-resolved (if available) or falls back to /etc/resolv.conf
//! - **macOS**: Uses scutil and networksetup commands
//!
//! When the VPN connects, DNS servers from the WireGuard config are set as system DNS.
//! When the VPN disconnects, the original DNS settings are restored.

use std::net::IpAddr;
use std::sync::Arc;
use tokio::sync::RwLock;

use crate::error::{AppError, Result};

/// Stored DNS state for restoration on disconnect.
#[derive(Debug, Clone, Default)]
pub struct DnsState {
    /// Original DNS servers before VPN connection.
    pub original_servers: Vec<IpAddr>,
    /// Original search domains.
    pub original_domains: Vec<String>,
    /// Platform-specific backup data (e.g., resolv.conf contents on Linux).
    pub backup_data: Option<String>,
    /// Whether DNS is currently managed by us.
    pub is_managed: bool,
}

/// DNS manager for handling system DNS configuration.
pub struct DnsManager {
    state: Arc<RwLock<DnsState>>,
    interface_name: String,
}

impl DnsManager {
    /// Create a new DNS manager for the given interface.
    pub fn new(interface_name: &str) -> Self {
        Self {
            state: Arc::new(RwLock::new(DnsState::default())),
            interface_name: interface_name.to_string(),
        }
    }

    /// Set VPN DNS servers, backing up current DNS configuration.
    ///
    /// # Arguments
    ///
    /// * `dns_servers` - DNS server addresses to set
    ///
    /// # Errors
    ///
    /// Returns an error if DNS configuration fails.
    pub async fn set_vpn_dns(&self, dns_servers: &[String]) -> Result<()> {
        if dns_servers.is_empty() {
            tracing::debug!("No DNS servers provided, skipping DNS configuration");
            return Ok(());
        }

        // Parse DNS server addresses
        let dns_addrs: Vec<IpAddr> = dns_servers
            .iter()
            .filter_map(|s| {
                s.parse::<IpAddr>().ok().or_else(|| {
                    tracing::warn!("Invalid DNS server address: {}", s);
                    None
                })
            })
            .collect();

        if dns_addrs.is_empty() {
            tracing::warn!("No valid DNS server addresses after parsing");
            return Ok(());
        }

        // Backup current DNS settings
        let backup = self.backup_dns().await?;

        // Update state
        {
            let mut state = self.state.write().await;
            state.backup_data = backup;
            state.is_managed = true;
        }

        // Set new DNS
        self.configure_dns(&dns_addrs).await?;

        tracing::info!(
            interface = %self.interface_name,
            servers = ?dns_addrs,
            "VPN DNS configured"
        );

        Ok(())
    }

    /// Restore original DNS configuration.
    ///
    /// # Errors
    ///
    /// Returns an error if DNS restoration fails.
    pub async fn restore_dns(&self) -> Result<()> {
        let state = self.state.read().await;

        if !state.is_managed {
            tracing::debug!("DNS not managed, nothing to restore");
            return Ok(());
        }

        drop(state); // Release read lock before write operations

        self.restore_dns_impl().await?;

        // Clear state
        {
            let mut state = self.state.write().await;
            state.is_managed = false;
            state.backup_data = None;
            state.original_servers.clear();
            state.original_domains.clear();
        }

        tracing::info!(
            interface = %self.interface_name,
            "Original DNS restored"
        );

        Ok(())
    }

    /// Check if DNS is currently managed.
    pub async fn is_managed(&self) -> bool {
        self.state.read().await.is_managed
    }

    // =========================================================================
    // Platform-specific implementations
    // =========================================================================

    #[cfg(target_os = "linux")]
    async fn backup_dns(&self) -> Result<Option<String>> {
        use tokio::fs;

        // Try systemd-resolved first
        if self.is_systemd_resolved_available().await {
            tracing::debug!("Using systemd-resolved for DNS management");
            // For systemd-resolved, we don't need to backup - it handles per-link DNS
            return Ok(None);
        }

        // Fall back to /etc/resolv.conf
        tracing::debug!("Using /etc/resolv.conf for DNS management");
        match fs::read_to_string("/etc/resolv.conf").await {
            Ok(content) => Ok(Some(content)),
            Err(e) => {
                tracing::warn!("Failed to read /etc/resolv.conf: {}", e);
                Ok(None)
            }
        }
    }

    #[cfg(target_os = "linux")]
    async fn configure_dns(&self, dns_servers: &[IpAddr]) -> Result<()> {
        if self.is_systemd_resolved_available().await {
            self.configure_dns_systemd_resolved(dns_servers).await
        } else {
            self.configure_dns_resolv_conf(dns_servers).await
        }
    }

    #[cfg(target_os = "linux")]
    async fn is_systemd_resolved_available(&self) -> bool {
        tokio::process::Command::new("systemctl")
            .args(["is-active", "--quiet", "systemd-resolved"])
            .status()
            .await
            .map(|s| s.success())
            .unwrap_or(false)
    }

    #[cfg(target_os = "linux")]
    async fn configure_dns_systemd_resolved(&self, dns_servers: &[IpAddr]) -> Result<()> {
        use tokio::process::Command;

        // Get the interface index
        let output = Command::new("ip")
            .args(["link", "show", &self.interface_name])
            .output()
            .await
            .map_err(|e| AppError::Dns(format!("Failed to get interface index: {}", e)))?;

        if !output.status.success() {
            return Err(AppError::Dns(format!(
                "Interface {} not found",
                self.interface_name
            )));
        }

        // Build DNS string
        let dns_str = dns_servers
            .iter()
            .map(|ip| ip.to_string())
            .collect::<Vec<_>>()
            .join(" ");

        // Use resolvectl to set DNS for the interface
        let status = Command::new("resolvectl")
            .args(["dns", &self.interface_name, &dns_str])
            .status()
            .await
            .map_err(|e| AppError::Dns(format!("Failed to run resolvectl: {}", e)))?;

        if !status.success() {
            return Err(AppError::Dns("resolvectl dns command failed".to_string()));
        }

        // Set the interface as the default route for DNS
        let status = Command::new("resolvectl")
            .args(["default-route", &self.interface_name, "true"])
            .status()
            .await
            .map_err(|e| AppError::Dns(format!("Failed to set default route: {}", e)))?;

        if !status.success() {
            tracing::warn!("Failed to set interface as default DNS route");
        }

        Ok(())
    }

    #[cfg(target_os = "linux")]
    async fn configure_dns_resolv_conf(&self, dns_servers: &[IpAddr]) -> Result<()> {
        use tokio::fs;

        // Build new resolv.conf content
        let mut content = String::from("# Managed by LCARS VPN - DO NOT EDIT\n");
        for server in dns_servers {
            content.push_str(&format!("nameserver {}\n", server));
        }

        // Write to resolv.conf
        fs::write("/etc/resolv.conf", &content)
            .await
            .map_err(|e| AppError::Dns(format!("Failed to write /etc/resolv.conf: {}", e)))?;

        Ok(())
    }

    #[cfg(target_os = "linux")]
    async fn restore_dns_impl(&self) -> Result<()> {
        let state = self.state.read().await;

        if self.is_systemd_resolved_available().await {
            // For systemd-resolved, just revert the interface settings
            let status = tokio::process::Command::new("resolvectl")
                .args(["revert", &self.interface_name])
                .status()
                .await
                .map_err(|e| AppError::Dns(format!("Failed to revert DNS: {}", e)))?;

            if !status.success() {
                tracing::warn!("resolvectl revert command may have failed");
            }
        } else if let Some(ref backup) = state.backup_data {
            // Restore /etc/resolv.conf
            tokio::fs::write("/etc/resolv.conf", backup)
                .await
                .map_err(|e| AppError::Dns(format!("Failed to restore /etc/resolv.conf: {}", e)))?;
        }

        Ok(())
    }

    #[cfg(target_os = "macos")]
    async fn backup_dns(&self) -> Result<Option<String>> {
        use tokio::process::Command;

        // Get current DNS configuration using scutil
        let output = Command::new("scutil")
            .args(["--dns"])
            .output()
            .await
            .map_err(|e| AppError::Dns(format!("Failed to run scutil: {}", e)))?;

        if output.status.success() {
            Ok(Some(String::from_utf8_lossy(&output.stdout).to_string()))
        } else {
            Ok(None)
        }
    }

    #[cfg(target_os = "macos")]
    async fn configure_dns(&self, dns_servers: &[IpAddr]) -> Result<()> {
        use tokio::process::Command;

        // Get list of network services
        let output = Command::new("networksetup")
            .args(["-listallnetworkservices"])
            .output()
            .await
            .map_err(|e| AppError::Dns(format!("Failed to list network services: {}", e)))?;

        if !output.status.success() {
            return Err(AppError::Dns("Failed to list network services".to_string()));
        }

        let services = String::from_utf8_lossy(&output.stdout);
        let dns_str = dns_servers
            .iter()
            .map(|ip| ip.to_string())
            .collect::<Vec<_>>()
            .join(" ");

        // Set DNS for common services (Wi-Fi and Ethernet)
        for service in ["Wi-Fi", "Ethernet", "USB 10/100/1000 LAN"] {
            if services.contains(service) {
                let status = Command::new("networksetup")
                    .args(["-setdnsservers", service, &dns_str])
                    .status()
                    .await;

                match status {
                    Ok(s) if s.success() => {
                        tracing::debug!("Set DNS for service: {}", service);
                    }
                    Ok(_) => {
                        tracing::debug!("Failed to set DNS for service: {}", service);
                    }
                    Err(e) => {
                        tracing::debug!("Error setting DNS for {}: {}", service, e);
                    }
                }
            }
        }

        // Also use scutil to create a resolver configuration
        let resolver_config = format!(
            r#"d.init
d.add ServerAddresses * {}
d.add InterfaceName {}
set State:/Network/Service/LCARS_VPN/DNS
"#,
            dns_servers
                .iter()
                .map(|ip| ip.to_string())
                .collect::<Vec<_>>()
                .join(" "),
            self.interface_name
        );

        let mut child = Command::new("scutil")
            .stdin(std::process::Stdio::piped())
            .spawn()
            .map_err(|e| AppError::Dns(format!("Failed to spawn scutil: {}", e)))?;

        if let Some(stdin) = child.stdin.as_mut() {
            use tokio::io::AsyncWriteExt;
            stdin
                .write_all(resolver_config.as_bytes())
                .await
                .map_err(|e| AppError::Dns(format!("Failed to write to scutil: {}", e)))?;
        }

        child
            .wait()
            .await
            .map_err(|e| AppError::Dns(format!("scutil command failed: {}", e)))?;

        Ok(())
    }

    #[cfg(target_os = "macos")]
    async fn restore_dns_impl(&self) -> Result<()> {
        use tokio::process::Command;

        // Remove our resolver configuration
        let remove_config = "remove State:/Network/Service/LCARS_VPN/DNS\n";

        let mut child = Command::new("scutil")
            .stdin(std::process::Stdio::piped())
            .spawn()
            .map_err(|e| AppError::Dns(format!("Failed to spawn scutil: {}", e)))?;

        if let Some(stdin) = child.stdin.as_mut() {
            use tokio::io::AsyncWriteExt;
            stdin
                .write_all(remove_config.as_bytes())
                .await
                .map_err(|e| AppError::Dns(format!("Failed to write to scutil: {}", e)))?;
        }

        child
            .wait()
            .await
            .map_err(|e| AppError::Dns(format!("scutil command failed: {}", e)))?;

        // Reset DNS to "empty" (DHCP) for common services
        let output = Command::new("networksetup")
            .args(["-listallnetworkservices"])
            .output()
            .await
            .map_err(|e| AppError::Dns(format!("Failed to list network services: {}", e)))?;

        if output.status.success() {
            let services = String::from_utf8_lossy(&output.stdout);
            for service in ["Wi-Fi", "Ethernet", "USB 10/100/1000 LAN"] {
                if services.contains(service) {
                    // Setting to "empty" restores DHCP DNS
                    let _ = Command::new("networksetup")
                        .args(["-setdnsservers", service, "empty"])
                        .status()
                        .await;
                }
            }
        }

        Ok(())
    }

    // Fallback for unsupported platforms
    #[cfg(not(any(target_os = "linux", target_os = "macos")))]
    async fn backup_dns(&self) -> Result<Option<String>> {
        tracing::warn!("DNS management not supported on this platform");
        Ok(None)
    }

    #[cfg(not(any(target_os = "linux", target_os = "macos")))]
    async fn configure_dns(&self, _dns_servers: &[IpAddr]) -> Result<()> {
        tracing::warn!("DNS management not supported on this platform");
        Ok(())
    }

    #[cfg(not(any(target_os = "linux", target_os = "macos")))]
    async fn restore_dns_impl(&self) -> Result<()> {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dns_state_default() {
        let state = DnsState::default();
        assert!(!state.is_managed);
        assert!(state.original_servers.is_empty());
        assert!(state.backup_data.is_none());
    }

    #[tokio::test]
    async fn test_dns_manager_not_managed_initially() {
        let manager = DnsManager::new("wg0");
        assert!(!manager.is_managed().await);
    }

    #[tokio::test]
    async fn test_set_vpn_dns_empty_servers() {
        let manager = DnsManager::new("wg0");
        // Should succeed but not actually configure anything
        let result = manager.set_vpn_dns(&[]).await;
        assert!(result.is_ok());
        assert!(!manager.is_managed().await);
    }
}
