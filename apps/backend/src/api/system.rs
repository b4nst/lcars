//! System API endpoints for job management, system status, indexers, and storage.

use axum::{
    extract::{Path, Query, State},
    response::Json,
};
use serde::{Deserialize, Serialize};

use crate::db::models::{Activity, Indexer};
use crate::error::{AppError, Result};
use crate::services::scheduler::{
    run_check_new_episodes_job, run_check_new_releases_job, run_cleanup_completed_job,
    run_refresh_metadata_job, run_search_missing_job,
};
use crate::AppState;

// =============================================================================
// Response Types
// =============================================================================

/// Response for successful job trigger.
#[derive(Serialize)]
pub struct JobTriggerResponse {
    pub success: bool,
    pub job: String,
    pub message: String,
}

/// Job information for listing.
#[derive(Serialize)]
pub struct JobInfo {
    pub name: String,
    pub description: String,
}

/// System status response.
#[derive(Debug, Serialize)]
pub struct SystemStatus {
    pub version: String,
    pub uptime_seconds: u64,
    pub database_size_bytes: u64,
    pub downloads: DownloadStats,
    pub vpn: VpnStatus,
}

/// Download statistics.
#[derive(Debug, Serialize)]
pub struct DownloadStats {
    pub active: i64,
    pub queued: i64,
    pub completed: i64,
    pub failed: i64,
    pub total_download_speed: u64,
    pub total_upload_speed: u64,
}

/// VPN connection status.
#[derive(Debug, Serialize)]
pub struct VpnStatus {
    pub connected: bool,
    pub interface: Option<String>,
}

/// Query parameters for activity listing.
#[derive(Debug, Deserialize)]
pub struct ActivityQuery {
    pub event_type: Option<String>,
    pub limit: Option<i64>,
    pub before: Option<String>,
}

/// Indexer response (masks API key).
#[derive(Debug, Serialize)]
pub struct IndexerResponse {
    pub id: i64,
    pub name: String,
    pub indexer_type: String,
    pub url: String,
    pub has_api_key: bool,
    pub enabled: bool,
    pub priority: i32,
    pub categories: Option<String>,
    pub last_check: Option<String>,
    pub last_error: Option<String>,
    pub created_at: String,
}

impl From<Indexer> for IndexerResponse {
    fn from(i: Indexer) -> Self {
        Self {
            id: i.id,
            name: i.name,
            indexer_type: i.indexer_type,
            url: i.url,
            has_api_key: i.api_key.is_some(),
            enabled: i.enabled,
            priority: i.priority,
            categories: i.categories,
            last_check: i.last_check,
            last_error: i.last_error,
            created_at: i.created_at,
        }
    }
}

/// Request to create an indexer.
#[derive(Debug, Deserialize)]
pub struct CreateIndexerRequest {
    pub name: String,
    pub indexer_type: String,
    pub url: String,
    pub api_key: Option<String>,
    pub enabled: Option<bool>,
    pub priority: Option<i32>,
    pub categories: Option<String>,
}

impl CreateIndexerRequest {
    /// Validate the create indexer request.
    fn validate(&self) -> Result<()> {
        // Name validation
        if self.name.is_empty() || self.name.len() > 100 {
            return Err(AppError::BadRequest(
                "Name must be 1-100 characters".to_string(),
            ));
        }

        // Type validation
        const VALID_TYPES: &[&str] = &["public", "private", "torznab", "newznab"];
        if !VALID_TYPES.contains(&self.indexer_type.as_str()) {
            return Err(AppError::BadRequest(format!(
                "Invalid indexer type. Must be one of: {}",
                VALID_TYPES.join(", ")
            )));
        }

        // URL validation
        reqwest::Url::parse(&self.url)
            .map_err(|_| AppError::BadRequest("Invalid URL format".to_string()))?;

        // Priority bounds
        if let Some(priority) = self.priority {
            if !(0..=100).contains(&priority) {
                return Err(AppError::BadRequest(
                    "Priority must be between 0 and 100".to_string(),
                ));
            }
        }

        Ok(())
    }
}

/// Request to update an indexer.
#[derive(Debug, Deserialize)]
pub struct UpdateIndexerRequest {
    pub name: Option<String>,
    pub indexer_type: Option<String>,
    pub url: Option<String>,
    pub api_key: Option<String>,
    pub enabled: Option<bool>,
    pub priority: Option<i32>,
    pub categories: Option<String>,
}

/// Response from testing an indexer.
#[derive(Debug, Serialize)]
pub struct IndexerTestResponse {
    pub success: bool,
    pub response_time_ms: Option<u64>,
    pub error: Option<String>,
}

/// Mount information response.
#[derive(Debug, Serialize)]
pub struct MountInfo {
    pub name: String,
    pub path: String,
    pub available: bool,
    pub free_bytes: Option<u64>,
    pub total_bytes: Option<u64>,
}

/// Response from testing a mount.
#[derive(Debug, Serialize)]
pub struct MountTestResponse {
    pub success: bool,
    pub available: bool,
    pub free_bytes: Option<u64>,
    pub error: Option<String>,
}

/// Success response for operations without specific data.
#[derive(Debug, Serialize)]
pub struct SuccessResponse {
    pub success: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
}

/// Available job names.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum JobName {
    SearchMissing,
    RefreshMetadata,
    CheckNewEpisodes,
    CheckNewReleases,
    CleanupCompleted,
}

impl std::fmt::Display for JobName {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            JobName::SearchMissing => write!(f, "search_missing"),
            JobName::RefreshMetadata => write!(f, "refresh_metadata"),
            JobName::CheckNewEpisodes => write!(f, "check_new_episodes"),
            JobName::CheckNewReleases => write!(f, "check_new_releases"),
            JobName::CleanupCompleted => write!(f, "cleanup_completed"),
        }
    }
}

// =============================================================================
// Job Handlers
// =============================================================================

/// GET /api/system/jobs
///
/// List all available background jobs.
pub async fn list_jobs() -> Json<Vec<JobInfo>> {
    Json(vec![
        JobInfo {
            name: "search_missing".to_string(),
            description: "Search indexers for missing media and queue downloads".to_string(),
        },
        JobInfo {
            name: "refresh_metadata".to_string(),
            description: "Refresh metadata from TMDB and MusicBrainz".to_string(),
        },
        JobInfo {
            name: "check_new_episodes".to_string(),
            description: "Check for new episodes of continuing TV shows".to_string(),
        },
        JobInfo {
            name: "check_new_releases".to_string(),
            description: "Check for new album releases from monitored artists".to_string(),
        },
        JobInfo {
            name: "cleanup_completed".to_string(),
            description: "Clean up torrents that have met seeding requirements".to_string(),
        },
    ])
}

/// POST /api/system/jobs/:name/run
///
/// Manually trigger a background job.
pub async fn trigger_job(
    State(state): State<AppState>,
    Path(job_name): Path<String>,
) -> Result<Json<JobTriggerResponse>> {
    let ctx = state.job_context();

    let job = match job_name.as_str() {
        "search_missing" => {
            tokio::spawn(async move {
                run_search_missing_job(&ctx).await;
            });
            JobName::SearchMissing
        }
        "refresh_metadata" => {
            tokio::spawn(async move {
                run_refresh_metadata_job(&ctx).await;
            });
            JobName::RefreshMetadata
        }
        "check_new_episodes" => {
            tokio::spawn(async move {
                run_check_new_episodes_job(&ctx).await;
            });
            JobName::CheckNewEpisodes
        }
        "check_new_releases" => {
            tokio::spawn(async move {
                run_check_new_releases_job(&ctx).await;
            });
            JobName::CheckNewReleases
        }
        "cleanup_completed" => {
            tokio::spawn(async move {
                run_cleanup_completed_job(&ctx).await;
            });
            JobName::CleanupCompleted
        }
        _ => {
            return Err(AppError::NotFound(format!("Job '{}' not found", job_name)));
        }
    };

    tracing::info!(job = %job, "Manually triggered job");

    Ok(Json(JobTriggerResponse {
        success: true,
        job: job.to_string(),
        message: format!("Job '{}' has been triggered", job),
    }))
}

// =============================================================================
// System Status Handler
// =============================================================================

/// GET /api/system/status
///
/// Get system status including version, uptime, and download stats.
pub async fn get_system_status(State(state): State<AppState>) -> Result<Json<SystemStatus>> {
    let uptime = state.start_time().elapsed().as_secs();

    // Get database size
    let database_size_bytes = {
        let db_path = &state.config.database.path;
        std::fs::metadata(db_path).map(|m| m.len()).unwrap_or(0)
    };

    // Get download statistics
    let downloads = {
        let db = state.db.lock().await;

        let active: i64 = db
            .query_row(
                "SELECT COUNT(*) FROM downloads WHERE status = 'downloading'",
                [],
                |row| row.get(0),
            )
            .unwrap_or(0);

        let queued: i64 = db
            .query_row(
                "SELECT COUNT(*) FROM downloads WHERE status = 'queued'",
                [],
                |row| row.get(0),
            )
            .unwrap_or(0);

        let completed: i64 = db
            .query_row(
                "SELECT COUNT(*) FROM downloads WHERE status = 'completed'",
                [],
                |row| row.get(0),
            )
            .unwrap_or(0);

        let failed: i64 = db
            .query_row(
                "SELECT COUNT(*) FROM downloads WHERE status = 'failed'",
                [],
                |row| row.get(0),
            )
            .unwrap_or(0);

        drop(db);

        // Get live speed stats from torrent engine
        let (total_download_speed, total_upload_speed) =
            if let Some(torrent_engine) = state.torrent_engine() {
                let stats = torrent_engine.list_all().await;
                let dl: u64 = stats.iter().map(|s| s.download_speed).sum();
                let ul: u64 = stats.iter().map(|s| s.upload_speed).sum();
                (dl, ul)
            } else {
                (0, 0)
            };

        DownloadStats {
            active,
            queued,
            completed,
            failed,
            total_download_speed,
            total_upload_speed,
        }
    };

    // Check VPN status
    let vpn = check_vpn_status();

    Ok(Json(SystemStatus {
        version: env!("CARGO_PKG_VERSION").to_string(),
        uptime_seconds: uptime,
        database_size_bytes,
        downloads,
        vpn,
    }))
}

/// Check if a VPN interface exists.
fn check_vpn_status() -> VpnStatus {
    // Common VPN interface names
    let vpn_interfaces = ["tun0", "wg0", "wg1", "utun0", "utun1", "utun2", "ppp0"];

    for iface in vpn_interfaces {
        if check_interface_exists(iface) {
            return VpnStatus {
                connected: true,
                interface: Some(iface.to_string()),
            };
        }
    }

    VpnStatus {
        connected: false,
        interface: None,
    }
}

/// Check if a network interface exists.
#[cfg(target_os = "linux")]
fn check_interface_exists(name: &str) -> bool {
    std::path::Path::new(&format!("/sys/class/net/{}", name)).exists()
}

#[cfg(target_os = "macos")]
fn check_interface_exists(name: &str) -> bool {
    std::process::Command::new("ifconfig")
        .arg(name)
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

#[cfg(not(any(target_os = "linux", target_os = "macos")))]
fn check_interface_exists(_name: &str) -> bool {
    false
}

// =============================================================================
// Activity Handler
// =============================================================================

/// GET /api/system/activity
///
/// Get recent activity log entries with optional filtering.
pub async fn get_activity(
    State(state): State<AppState>,
    Query(query): Query<ActivityQuery>,
) -> Result<Json<Vec<Activity>>> {
    let limit = query.limit.unwrap_or(50).min(500);
    let db = state.db.lock().await;

    let activities: Vec<Activity> = match (&query.event_type, &query.before) {
        (Some(event_type), Some(before)) => {
            let mut stmt = db.prepare(
                r#"
                SELECT id, event_type, message, media_type, media_id, download_id, user_id, metadata, created_at
                FROM activity
                WHERE event_type = ?1 AND created_at < ?2
                ORDER BY created_at DESC
                LIMIT ?3
                "#,
            )?;
            let result = stmt
                .query_map(
                    rusqlite::params![event_type, before, limit],
                    map_activity_row,
                )?
                .collect::<std::result::Result<Vec<_>, _>>()?;
            result
        }
        (Some(event_type), None) => {
            let mut stmt = db.prepare(
                r#"
                SELECT id, event_type, message, media_type, media_id, download_id, user_id, metadata, created_at
                FROM activity
                WHERE event_type = ?1
                ORDER BY created_at DESC
                LIMIT ?2
                "#,
            )?;
            let result = stmt
                .query_map(rusqlite::params![event_type, limit], map_activity_row)?
                .collect::<std::result::Result<Vec<_>, _>>()?;
            result
        }
        (None, Some(before)) => {
            let mut stmt = db.prepare(
                r#"
                SELECT id, event_type, message, media_type, media_id, download_id, user_id, metadata, created_at
                FROM activity
                WHERE created_at < ?1
                ORDER BY created_at DESC
                LIMIT ?2
                "#,
            )?;
            let result = stmt
                .query_map(rusqlite::params![before, limit], map_activity_row)?
                .collect::<std::result::Result<Vec<_>, _>>()?;
            result
        }
        (None, None) => {
            let mut stmt = db.prepare(
                r#"
                SELECT id, event_type, message, media_type, media_id, download_id, user_id, metadata, created_at
                FROM activity
                ORDER BY created_at DESC
                LIMIT ?1
                "#,
            )?;
            let result = stmt
                .query_map(rusqlite::params![limit], map_activity_row)?
                .collect::<std::result::Result<Vec<_>, _>>()?;
            result
        }
    };

    Ok(Json(activities))
}

/// Maps a database row to an Activity struct.
fn map_activity_row(row: &rusqlite::Row) -> rusqlite::Result<Activity> {
    Ok(Activity {
        id: row.get(0)?,
        event_type: row.get(1)?,
        message: row.get(2)?,
        media_type: row.get(3)?,
        media_id: row.get(4)?,
        download_id: row.get(5)?,
        user_id: row.get(6)?,
        metadata: row.get(7)?,
        created_at: row.get(8)?,
    })
}

// =============================================================================
// Indexer Handlers
// =============================================================================

/// GET /api/system/indexers
///
/// List all configured indexers.
pub async fn list_indexers(State(state): State<AppState>) -> Result<Json<Vec<IndexerResponse>>> {
    let db = state.db.lock().await;

    let mut stmt = db.prepare(
        r#"
        SELECT id, name, indexer_type, url, api_key, enabled, priority, categories, last_check, last_error, created_at
        FROM indexers
        ORDER BY priority DESC
        "#,
    )?;

    let indexers = stmt
        .query_map([], map_indexer_row)?
        .collect::<std::result::Result<Vec<Indexer>, _>>()?
        .into_iter()
        .map(IndexerResponse::from)
        .collect();

    Ok(Json(indexers))
}

/// POST /api/system/indexers
///
/// Create a new indexer.
pub async fn create_indexer(
    State(state): State<AppState>,
    Json(req): Json<CreateIndexerRequest>,
) -> Result<Json<IndexerResponse>> {
    // Validate request
    req.validate()?;

    let db = state.db.lock().await;

    let enabled = req.enabled.unwrap_or(true);
    let priority = req.priority.unwrap_or(0);

    db.execute(
        r#"
        INSERT INTO indexers (name, indexer_type, url, api_key, enabled, priority, categories, created_at)
        VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, datetime('now'))
        "#,
        rusqlite::params![
            req.name,
            req.indexer_type,
            req.url,
            req.api_key,
            enabled,
            priority,
            req.categories
        ],
    )?;

    let id = db.last_insert_rowid();

    let indexer = db.query_row(
        r#"
        SELECT id, name, indexer_type, url, api_key, enabled, priority, categories, last_check, last_error, created_at
        FROM indexers WHERE id = ?1
        "#,
        [id],
        map_indexer_row,
    )?;

    tracing::info!(indexer_id = id, name = %req.name, "Created indexer");

    Ok(Json(IndexerResponse::from(indexer)))
}

/// PUT /api/system/indexers/:id
///
/// Update an existing indexer.
pub async fn update_indexer(
    State(state): State<AppState>,
    Path(indexer_id): Path<i64>,
    Json(req): Json<UpdateIndexerRequest>,
) -> Result<Json<IndexerResponse>> {
    let db = state.db.lock().await;

    // Verify indexer exists
    let exists: bool = db
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM indexers WHERE id = ?1)",
            [indexer_id],
            |row| row.get(0),
        )
        .unwrap_or(false);

    if !exists {
        return Err(AppError::NotFound("Indexer not found".to_string()));
    }

    // Build dynamic update query
    let mut updates = Vec::new();
    let mut params: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();

    if let Some(ref name) = req.name {
        updates.push("name = ?");
        params.push(Box::new(name.clone()));
    }
    if let Some(ref indexer_type) = req.indexer_type {
        updates.push("indexer_type = ?");
        params.push(Box::new(indexer_type.clone()));
    }
    if let Some(ref url) = req.url {
        updates.push("url = ?");
        params.push(Box::new(url.clone()));
    }
    if let Some(ref api_key) = req.api_key {
        updates.push("api_key = ?");
        params.push(Box::new(api_key.clone()));
    }
    if let Some(enabled) = req.enabled {
        updates.push("enabled = ?");
        params.push(Box::new(enabled));
    }
    if let Some(priority) = req.priority {
        updates.push("priority = ?");
        params.push(Box::new(priority));
    }
    if let Some(ref categories) = req.categories {
        updates.push("categories = ?");
        params.push(Box::new(categories.clone()));
    }

    if updates.is_empty() {
        return Err(AppError::BadRequest(
            "No fields to update provided".to_string(),
        ));
    }

    params.push(Box::new(indexer_id));

    let sql = format!("UPDATE indexers SET {} WHERE id = ?", updates.join(", "));

    let param_refs: Vec<&dyn rusqlite::ToSql> = params.iter().map(|p| p.as_ref()).collect();
    db.execute(&sql, param_refs.as_slice())?;

    let indexer = db.query_row(
        r#"
        SELECT id, name, indexer_type, url, api_key, enabled, priority, categories, last_check, last_error, created_at
        FROM indexers WHERE id = ?1
        "#,
        [indexer_id],
        map_indexer_row,
    )?;

    tracing::info!(indexer_id = indexer_id, "Updated indexer");

    Ok(Json(IndexerResponse::from(indexer)))
}

/// DELETE /api/system/indexers/:id
///
/// Delete an indexer.
pub async fn delete_indexer(
    State(state): State<AppState>,
    Path(indexer_id): Path<i64>,
) -> Result<Json<SuccessResponse>> {
    let db = state.db.lock().await;

    let rows_affected = db.execute("DELETE FROM indexers WHERE id = ?1", [indexer_id])?;

    if rows_affected == 0 {
        return Err(AppError::NotFound("Indexer not found".to_string()));
    }

    tracing::info!(indexer_id = indexer_id, "Deleted indexer");

    Ok(Json(SuccessResponse {
        success: true,
        message: Some("Indexer deleted successfully".to_string()),
    }))
}

/// POST /api/system/indexers/:id/test
///
/// Test an indexer's connectivity.
pub async fn test_indexer(
    State(state): State<AppState>,
    Path(indexer_id): Path<i64>,
) -> Result<Json<IndexerTestResponse>> {
    tracing::info!(indexer_id = indexer_id, "Testing indexer connectivity");

    let (url, api_key): (String, Option<String>) = {
        let db = state.db.lock().await;
        db.query_row(
            "SELECT url, api_key FROM indexers WHERE id = ?1",
            [indexer_id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .map_err(|e| match e {
            rusqlite::Error::QueryReturnedNoRows => {
                AppError::NotFound("Indexer not found".to_string())
            }
            _ => AppError::Sqlite(e),
        })?
    };

    // Validate URL to prevent SSRF
    validate_indexer_url(&url)?;

    // Test connectivity
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .build()
        .map_err(|e| AppError::Internal(format!("Failed to create HTTP client: {}", e)))?;

    let start = std::time::Instant::now();

    let mut request = client.get(&url);
    if let Some(key) = &api_key {
        request = request.header("X-Api-Key", key);
    }

    match request.send().await {
        Ok(response) => {
            let response_time_ms = start.elapsed().as_millis() as u64;
            let status = response.status();
            let success = status.is_success();

            tracing::info!(
                indexer_id = indexer_id,
                status = %status,
                response_time_ms = response_time_ms,
                "Indexer test completed"
            );

            // Update last_check in database
            let db = state.db.lock().await;
            if success {
                db.execute(
                    "UPDATE indexers SET last_check = datetime('now'), last_error = NULL WHERE id = ?1",
                    [indexer_id],
                )?;
            } else {
                let error = format!("HTTP {}", status);
                db.execute(
                    "UPDATE indexers SET last_check = datetime('now'), last_error = ?1 WHERE id = ?2",
                    rusqlite::params![error, indexer_id],
                )?;
            }

            Ok(Json(IndexerTestResponse {
                success,
                response_time_ms: Some(response_time_ms),
                error: if success {
                    None
                } else {
                    Some(format!("HTTP {}", status))
                },
            }))
        }
        Err(e) => {
            // Update last_error in database
            let error_msg = e.to_string();

            tracing::warn!(
                indexer_id = indexer_id,
                error = %e,
                "Indexer test failed"
            );

            let db = state.db.lock().await;
            db.execute(
                "UPDATE indexers SET last_check = datetime('now'), last_error = ?1 WHERE id = ?2",
                rusqlite::params![error_msg, indexer_id],
            )?;

            Ok(Json(IndexerTestResponse {
                success: false,
                response_time_ms: None,
                error: Some(error_msg),
            }))
        }
    }
}

/// Validate indexer URL to prevent SSRF attacks.
fn validate_indexer_url(url: &str) -> Result<()> {
    use std::net::IpAddr;

    let parsed = reqwest::Url::parse(url)
        .map_err(|_| AppError::BadRequest("Invalid URL format".to_string()))?;

    // Only allow http/https
    match parsed.scheme() {
        "http" | "https" => {}
        _ => {
            return Err(AppError::BadRequest(
                "URL must use http or https".to_string(),
            ))
        }
    }

    // Block localhost and private IPs
    if let Some(host) = parsed.host_str() {
        if host == "localhost" || host == "127.0.0.1" || host == "0.0.0.0" || host == "::1" {
            return Err(AppError::BadRequest(
                "Localhost URLs not allowed".to_string(),
            ));
        }

        // Check if host is an IP address
        if let Ok(addr) = host.parse::<IpAddr>() {
            match addr {
                IpAddr::V4(ipv4) => {
                    if ipv4.is_loopback() || ipv4.is_private() || ipv4.is_link_local() {
                        return Err(AppError::BadRequest(
                            "Private/loopback IPs not allowed".to_string(),
                        ));
                    }
                    // Block 0.0.0.0/8 and 169.254.0.0/16
                    let octets = ipv4.octets();
                    if octets[0] == 0 || (octets[0] == 169 && octets[1] == 254) {
                        return Err(AppError::BadRequest(
                            "Reserved IP ranges not allowed".to_string(),
                        ));
                    }
                }
                IpAddr::V6(ipv6) => {
                    if ipv6.is_loopback() || ipv6.is_unspecified() {
                        return Err(AppError::BadRequest(
                            "Loopback/unspecified IPs not allowed".to_string(),
                        ));
                    }
                }
            }
        }
    }

    Ok(())
}

/// Maps a database row to an Indexer struct.
fn map_indexer_row(row: &rusqlite::Row) -> rusqlite::Result<Indexer> {
    Ok(Indexer {
        id: row.get(0)?,
        name: row.get(1)?,
        indexer_type: row.get(2)?,
        url: row.get(3)?,
        api_key: row.get(4)?,
        enabled: row.get(5)?,
        priority: row.get(6)?,
        categories: row.get(7)?,
        last_check: row.get(8)?,
        last_error: row.get(9)?,
        created_at: row.get(10)?,
    })
}

// =============================================================================
// Storage Handlers
// =============================================================================

/// GET /api/system/storage/mounts
///
/// List all configured storage mounts with their status.
pub async fn list_mounts(State(state): State<AppState>) -> Result<Json<Vec<MountInfo>>> {
    let storage_manager = state
        .storage_manager()
        .ok_or_else(|| AppError::Internal("Storage manager not available".to_string()))?;

    let mount_names = storage_manager.list_mounts();
    let mut mounts = Vec::new();

    for name in mount_names {
        if let Some(mount) = storage_manager.get_mount(name) {
            let root = mount.root();
            let path = root.to_string_lossy().to_string();
            let available = root.exists();

            let (free_bytes, total_bytes) = if available {
                get_disk_space(root)
            } else {
                (None, None)
            };

            mounts.push(MountInfo {
                name: name.to_string(),
                path,
                available,
                free_bytes,
                total_bytes,
            });
        }
    }

    Ok(Json(mounts))
}

/// POST /api/system/storage/mounts/:name/test
///
/// Test a storage mount's availability.
pub async fn test_mount(
    State(state): State<AppState>,
    Path(mount_name): Path<String>,
) -> Result<Json<MountTestResponse>> {
    let storage_manager = state
        .storage_manager()
        .ok_or_else(|| AppError::Internal("Storage manager not available".to_string()))?;

    let mount = storage_manager
        .get_mount(&mount_name)
        .ok_or_else(|| AppError::NotFound(format!("Mount '{}' not found", mount_name)))?;

    let root = mount.root();

    if !root.exists() {
        return Ok(Json(MountTestResponse {
            success: false,
            available: false,
            free_bytes: None,
            error: Some("Mount path does not exist".to_string()),
        }));
    }

    // Try to write a test file
    let test_file = root.join(".lcars_mount_test");
    match std::fs::write(&test_file, "test") {
        Ok(_) => {
            let _ = std::fs::remove_file(&test_file);
            let (free_bytes, _) = get_disk_space(root);

            Ok(Json(MountTestResponse {
                success: true,
                available: true,
                free_bytes,
                error: None,
            }))
        }
        Err(e) => Ok(Json(MountTestResponse {
            success: false,
            available: true,
            free_bytes: None,
            error: Some(format!("Write test failed: {}", e)),
        })),
    }
}

/// Get disk space for a path.
#[cfg(unix)]
fn get_disk_space(path: &std::path::Path) -> (Option<u64>, Option<u64>) {
    use std::ffi::CString;
    use std::os::unix::ffi::OsStrExt;

    let c_path = match CString::new(path.as_os_str().as_bytes()) {
        Ok(p) => p,
        Err(_) => return (None, None),
    };

    let mut stat: libc::statvfs = unsafe { std::mem::zeroed() };
    let result = unsafe { libc::statvfs(c_path.as_ptr(), &mut stat) };

    if result == 0 {
        // Allow casts - types differ between platforms (u32 on some, u64 on others)
        #[allow(clippy::unnecessary_cast)]
        let free = stat.f_bavail as u64 * stat.f_frsize as u64;
        #[allow(clippy::unnecessary_cast)]
        let total = stat.f_blocks as u64 * stat.f_frsize as u64;
        (Some(free), Some(total))
    } else {
        (None, None)
    }
}

#[cfg(not(unix))]
fn get_disk_space(_path: &std::path::Path) -> (Option<u64>, Option<u64>) {
    (None, None)
}
