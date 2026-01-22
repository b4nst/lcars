//! Configuration module for the LCARS backend.
//!
//! Loads configuration from `config.toml` with environment variable overrides.

use config::{Config as ConfigLoader, Environment, File};
use serde::Deserialize;
use std::path::PathBuf;

use crate::error::AppError;

/// Main application configuration
#[derive(Debug, Clone, Deserialize)]
pub struct Config {
    #[serde(default)]
    pub server: ServerConfig,
    #[serde(default)]
    pub database: DatabaseConfig,
    #[serde(default)]
    pub tmdb: TmdbConfig,
    #[serde(default)]
    pub musicbrainz: MusicBrainzConfig,
    #[serde(default)]
    pub torrent: TorrentConfig,
    #[serde(default)]
    pub soulseek: SoulseekConfig,
    #[serde(default)]
    pub storage: StorageConfig,
    #[serde(default)]
    pub scheduler: SchedulerConfig,
    #[serde(default)]
    pub music: MusicConfig,
    #[serde(default)]
    pub wireguard: Option<WireGuardConfig>,
}

/// Server configuration
#[derive(Clone, Deserialize)]
pub struct ServerConfig {
    #[serde(default = "default_host")]
    pub host: String,
    #[serde(default = "default_port")]
    pub port: u16,
    pub jwt_secret: Option<String>,
    /// Enable secure cookies (should be true in production with HTTPS)
    #[serde(default)]
    pub secure_cookies: bool,
    /// Allowed CORS origins (empty means same-origin only)
    #[serde(default)]
    pub cors_origins: Vec<String>,
}

// Custom Debug implementation to avoid exposing jwt_secret
impl std::fmt::Debug for ServerConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ServerConfig")
            .field("host", &self.host)
            .field("port", &self.port)
            .field(
                "jwt_secret",
                &self.jwt_secret.as_ref().map(|_| "[REDACTED]"),
            )
            .finish()
    }
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            host: default_host(),
            port: default_port(),
            jwt_secret: None,
            secure_cookies: false,
            cors_origins: Vec::new(),
        }
    }
}

fn default_host() -> String {
    "0.0.0.0".to_string()
}

fn default_port() -> u16 {
    8080
}

/// Database configuration
#[derive(Debug, Clone, Deserialize)]
pub struct DatabaseConfig {
    #[serde(default = "default_db_path")]
    pub path: PathBuf,
}

impl Default for DatabaseConfig {
    fn default() -> Self {
        Self {
            path: default_db_path(),
        }
    }
}

fn default_db_path() -> PathBuf {
    PathBuf::from("./data/lcars.db")
}

/// TMDB API configuration
#[derive(Clone, Deserialize, Default)]
pub struct TmdbConfig {
    pub api_key: Option<String>,
}

// Custom Debug implementation to avoid exposing api_key
impl std::fmt::Debug for TmdbConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TmdbConfig")
            .field("api_key", &self.api_key.as_ref().map(|_| "[REDACTED]"))
            .finish()
    }
}

/// MusicBrainz configuration
#[derive(Debug, Clone, Deserialize)]
pub struct MusicBrainzConfig {
    #[serde(default = "default_rate_limit")]
    pub rate_limit_ms: u64,
}

impl Default for MusicBrainzConfig {
    fn default() -> Self {
        Self {
            rate_limit_ms: default_rate_limit(),
        }
    }
}

fn default_rate_limit() -> u64 {
    1000 // MusicBrainz requires max 1 request/second
}

/// Torrent client configuration
#[derive(Debug, Clone, Deserialize)]
pub struct TorrentConfig {
    #[serde(default = "default_download_dir")]
    pub download_dir: PathBuf,
    #[serde(default)]
    pub bind_interface: String,
    #[serde(default = "default_max_connections")]
    pub max_connections: u32,
    #[serde(default = "default_port_range")]
    pub port_range: (u16, u16),
    #[serde(default)]
    pub seeding: SeedingConfig,
}

impl Default for TorrentConfig {
    fn default() -> Self {
        Self {
            download_dir: default_download_dir(),
            bind_interface: String::new(),
            max_connections: default_max_connections(),
            port_range: default_port_range(),
            seeding: SeedingConfig::default(),
        }
    }
}

fn default_download_dir() -> PathBuf {
    PathBuf::from("./downloads")
}

fn default_max_connections() -> u32 {
    100
}

fn default_port_range() -> (u16, u16) {
    (6881, 6889)
}

/// Seeding configuration
#[derive(Debug, Clone, Deserialize)]
pub struct SeedingConfig {
    #[serde(default = "default_seeding_enabled")]
    pub enabled: bool,
    #[serde(default = "default_ratio_limit")]
    pub ratio_limit: f64,
    #[serde(default = "default_time_limit")]
    pub time_limit_hours: u64,
}

impl Default for SeedingConfig {
    fn default() -> Self {
        Self {
            enabled: default_seeding_enabled(),
            ratio_limit: default_ratio_limit(),
            time_limit_hours: default_time_limit(),
        }
    }
}

fn default_seeding_enabled() -> bool {
    true
}

fn default_ratio_limit() -> f64 {
    1.0
}

fn default_time_limit() -> u64 {
    48
}

/// Soulseek client configuration
#[derive(Clone, Deserialize)]
pub struct SoulseekConfig {
    pub username: Option<String>,
    pub password: Option<String>,
    #[serde(default = "default_soulseek_download_dir")]
    pub download_dir: PathBuf,
    #[serde(default = "default_listen_port")]
    pub listen_port: u16,
    #[serde(default = "default_server_host")]
    pub server_host: String,
    #[serde(default = "default_server_port")]
    pub server_port: u16,
    #[serde(default = "default_max_concurrent_downloads")]
    pub max_concurrent_downloads: usize,
    // Sharing configuration
    #[serde(default)]
    pub share_dirs: Vec<PathBuf>,
    #[serde(default)]
    pub share_hidden: bool,
    #[serde(default = "default_upload_slots")]
    pub upload_slots: u32,
    #[serde(default)]
    pub upload_speed_limit: Option<u64>,
    #[serde(default)]
    pub sharing_enabled: bool,
    // Connection management
    #[serde(default = "default_auto_reconnect")]
    pub auto_reconnect: bool,
    #[serde(default = "default_connect_timeout")]
    pub connect_timeout: u64,
    #[serde(default)]
    pub max_reconnect_attempts: Option<u32>,
    #[serde(default = "default_reconnect_delay_max")]
    pub reconnect_delay_max: u64,
    #[serde(default = "default_keepalive_interval")]
    pub keepalive_interval: u64,
}

// Custom Debug implementation to avoid exposing password
impl std::fmt::Debug for SoulseekConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SoulseekConfig")
            .field("username", &self.username)
            .field("password", &self.password.as_ref().map(|_| "[REDACTED]"))
            .field("download_dir", &self.download_dir)
            .field("listen_port", &self.listen_port)
            .field("server_host", &self.server_host)
            .field("server_port", &self.server_port)
            .field("max_concurrent_downloads", &self.max_concurrent_downloads)
            .field("share_dirs", &self.share_dirs)
            .field("share_hidden", &self.share_hidden)
            .field("upload_slots", &self.upload_slots)
            .field("upload_speed_limit", &self.upload_speed_limit)
            .field("sharing_enabled", &self.sharing_enabled)
            .field("auto_reconnect", &self.auto_reconnect)
            .field("connect_timeout", &self.connect_timeout)
            .field("max_reconnect_attempts", &self.max_reconnect_attempts)
            .field("reconnect_delay_max", &self.reconnect_delay_max)
            .field("keepalive_interval", &self.keepalive_interval)
            .finish()
    }
}

impl Default for SoulseekConfig {
    fn default() -> Self {
        Self {
            username: None,
            password: None,
            download_dir: default_soulseek_download_dir(),
            listen_port: default_listen_port(),
            server_host: default_server_host(),
            server_port: default_server_port(),
            max_concurrent_downloads: default_max_concurrent_downloads(),
            share_dirs: Vec::new(),
            share_hidden: false,
            upload_slots: default_upload_slots(),
            upload_speed_limit: None,
            sharing_enabled: false,
            auto_reconnect: default_auto_reconnect(),
            connect_timeout: default_connect_timeout(),
            max_reconnect_attempts: None,
            reconnect_delay_max: default_reconnect_delay_max(),
            keepalive_interval: default_keepalive_interval(),
        }
    }
}

fn default_soulseek_download_dir() -> PathBuf {
    PathBuf::from("./downloads/soulseek")
}

fn default_listen_port() -> u16 {
    2234
}

fn default_server_host() -> String {
    "server.slsknet.org".to_string()
}

fn default_server_port() -> u16 {
    2242
}

fn default_max_concurrent_downloads() -> usize {
    5
}

fn default_upload_slots() -> u32 {
    5
}

fn default_auto_reconnect() -> bool {
    true
}

fn default_connect_timeout() -> u64 {
    30
}

fn default_reconnect_delay_max() -> u64 {
    60
}

fn default_keepalive_interval() -> u64 {
    60
}

/// WireGuard VPN configuration
#[derive(Debug, Clone, Deserialize)]
pub struct WireGuardConfig {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default)]
    pub interface_name: Option<String>,
    #[serde(default)]
    pub config_file: Option<PathBuf>,
    #[serde(default)]
    pub inline: Option<WireGuardInlineConfig>,
    #[serde(default = "default_health_check_interval_secs")]
    pub health_check_interval_secs: u64,
    #[serde(default = "default_wg_auto_reconnect")]
    pub auto_reconnect: bool,
    #[serde(default = "default_reconnect_delay_max_secs")]
    pub reconnect_delay_max_secs: u64,
    #[serde(default = "default_kill_switch")]
    pub kill_switch: bool,
}

impl Default for WireGuardConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            interface_name: None,
            config_file: None,
            inline: None,
            health_check_interval_secs: default_health_check_interval_secs(),
            auto_reconnect: default_wg_auto_reconnect(),
            reconnect_delay_max_secs: default_reconnect_delay_max_secs(),
            kill_switch: default_kill_switch(),
        }
    }
}

fn default_health_check_interval_secs() -> u64 {
    30
}

fn default_wg_auto_reconnect() -> bool {
    true
}

fn default_reconnect_delay_max_secs() -> u64 {
    300
}

fn default_kill_switch() -> bool {
    true
}

/// WireGuard inline configuration (alternative to config_file)
#[derive(Clone, Deserialize)]
pub struct WireGuardInlineConfig {
    pub private_key: String,
    pub addresses: Vec<String>,
    #[serde(default)]
    pub listen_port: Option<u16>,
    #[serde(default)]
    pub dns: Option<Vec<String>>,
    #[serde(default)]
    pub mtu: Option<u16>,
    pub peer: WireGuardPeerConfig,
}

// Custom Debug implementation to avoid exposing private_key
impl std::fmt::Debug for WireGuardInlineConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("WireGuardInlineConfig")
            .field("private_key", &"[REDACTED]")
            .field("addresses", &self.addresses)
            .field("listen_port", &self.listen_port)
            .field("dns", &self.dns)
            .field("mtu", &self.mtu)
            .field("peer", &self.peer)
            .finish()
    }
}

/// WireGuard peer configuration
#[derive(Clone, Deserialize)]
pub struct WireGuardPeerConfig {
    pub public_key: String,
    #[serde(default)]
    pub preshared_key: Option<String>,
    pub endpoint: String,
    pub allowed_ips: Vec<String>,
    #[serde(default = "default_persistent_keepalive")]
    pub persistent_keepalive: u16,
}

// Custom Debug implementation to avoid exposing preshared_key
impl std::fmt::Debug for WireGuardPeerConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("WireGuardPeerConfig")
            .field("public_key", &self.public_key)
            .field(
                "preshared_key",
                &self.preshared_key.as_ref().map(|_| "[REDACTED]"),
            )
            .field("endpoint", &self.endpoint)
            .field("allowed_ips", &self.allowed_ips)
            .field("persistent_keepalive", &self.persistent_keepalive)
            .finish()
    }
}

fn default_persistent_keepalive() -> u16 {
    25
}

/// Storage configuration
#[derive(Debug, Clone, Deserialize, Default)]
pub struct StorageConfig {
    #[serde(default)]
    pub mounts: Vec<MountConfig>,
    #[serde(default)]
    pub naming: NamingConfig,
    #[serde(default)]
    pub rules: Vec<StorageRule>,
}

/// Mount point configuration
#[derive(Clone, Deserialize)]
pub struct MountConfig {
    pub name: String,
    #[serde(rename = "type")]
    pub mount_type: MountType,
    #[serde(default)]
    pub path: Option<PathBuf>,
    #[serde(default)]
    pub host: Option<String>,
    #[serde(default)]
    pub share: Option<String>,
    #[serde(default)]
    pub username: Option<String>,
    #[serde(default)]
    pub password: Option<String>,
    #[serde(default)]
    pub mount_point: Option<PathBuf>,
    #[serde(default = "default_enabled")]
    pub enabled: bool,
}

// Custom Debug implementation to avoid exposing password
impl std::fmt::Debug for MountConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MountConfig")
            .field("name", &self.name)
            .field("mount_type", &self.mount_type)
            .field("path", &self.path)
            .field("host", &self.host)
            .field("share", &self.share)
            .field("username", &self.username)
            .field("password", &self.password.as_ref().map(|_| "[REDACTED]"))
            .field("mount_point", &self.mount_point)
            .field("enabled", &self.enabled)
            .finish()
    }
}

fn default_enabled() -> bool {
    true
}

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum MountType {
    Local,
    Smb,
}

/// File naming patterns
#[derive(Debug, Clone, Deserialize)]
pub struct NamingConfig {
    #[serde(default = "default_movie_pattern")]
    pub movie_pattern: String,
    #[serde(default = "default_tv_pattern")]
    pub tv_pattern: String,
    #[serde(default = "default_music_pattern")]
    pub music_pattern: String,
}

impl Default for NamingConfig {
    fn default() -> Self {
        Self {
            movie_pattern: default_movie_pattern(),
            tv_pattern: default_tv_pattern(),
            music_pattern: default_music_pattern(),
        }
    }
}

fn default_movie_pattern() -> String {
    "movie/{title} ({year})/{title} ({year}) - {quality}.{ext}".to_string()
}

fn default_tv_pattern() -> String {
    "tv/{title}/S{season:02}/{title} - S{season:02}E{episode:02} - {episode_title}.{ext}"
        .to_string()
}

fn default_music_pattern() -> String {
    "music/{artist}/{album}/{title}.{ext}".to_string()
}

/// Storage rule for post-download processing
#[derive(Debug, Clone, Deserialize)]
pub struct StorageRule {
    pub action: StorageAction,
    pub destination: String,
    #[serde(default)]
    pub media_types: Vec<String>,
}

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum StorageAction {
    Move,
    Copy,
}

/// Scheduler configuration with cron expressions
#[derive(Debug, Clone, Deserialize)]
pub struct SchedulerConfig {
    #[serde(default = "default_search_missing")]
    pub search_missing: String,
    #[serde(default = "default_refresh_metadata")]
    pub refresh_metadata: String,
    #[serde(default = "default_check_new_episodes")]
    pub check_new_episodes: String,
    #[serde(default = "default_check_new_releases")]
    pub check_new_releases: String,
    #[serde(default = "default_cleanup_completed")]
    pub cleanup_completed: String,
}

impl Default for SchedulerConfig {
    fn default() -> Self {
        Self {
            search_missing: default_search_missing(),
            refresh_metadata: default_refresh_metadata(),
            check_new_episodes: default_check_new_episodes(),
            check_new_releases: default_check_new_releases(),
            cleanup_completed: default_cleanup_completed(),
        }
    }
}

fn default_search_missing() -> String {
    "0 0 */6 * * *".to_string()
}

fn default_refresh_metadata() -> String {
    "0 0 2 * * *".to_string()
}

fn default_check_new_episodes() -> String {
    "0 0 */12 * * *".to_string()
}

fn default_check_new_releases() -> String {
    "0 0 3 * * *".to_string()
}

fn default_cleanup_completed() -> String {
    "0 0 * * * *".to_string()
}

/// Music acquisition configuration
#[derive(Debug, Clone, Deserialize)]
pub struct MusicConfig {
    /// Search source order preference (options: "indexers", "soulseek")
    #[serde(default = "default_search_sources")]
    pub search_sources: Vec<String>,
    /// Auto-download source preference ("indexers", "soulseek", or "any")
    #[serde(default = "default_auto_download_source")]
    pub auto_download_source: String,
    /// Quality preferences for search result matching
    #[serde(default)]
    pub quality: MusicQualityConfig,
}

impl Default for MusicConfig {
    fn default() -> Self {
        Self {
            search_sources: default_search_sources(),
            auto_download_source: default_auto_download_source(),
            quality: MusicQualityConfig::default(),
        }
    }
}

fn default_search_sources() -> Vec<String> {
    vec!["indexers".to_string(), "soulseek".to_string()]
}

fn default_auto_download_source() -> String {
    "indexers".to_string()
}

/// Quality preferences for music downloads
#[derive(Debug, Clone, Deserialize)]
pub struct MusicQualityConfig {
    /// Preferred audio formats in priority order (e.g., ["flac", "mp3 320"])
    #[serde(default = "default_preferred_formats")]
    pub preferred_formats: Vec<String>,
    /// Minimum acceptable bitrate in kbps
    #[serde(default = "default_min_bitrate")]
    pub min_bitrate: u32,
    /// Prefer complete albums over partial downloads
    #[serde(default = "default_prefer_complete_albums")]
    pub prefer_complete_albums: bool,
}

impl Default for MusicQualityConfig {
    fn default() -> Self {
        Self {
            preferred_formats: default_preferred_formats(),
            min_bitrate: default_min_bitrate(),
            prefer_complete_albums: default_prefer_complete_albums(),
        }
    }
}

fn default_preferred_formats() -> Vec<String> {
    vec!["flac".to_string(), "mp3".to_string()]
}

fn default_min_bitrate() -> u32 {
    256
}

fn default_prefer_complete_albums() -> bool {
    true
}

impl Config {
    /// Load configuration from file and environment variables.
    ///
    /// Configuration is loaded in the following order (later sources override earlier):
    /// 1. Default values
    /// 2. `config.toml` in current directory (optional)
    /// 3. Environment variables with `LCARS_` prefix
    ///
    /// Environment variables use double underscore for nesting:
    /// - `LCARS_SERVER__PORT=9000` sets `server.port`
    /// - `LCARS_DATABASE__PATH=/data/db.sqlite` sets `database.path`
    pub fn load() -> Result<Self, AppError> {
        Self::load_from("config.toml")
    }

    /// Load configuration from a specific file path.
    pub fn load_from(config_path: &str) -> Result<Self, AppError> {
        let config = ConfigLoader::builder()
            // Start with defaults
            .set_default("server.host", "0.0.0.0")?
            .set_default("server.port", 8080)?
            .set_default("database.path", "./data/lcars.db")?
            .set_default("musicbrainz.rate_limit_ms", 1000)?
            .set_default("torrent.download_dir", "./downloads")?
            .set_default("torrent.max_connections", 100)?
            .set_default("torrent.seeding.enabled", true)?
            .set_default("torrent.seeding.ratio_limit", 1.0)?
            .set_default("torrent.seeding.time_limit_hours", 48)?
            // Add config file (optional)
            .add_source(File::with_name(config_path).required(false))
            // Override with environment variables
            // LCARS_SERVER__PORT=9000 -> server.port = 9000
            .add_source(
                Environment::with_prefix("LCARS")
                    .separator("__")
                    .try_parsing(true),
            )
            .build()?;

        let config: Config = config.try_deserialize()?;

        // Validate required fields for certain operations
        config.validate()?;

        Ok(config)
    }

    /// Validate configuration for required fields.
    fn validate(&self) -> Result<(), AppError> {
        // JWT secret is required in production but we don't fail here
        // since it might be set later or not needed for all operations
        if self.server.jwt_secret.is_none() {
            tracing::warn!("JWT secret not configured - authentication will not work");
        }

        if self.tmdb.api_key.is_none() {
            tracing::warn!("TMDB API key not configured - movie/TV metadata lookups will fail");
        }

        Ok(())
    }

    /// Get the server socket address
    pub fn server_addr(&self) -> std::net::SocketAddr {
        use std::net::{IpAddr, SocketAddr};
        let ip: IpAddr = self.server.host.parse().unwrap_or_else(|_| {
            tracing::warn!("Invalid host '{}', using 0.0.0.0", self.server.host);
            "0.0.0.0".parse().unwrap()
        });
        SocketAddr::new(ip, self.server.port)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = Config::load_from("nonexistent.toml").unwrap();
        assert_eq!(config.server.port, 8080);
        assert_eq!(config.server.host, "0.0.0.0");
        assert_eq!(config.database.path, PathBuf::from("./data/lcars.db"));
        assert_eq!(config.musicbrainz.rate_limit_ms, 1000);
    }

    #[test]
    fn test_server_addr() {
        let config = Config::load_from("nonexistent.toml").unwrap();
        let addr = config.server_addr();
        assert_eq!(addr.port(), 8080);
    }

    #[test]
    fn test_torrent_defaults() {
        let config = Config::load_from("nonexistent.toml").unwrap();
        assert_eq!(config.torrent.max_connections, 100);
        assert!(config.torrent.seeding.enabled);
        assert_eq!(config.torrent.seeding.ratio_limit, 1.0);
        assert_eq!(config.torrent.seeding.time_limit_hours, 48);
    }
}
