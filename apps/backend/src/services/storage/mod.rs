//! Storage service for managing media files across local and network storage.
//!
//! Provides a unified interface for file operations across different storage backends
//! (local filesystem, SMB shares) with support for configurable naming patterns and
//! post-download processing rules.

#![allow(dead_code)]

mod local;
mod naming;

pub use local::LocalMount;
pub use naming::NamingEngine;

use async_trait::async_trait;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use crate::config::{MountType, StorageAction, StorageConfig, StorageRule};
use crate::db::models::{Album, Artist, Episode, MediaType, Movie, Track, TvShow};
use crate::error::{AppError, Result};

/// Trait defining the interface for storage backends.
///
/// Implementations handle file operations for different storage types
/// (local filesystem, SMB shares, etc.).
#[async_trait]
pub trait Mount: Send + Sync {
    /// Returns the unique name identifier for this mount.
    fn name(&self) -> &str;

    /// Returns the type of mount (e.g., "local", "smb").
    fn mount_type(&self) -> &str;

    /// Returns the root path of the mount.
    fn root(&self) -> &Path;

    /// Checks if the mount point is currently accessible.
    async fn available(&self) -> bool;

    /// Returns the free space available on the mount in bytes.
    async fn free_space(&self) -> Result<u64>;

    /// Checks if a path exists relative to the mount root.
    async fn exists(&self, path: &Path) -> bool;

    /// Writes a file from source to destination relative to mount root.
    ///
    /// Creates parent directories as needed.
    async fn write_file(&self, source: &Path, dest: &Path) -> Result<()>;

    /// Deletes a file at the path relative to mount root.
    async fn delete_file(&self, path: &Path) -> Result<()>;

    /// Creates all directories in the path relative to mount root.
    async fn create_dir_all(&self, path: &Path) -> Result<()>;
}

/// Media metadata for naming pattern expansion.
///
/// Contains all information needed to generate file paths using naming patterns.
/// Large variants are boxed to reduce overall enum size.
#[derive(Debug, Clone)]
pub enum MediaInfo {
    Movie {
        movie: Box<Movie>,
        quality: String,
    },
    Episode {
        show: Box<TvShow>,
        episode: Box<Episode>,
        quality: String,
    },
    Album {
        artist: Box<Artist>,
        album: Box<Album>,
    },
    Track {
        artist: Box<Artist>,
        album: Box<Album>,
        track: Box<Track>,
    },
}

/// Result of processing a completed download.
#[derive(Debug, Clone)]
pub struct ProcessedFile {
    /// The original source file path.
    pub source: PathBuf,
    /// The final destination path after processing.
    pub destination: PathBuf,
    /// The mount name where the file was stored.
    pub mount_name: String,
    /// Size of the file in bytes.
    pub size: u64,
}

/// Video file extensions.
const VIDEO_EXTENSIONS: &[&str] = &["mkv", "mp4", "avi", "m4v", "wmv", "mov", "webm"];

/// Audio file extensions.
const AUDIO_EXTENSIONS: &[&str] = &["flac", "mp3", "m4a", "ogg", "wav", "aac", "opus", "alac"];

/// Manages storage operations and post-download processing.
///
/// Coordinates file operations across multiple mounts with configurable
/// rules for organizing and renaming media files.
pub struct StorageManager {
    mounts: HashMap<String, Arc<dyn Mount>>,
    rules: Vec<StorageRule>,
    naming: NamingEngine,
}

impl StorageManager {
    /// Creates a new StorageManager from configuration.
    ///
    /// Initializes all enabled mounts and validates storage rules.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - A mount configuration is invalid
    /// - A storage rule references a non-existent mount
    pub fn new(config: StorageConfig) -> Result<Self> {
        let mut mounts: HashMap<String, Arc<dyn Mount>> = HashMap::new();

        for mount_config in config.mounts {
            if !mount_config.enabled {
                tracing::debug!(name = %mount_config.name, "Skipping disabled mount");
                continue;
            }

            let mount: Arc<dyn Mount> = match mount_config.mount_type {
                MountType::Local => {
                    let path = mount_config.path.ok_or_else(|| {
                        AppError::Internal(format!(
                            "Local mount '{}' missing required 'path' field",
                            mount_config.name
                        ))
                    })?;
                    Arc::new(LocalMount::new(mount_config.name.clone(), path))
                }
                MountType::Smb => {
                    // SMB mount support is optional per the issue
                    tracing::warn!(
                        name = %mount_config.name,
                        "SMB mounts not yet implemented, skipping"
                    );
                    continue;
                }
            };

            tracing::info!(
                name = %mount.name(),
                mount_type = %mount.mount_type(),
                "Registered storage mount"
            );
            mounts.insert(mount_config.name, mount);
        }

        // Validate rules reference valid mounts
        for rule in &config.rules {
            if !mounts.contains_key(&rule.destination) {
                return Err(AppError::Internal(format!(
                    "Storage rule references unknown mount: '{}'",
                    rule.destination
                )));
            }
        }

        let naming = NamingEngine::new(config.naming);

        Ok(Self {
            mounts,
            rules: config.rules,
            naming,
        })
    }

    /// Creates a new StorageManager wrapped in Arc for shared access.
    pub fn new_shared(config: StorageConfig) -> Result<Arc<Self>> {
        Ok(Arc::new(Self::new(config)?))
    }

    /// Processes a completed download by moving/copying files to their destinations.
    ///
    /// # Arguments
    ///
    /// * `download_path` - Path to the downloaded content (file or directory)
    /// * `media_info` - Metadata about the media for naming pattern expansion
    ///
    /// # Returns
    ///
    /// Returns information about all processed files including their final paths.
    ///
    /// # Process
    ///
    /// 1. Finds media files in the download directory
    /// 2. Generates destination paths using naming patterns
    /// 3. Executes applicable storage rules (move/copy)
    /// 4. Cleans up empty directories
    pub async fn process_completed_download(
        &self,
        download_path: &Path,
        media_info: &MediaInfo,
    ) -> Result<Vec<ProcessedFile>> {
        tracing::debug!(
            download_path = ?download_path,
            "Processing completed download"
        );

        let media_type = match media_info {
            MediaInfo::Movie { .. } => MediaType::Movie,
            MediaInfo::Episode { .. } => MediaType::Episode,
            MediaInfo::Album { .. } => MediaType::Album,
            MediaInfo::Track { .. } => MediaType::Track,
        };

        // Find media files
        let files = find_media_files(download_path, media_type).await?;

        if files.is_empty() {
            return Err(AppError::NotFound(format!(
                "No media files found in {:?}",
                download_path
            )));
        }

        tracing::debug!(file_count = files.len(), "Found media files to process");

        let mut processed = Vec::new();

        // Find applicable rules
        let applicable_rules: Vec<_> = self
            .rules
            .iter()
            .filter(|rule| {
                rule.media_types.is_empty()
                    || rule
                        .media_types
                        .iter()
                        .any(|t| t.eq_ignore_ascii_case(&media_type.to_string()))
            })
            .collect();

        if applicable_rules.is_empty() {
            tracing::warn!(
                media_type = %media_type,
                "No storage rules apply to this media type"
            );
            return Ok(processed);
        }

        for source_file in files {
            let ext = source_file
                .extension()
                .and_then(|e| e.to_str())
                .unwrap_or("");

            // Generate destination path using naming pattern
            let relative_dest = self.naming.generate_path(media_info, ext);

            // Execute rules in order
            for rule in &applicable_rules {
                let mount = self.mounts.get(&rule.destination).ok_or_else(|| {
                    AppError::Internal(format!("Mount '{}' not found for rule", rule.destination))
                })?;

                // Check mount availability
                if !mount.available().await {
                    tracing::warn!(
                        mount = %mount.name(),
                        "Mount not available, skipping rule"
                    );
                    continue;
                }

                let dest_path = PathBuf::from(&relative_dest);

                // Get file size before move
                let file_size = tokio::fs::metadata(&source_file)
                    .await
                    .map(|m| m.len())
                    .unwrap_or(0);

                // Execute action
                match rule.action {
                    StorageAction::Move => {
                        tracing::debug!(
                            source = ?source_file,
                            dest = ?dest_path,
                            mount = %mount.name(),
                            "Moving file"
                        );
                        mount.write_file(&source_file, &dest_path).await?;
                        // Delete source after successful write
                        if let Err(e) = tokio::fs::remove_file(&source_file).await {
                            tracing::warn!(
                                source = ?source_file,
                                error = %e,
                                "Failed to remove source file after move"
                            );
                        }
                    }
                    StorageAction::Copy => {
                        tracing::debug!(
                            source = ?source_file,
                            dest = ?dest_path,
                            mount = %mount.name(),
                            "Copying file"
                        );
                        mount.write_file(&source_file, &dest_path).await?;
                    }
                }

                let full_dest = mount.root().join(&dest_path);

                processed.push(ProcessedFile {
                    source: source_file.clone(),
                    destination: full_dest,
                    mount_name: mount.name().to_string(),
                    size: file_size,
                });

                // Only apply first matching rule per file
                break;
            }
        }

        // Clean up empty directories in the download path
        if download_path.is_dir() {
            if let Err(e) = cleanup_empty_dirs(download_path).await {
                tracing::warn!(
                    path = ?download_path,
                    error = %e,
                    "Failed to clean up empty directories"
                );
            }
        }

        tracing::info!(
            processed_count = processed.len(),
            "Download processing complete"
        );

        Ok(processed)
    }

    /// Gets a mount by name.
    pub fn get_mount(&self, name: &str) -> Option<&Arc<dyn Mount>> {
        self.mounts.get(name)
    }

    /// Lists all registered mounts.
    pub fn list_mounts(&self) -> Vec<&str> {
        self.mounts.keys().map(|s| s.as_str()).collect()
    }

    /// Returns the naming engine for external use.
    pub fn naming(&self) -> &NamingEngine {
        &self.naming
    }
}

/// Finds media files in a directory or returns the file if it's a single file.
///
/// For video media types, returns files sorted by size (largest first).
/// For audio media types, returns files sorted by name.
pub async fn find_media_files(path: &Path, media_type: MediaType) -> Result<Vec<PathBuf>> {
    let extensions: &[&str] = match media_type {
        MediaType::Movie | MediaType::Episode => VIDEO_EXTENSIONS,
        MediaType::Album | MediaType::Track => AUDIO_EXTENSIONS,
    };

    if path.is_file() {
        // Single file - check extension
        if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
            if extensions.iter().any(|e| e.eq_ignore_ascii_case(ext)) {
                return Ok(vec![path.to_path_buf()]);
            }
        }
        return Ok(Vec::new());
    }

    // Directory - walk and find matching files
    let mut files = Vec::new();
    let mut stack = vec![path.to_path_buf()];

    while let Some(current) = stack.pop() {
        let mut entries = tokio::fs::read_dir(&current).await.map_err(|e| {
            AppError::Internal(format!("Failed to read directory {:?}: {}", current, e))
        })?;

        while let Some(entry) = entries
            .next_entry()
            .await
            .map_err(|e| AppError::Internal(format!("Failed to read directory entry: {}", e)))?
        {
            let entry_path = entry.path();

            if entry_path.is_dir() {
                stack.push(entry_path);
            } else if let Some(ext) = entry_path.extension().and_then(|e| e.to_str()) {
                if extensions.iter().any(|e| e.eq_ignore_ascii_case(ext)) {
                    files.push(entry_path);
                }
            }
        }
    }

    // Sort by size (largest first) for video, by name for audio
    match media_type {
        MediaType::Movie | MediaType::Episode => {
            // Get sizes and sort by size descending
            let mut files_with_size: Vec<(PathBuf, u64)> = Vec::new();
            for file in files {
                let size = tokio::fs::metadata(&file)
                    .await
                    .map(|m| m.len())
                    .unwrap_or(0);
                files_with_size.push((file, size));
            }
            files_with_size.sort_by(|a, b| b.1.cmp(&a.1));
            Ok(files_with_size.into_iter().map(|(f, _)| f).collect())
        }
        MediaType::Album | MediaType::Track => {
            files.sort();
            Ok(files)
        }
    }
}

/// Recursively removes empty directories starting from the given path.
///
/// Walks up from the path, removing directories that become empty
/// after their contents are processed.
pub async fn cleanup_empty_dirs(path: &Path) -> Result<()> {
    if !path.is_dir() {
        return Ok(());
    }

    // Walk the directory tree depth-first
    let mut dirs_to_check = Vec::new();
    let mut stack = vec![path.to_path_buf()];

    while let Some(current) = stack.pop() {
        dirs_to_check.push(current.clone());

        if let Ok(mut entries) = tokio::fs::read_dir(&current).await {
            while let Ok(Some(entry)) = entries.next_entry().await {
                let entry_path = entry.path();
                if entry_path.is_dir() {
                    stack.push(entry_path);
                }
            }
        }
    }

    // Process directories in reverse order (deepest first)
    dirs_to_check.reverse();

    for dir in dirs_to_check {
        // Check if directory is empty
        let is_empty = match tokio::fs::read_dir(&dir).await {
            Ok(mut entries) => entries.next_entry().await.ok().flatten().is_none(),
            Err(_) => continue,
        };

        if is_empty {
            tracing::debug!(dir = ?dir, "Removing empty directory");
            if let Err(e) = tokio::fs::remove_dir(&dir).await {
                tracing::trace!(
                    dir = ?dir,
                    error = %e,
                    "Could not remove directory"
                );
            }
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    fn create_test_file(path: &Path, content: &str) {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        fs::write(path, content).unwrap();
    }

    #[tokio::test]
    async fn test_find_media_files_single_file() {
        let temp = TempDir::new().unwrap();
        let file_path = temp.path().join("movie.mkv");
        create_test_file(&file_path, "video content");

        let files = find_media_files(&file_path, MediaType::Movie)
            .await
            .unwrap();
        assert_eq!(files.len(), 1);
        assert_eq!(files[0], file_path);
    }

    #[tokio::test]
    async fn test_find_media_files_wrong_extension() {
        let temp = TempDir::new().unwrap();
        let file_path = temp.path().join("document.txt");
        create_test_file(&file_path, "text content");

        let files = find_media_files(&file_path, MediaType::Movie)
            .await
            .unwrap();
        assert!(files.is_empty());
    }

    #[tokio::test]
    async fn test_find_media_files_directory() {
        let temp = TempDir::new().unwrap();
        let video_path = temp.path().join("movie.mkv");
        let audio_path = temp.path().join("sample.mp3");
        let text_path = temp.path().join("readme.txt");

        create_test_file(&video_path, "video content larger file here");
        create_test_file(&audio_path, "audio");
        create_test_file(&text_path, "text");

        let video_files = find_media_files(temp.path(), MediaType::Movie)
            .await
            .unwrap();
        assert_eq!(video_files.len(), 1);
        assert_eq!(video_files[0], video_path);

        let audio_files = find_media_files(temp.path(), MediaType::Album)
            .await
            .unwrap();
        assert_eq!(audio_files.len(), 1);
        assert_eq!(audio_files[0], audio_path);
    }

    #[tokio::test]
    async fn test_find_media_files_nested_directory() {
        let temp = TempDir::new().unwrap();
        let nested_video = temp.path().join("subdir").join("movie.mp4");
        create_test_file(&nested_video, "video");

        let files = find_media_files(temp.path(), MediaType::Movie)
            .await
            .unwrap();
        assert_eq!(files.len(), 1);
        assert_eq!(files[0], nested_video);
    }

    #[tokio::test]
    async fn test_cleanup_empty_dirs() {
        let temp = TempDir::new().unwrap();
        let deep_dir = temp.path().join("a").join("b").join("c");
        fs::create_dir_all(&deep_dir).unwrap();

        // Put a file in the middle directory
        let file_path = temp.path().join("a").join("file.txt");
        create_test_file(&file_path, "content");

        cleanup_empty_dirs(temp.path()).await.unwrap();

        // 'c' and 'b' should be removed, but 'a' should remain (has file)
        assert!(!deep_dir.exists());
        assert!(!temp.path().join("a").join("b").exists());
        assert!(temp.path().join("a").exists());
    }

    #[tokio::test]
    async fn test_cleanup_empty_dirs_all_empty() {
        let temp = TempDir::new().unwrap();
        let deep_dir = temp.path().join("empty1").join("empty2").join("empty3");
        fs::create_dir_all(&deep_dir).unwrap();

        cleanup_empty_dirs(temp.path()).await.unwrap();

        // All nested empty dirs should be removed
        assert!(!temp.path().join("empty1").exists());
    }
}
