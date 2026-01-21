//! Activity logging service for tracking system events.

#![allow(dead_code)]

use rusqlite::Connection;
use serde::Serialize;
use std::sync::Arc;
use tokio::sync::Mutex;

/// Event types for activity logging.
#[derive(Debug, Clone, Copy)]
pub enum EventType {
    // Media events
    MediaAdded,
    MediaUpdated,
    MediaDeleted,
    MetadataRefreshed,

    // Download events
    DownloadStarted,
    DownloadCompleted,
    DownloadFailed,
    DownloadPaused,
    DownloadResumed,

    // Job events
    JobStarted,
    JobCompleted,
    JobFailed,

    // User events
    UserLogin,
    UserLogout,
    UserCreated,
    UserDeleted,

    // System events
    SystemStarted,
    ConfigChanged,
}

impl EventType {
    /// Get the string representation for database storage.
    pub fn as_str(&self) -> &'static str {
        match self {
            EventType::MediaAdded => "media_added",
            EventType::MediaUpdated => "media_updated",
            EventType::MediaDeleted => "media_deleted",
            EventType::MetadataRefreshed => "metadata_refreshed",
            EventType::DownloadStarted => "download_started",
            EventType::DownloadCompleted => "download_completed",
            EventType::DownloadFailed => "download_failed",
            EventType::DownloadPaused => "download_paused",
            EventType::DownloadResumed => "download_resumed",
            EventType::JobStarted => "job_started",
            EventType::JobCompleted => "job_completed",
            EventType::JobFailed => "job_failed",
            EventType::UserLogin => "user_login",
            EventType::UserLogout => "user_logout",
            EventType::UserCreated => "user_created",
            EventType::UserDeleted => "user_deleted",
            EventType::SystemStarted => "system_started",
            EventType::ConfigChanged => "config_changed",
        }
    }
}

/// Builder for creating activity log entries.
pub struct ActivityBuilder {
    event_type: EventType,
    message: String,
    media_type: Option<String>,
    media_id: Option<i64>,
    download_id: Option<i64>,
    user_id: Option<i64>,
    metadata: Option<String>,
}

impl ActivityBuilder {
    /// Create a new activity builder.
    pub fn new(event_type: EventType, message: impl Into<String>) -> Self {
        Self {
            event_type,
            message: message.into(),
            media_type: None,
            media_id: None,
            download_id: None,
            user_id: None,
            metadata: None,
        }
    }

    /// Associate with a media item.
    pub fn media(mut self, media_type: &str, media_id: i64) -> Self {
        self.media_type = Some(media_type.to_string());
        self.media_id = Some(media_id);
        self
    }

    /// Associate with a download.
    pub fn download(mut self, download_id: i64) -> Self {
        self.download_id = Some(download_id);
        self
    }

    /// Associate with a user.
    pub fn user(mut self, user_id: i64) -> Self {
        self.user_id = Some(user_id);
        self
    }

    /// Add metadata as JSON.
    pub fn metadata<T: Serialize>(mut self, data: &T) -> Self {
        self.metadata = serde_json::to_string(data).ok();
        self
    }

    /// Log the activity to the database.
    pub async fn log(self, db: &Arc<Mutex<Connection>>) {
        let db = db.lock().await;

        if let Err(e) = db.execute(
            r#"
            INSERT INTO activity (event_type, message, media_type, media_id, download_id, user_id, metadata, created_at)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, datetime('now'))
            "#,
            rusqlite::params![
                self.event_type.as_str(),
                self.message,
                self.media_type,
                self.media_id,
                self.download_id,
                self.user_id,
                self.metadata
            ],
        ) {
            tracing::error!(error = %e, "Failed to log activity");
        }
    }

    /// Log the activity synchronously (for use when already holding the lock).
    pub fn log_sync(self, conn: &Connection) {
        if let Err(e) = conn.execute(
            r#"
            INSERT INTO activity (event_type, message, media_type, media_id, download_id, user_id, metadata, created_at)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, datetime('now'))
            "#,
            rusqlite::params![
                self.event_type.as_str(),
                self.message,
                self.media_type,
                self.media_id,
                self.download_id,
                self.user_id,
                self.metadata
            ],
        ) {
            tracing::error!(error = %e, "Failed to log activity");
        }
    }
}

/// Convenience function to log a simple event.
pub async fn log_event(
    db: &Arc<Mutex<Connection>>,
    event_type: EventType,
    message: impl Into<String>,
) {
    ActivityBuilder::new(event_type, message).log(db).await;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_event_type_as_str() {
        assert_eq!(EventType::MediaAdded.as_str(), "media_added");
        assert_eq!(EventType::DownloadCompleted.as_str(), "download_completed");
        assert_eq!(EventType::UserLogin.as_str(), "user_login");
    }
}
