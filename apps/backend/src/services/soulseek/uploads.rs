//! Upload handling for Soulseek file sharing.
//!
//! This module manages the upload queue and tracks active uploads to peers.

use serde::Serialize;
use std::collections::{HashMap, VecDeque};
use std::path::PathBuf;
use std::time::Instant;
use uuid::Uuid;

/// Status of an upload.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum UploadStatus {
    /// Waiting in queue.
    Queued,
    /// Currently transferring.
    Transferring,
    /// Successfully completed.
    Completed,
    /// Failed with error.
    Failed,
    /// Cancelled by user or peer.
    Cancelled,
}

/// State of an active or completed upload.
#[derive(Debug, Clone)]
pub struct UploadState {
    /// Unique upload ID.
    pub id: String,
    /// Username of the peer downloading.
    pub username: String,
    /// Virtual path of the file being uploaded.
    pub filename: String,
    /// Absolute path on disk.
    pub local_path: PathBuf,
    /// Total file size in bytes.
    pub size: u64,
    /// Bytes uploaded so far.
    pub uploaded: u64,
    /// Current upload speed in bytes/sec.
    pub speed: u64,
    /// Current status.
    pub status: UploadStatus,
    /// Transfer ticket from the peer.
    pub ticket: u32,
    /// When the upload started.
    pub started_at: Option<Instant>,
    /// When the upload completed or failed.
    pub completed_at: Option<Instant>,
    /// Error message if failed.
    pub error: Option<String>,
}

impl UploadState {
    /// Create a new upload state.
    pub fn new(
        username: String,
        filename: String,
        local_path: PathBuf,
        size: u64,
        ticket: u32,
    ) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            username,
            filename,
            local_path,
            size,
            uploaded: 0,
            speed: 0,
            status: UploadStatus::Queued,
            ticket,
            started_at: None,
            completed_at: None,
            error: None,
        }
    }

    /// Mark the upload as started.
    pub fn start(&mut self) {
        self.status = UploadStatus::Transferring;
        self.started_at = Some(Instant::now());
    }

    /// Update progress.
    pub fn update_progress(&mut self, uploaded: u64, speed: u64) {
        self.uploaded = uploaded;
        self.speed = speed;
    }

    /// Mark the upload as completed.
    pub fn complete(&mut self) {
        self.status = UploadStatus::Completed;
        self.uploaded = self.size;
        self.speed = 0;
        self.completed_at = Some(Instant::now());
    }

    /// Mark the upload as failed.
    pub fn fail(&mut self, error: String) {
        self.status = UploadStatus::Failed;
        self.speed = 0;
        self.error = Some(error);
        self.completed_at = Some(Instant::now());
    }

    /// Mark the upload as cancelled.
    pub fn cancel(&mut self) {
        self.status = UploadStatus::Cancelled;
        self.speed = 0;
        self.completed_at = Some(Instant::now());
    }

    /// Calculate progress as a percentage (0-100).
    pub fn progress_percent(&self) -> f64 {
        if self.size == 0 {
            100.0
        } else {
            (self.uploaded as f64 / self.size as f64) * 100.0
        }
    }

    /// Check if the upload is finished (completed, failed, or cancelled).
    pub fn is_finished(&self) -> bool {
        matches!(
            self.status,
            UploadStatus::Completed | UploadStatus::Failed | UploadStatus::Cancelled
        )
    }
}

/// A pending upload request waiting in queue.
#[derive(Debug, Clone)]
pub struct PendingUpload {
    /// Username of the peer.
    pub username: String,
    /// Virtual path of the requested file.
    pub filename: String,
    /// Absolute path on disk.
    pub local_path: PathBuf,
    /// File size.
    pub size: u64,
    /// Transfer ticket.
    pub ticket: u32,
    /// When the request was received.
    pub queued_at: Instant,
}

/// Upload queue manager.
#[derive(Debug)]
pub struct UploadQueue {
    /// Active uploads (by ID).
    active: HashMap<String, UploadState>,
    /// Pending uploads waiting for a slot.
    pending: VecDeque<PendingUpload>,
    /// Maximum concurrent uploads.
    max_slots: u32,
    /// Total bytes uploaded (lifetime).
    total_uploaded: u64,
    /// Total uploads completed (lifetime).
    total_completed: u64,
}

impl UploadQueue {
    /// Create a new upload queue.
    pub fn new(max_slots: u32) -> Self {
        Self {
            active: HashMap::new(),
            pending: VecDeque::new(),
            max_slots,
            total_uploaded: 0,
            total_completed: 0,
        }
    }

    /// Get the number of active uploads.
    pub fn active_count(&self) -> usize {
        self.active
            .values()
            .filter(|u| u.status == UploadStatus::Transferring)
            .count()
    }

    /// Get the number of pending uploads.
    pub fn pending_count(&self) -> usize {
        self.pending.len()
    }

    /// Check if we have a free upload slot.
    pub fn has_free_slot(&self) -> bool {
        self.active_count() < self.max_slots as usize
    }

    /// Get the queue position for a username/filename.
    pub fn get_queue_position(&self, username: &str, filename: &str) -> Option<u32> {
        self.pending
            .iter()
            .position(|p| p.username == username && p.filename == filename)
            .map(|pos| pos as u32 + 1)
    }

    /// Add an upload request to the queue.
    ///
    /// Returns the UploadState if started immediately, or None if queued.
    pub fn enqueue(
        &mut self,
        username: String,
        filename: String,
        local_path: PathBuf,
        size: u64,
        ticket: u32,
    ) -> Option<UploadState> {
        if self.has_free_slot() {
            // Start immediately
            let mut state = UploadState::new(username, filename, local_path, size, ticket);
            state.start();
            let id = state.id.clone();
            self.active.insert(id, state.clone());
            Some(state)
        } else {
            // Add to queue
            self.pending.push_back(PendingUpload {
                username,
                filename,
                local_path,
                size,
                ticket,
                queued_at: Instant::now(),
            });
            None
        }
    }

    /// Try to start the next queued upload if a slot is available.
    ///
    /// Returns the started upload state if one was started.
    pub fn try_start_next(&mut self) -> Option<UploadState> {
        if !self.has_free_slot() {
            return None;
        }

        let pending = self.pending.pop_front()?;
        let mut state = UploadState::new(
            pending.username,
            pending.filename,
            pending.local_path,
            pending.size,
            pending.ticket,
        );
        state.start();
        let id = state.id.clone();
        self.active.insert(id, state.clone());
        Some(state)
    }

    /// Get an upload by ID.
    pub fn get(&self, id: &str) -> Option<&UploadState> {
        self.active.get(id)
    }

    /// Get a mutable reference to an upload by ID.
    pub fn get_mut(&mut self, id: &str) -> Option<&mut UploadState> {
        self.active.get_mut(id)
    }

    /// Find an upload by username and ticket.
    pub fn find_by_ticket(&self, username: &str, ticket: u32) -> Option<&UploadState> {
        self.active
            .values()
            .find(|u| u.username == username && u.ticket == ticket)
    }

    /// Find a mutable upload by username and ticket.
    pub fn find_by_ticket_mut(&mut self, username: &str, ticket: u32) -> Option<&mut UploadState> {
        self.active
            .values_mut()
            .find(|u| u.username == username && u.ticket == ticket)
    }

    /// Update upload progress.
    pub fn update_progress(&mut self, id: &str, uploaded: u64, speed: u64) {
        if let Some(upload) = self.active.get_mut(id) {
            upload.update_progress(uploaded, speed);
        }
    }

    /// Complete an upload.
    pub fn complete(&mut self, id: &str) {
        if let Some(upload) = self.active.get_mut(id) {
            self.total_uploaded += upload.size;
            self.total_completed += 1;
            upload.complete();
        }
    }

    /// Fail an upload.
    pub fn fail(&mut self, id: &str, error: String) {
        if let Some(upload) = self.active.get_mut(id) {
            self.total_uploaded += upload.uploaded;
            upload.fail(error);
        }
    }

    /// Cancel an upload.
    pub fn cancel(&mut self, id: &str) -> bool {
        if let Some(upload) = self.active.get_mut(id) {
            upload.cancel();
            true
        } else {
            // Check if it's in the pending queue
            // We don't have ID for pending, but this method is primarily for active uploads
            false
        }
    }

    /// Remove a pending upload by username and filename.
    pub fn remove_pending(&mut self, username: &str, filename: &str) -> bool {
        let initial_len = self.pending.len();
        self.pending
            .retain(|p| !(p.username == username && p.filename == filename));
        self.pending.len() != initial_len
    }

    /// Get all active uploads (including completed/failed that haven't been cleaned up).
    pub fn get_active(&self) -> Vec<&UploadState> {
        self.active.values().collect()
    }

    /// Get only currently transferring uploads.
    pub fn get_transferring(&self) -> Vec<&UploadState> {
        self.active
            .values()
            .filter(|u| u.status == UploadStatus::Transferring)
            .collect()
    }

    /// Get all pending uploads.
    pub fn get_pending(&self) -> Vec<&PendingUpload> {
        self.pending.iter().collect()
    }

    /// Clean up completed uploads older than the specified duration.
    pub fn cleanup_finished(&mut self, max_age_secs: u64) {
        let now = Instant::now();
        self.active.retain(|_, upload| {
            if let Some(completed_at) = upload.completed_at {
                now.duration_since(completed_at).as_secs() < max_age_secs
            } else {
                true
            }
        });
    }

    /// Get total bytes uploaded (lifetime).
    pub fn total_uploaded(&self) -> u64 {
        self.total_uploaded
    }

    /// Get total uploads completed (lifetime).
    pub fn total_completed(&self) -> u64 {
        self.total_completed
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_upload_state_lifecycle() {
        let mut state = UploadState::new(
            "user".to_string(),
            "file.mp3".to_string(),
            PathBuf::from("/path/file.mp3"),
            1000,
            12345,
        );

        assert_eq!(state.status, UploadStatus::Queued);
        assert!(!state.is_finished());

        state.start();
        assert_eq!(state.status, UploadStatus::Transferring);
        assert!(state.started_at.is_some());

        state.update_progress(500, 100);
        assert_eq!(state.uploaded, 500);
        assert_eq!(state.speed, 100);
        assert_eq!(state.progress_percent(), 50.0);

        state.complete();
        assert_eq!(state.status, UploadStatus::Completed);
        assert!(state.is_finished());
    }

    #[test]
    fn test_upload_queue_slots() {
        let mut queue = UploadQueue::new(2);

        assert!(queue.has_free_slot());

        // First upload starts immediately
        let upload1 = queue.enqueue(
            "user1".to_string(),
            "file1.mp3".to_string(),
            PathBuf::from("/file1.mp3"),
            1000,
            1,
        );
        assert!(upload1.is_some());
        assert_eq!(queue.active_count(), 1);

        // Second upload starts immediately
        let upload2 = queue.enqueue(
            "user2".to_string(),
            "file2.mp3".to_string(),
            PathBuf::from("/file2.mp3"),
            1000,
            2,
        );
        assert!(upload2.is_some());
        assert_eq!(queue.active_count(), 2);
        assert!(!queue.has_free_slot());

        // Third upload gets queued
        let upload3 = queue.enqueue(
            "user3".to_string(),
            "file3.mp3".to_string(),
            PathBuf::from("/file3.mp3"),
            1000,
            3,
        );
        assert!(upload3.is_none());
        assert_eq!(queue.pending_count(), 1);

        // Check queue position
        let pos = queue.get_queue_position("user3", "file3.mp3");
        assert_eq!(pos, Some(1));

        // Complete first upload
        if let Some(u) = upload1 {
            queue.complete(&u.id);
        }

        // Now we can start the next one
        let next = queue.try_start_next();
        assert!(next.is_some());
        assert_eq!(queue.pending_count(), 0);
    }

    #[test]
    fn test_find_by_ticket() {
        let mut queue = UploadQueue::new(5);

        queue.enqueue(
            "user1".to_string(),
            "file1.mp3".to_string(),
            PathBuf::from("/file1.mp3"),
            1000,
            12345,
        );

        let found = queue.find_by_ticket("user1", 12345);
        assert!(found.is_some());
        assert_eq!(found.unwrap().username, "user1");

        let not_found = queue.find_by_ticket("user1", 99999);
        assert!(not_found.is_none());
    }
}
