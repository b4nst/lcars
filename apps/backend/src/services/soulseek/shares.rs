//! Share index management for Soulseek file sharing.
//!
//! This module handles scanning directories, indexing shared files,
//! and extracting audio metadata for matching search queries.

use lofty::prelude::*;
use lofty::probe::Probe;
use soulseek_protocol::peers::p2p::shared_directories::{
    Attribute, Directory, File as ProtocolFile, SharedDirectories,
};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::Instant;
use walkdir::WalkDir;

use crate::error::Result;

/// Audio file attributes extracted from metadata.
#[derive(Debug, Clone, Default)]
pub struct FileAttributes {
    /// Bitrate in kbps.
    pub bitrate: Option<u32>,
    /// Duration in seconds.
    pub duration: Option<u32>,
    /// Sample rate in Hz.
    pub sample_rate: Option<u32>,
    /// Bit depth.
    pub bit_depth: Option<u32>,
    /// Whether the file uses variable bitrate.
    pub vbr: Option<bool>,
}

impl FileAttributes {
    /// Convert to protocol attributes for Soulseek messages.
    pub fn to_protocol_attributes(&self) -> Vec<Attribute> {
        let mut attrs = Vec::new();

        // Attribute places follow Soulseek protocol:
        // 0 = bitrate, 1 = duration, 2 = VBR, 4 = sample rate, 5 = bit depth
        if let Some(bitrate) = self.bitrate {
            attrs.push(Attribute {
                place: 0,
                attribute: bitrate,
            });
        }
        if let Some(duration) = self.duration {
            attrs.push(Attribute {
                place: 1,
                attribute: duration,
            });
        }
        if let Some(vbr) = self.vbr {
            attrs.push(Attribute {
                place: 2,
                attribute: if vbr { 1 } else { 0 },
            });
        }
        if let Some(sample_rate) = self.sample_rate {
            attrs.push(Attribute {
                place: 4,
                attribute: sample_rate,
            });
        }
        if let Some(bit_depth) = self.bit_depth {
            attrs.push(Attribute {
                place: 5,
                attribute: bit_depth,
            });
        }

        attrs
    }
}

/// A file in the share index.
#[derive(Debug, Clone)]
pub struct SharedFile {
    /// Absolute path on disk.
    pub path: PathBuf,
    /// Path as seen by peers (uses backslashes for Soulseek compatibility).
    pub virtual_path: String,
    /// File size in bytes.
    pub size: u64,
    /// File extension (lowercase).
    pub extension: String,
    /// Audio attributes if available.
    pub attributes: FileAttributes,
}

/// Statistics about the share index.
#[derive(Debug, Clone)]
pub struct ShareStats {
    /// Total number of shared files.
    pub total_files: u64,
    /// Total number of shared folders.
    pub total_folders: u64,
    /// When the index was last updated.
    pub last_indexed: Option<Instant>,
    /// Total size of all shared files in bytes.
    pub total_size: u64,
}

/// Index of all shared files.
#[derive(Debug)]
pub struct ShareIndex {
    /// Files grouped by directory.
    directories: HashMap<String, Vec<SharedFile>>,
    /// All files for quick lookup by virtual path.
    files_by_path: HashMap<String, SharedFile>,
    /// Statistics.
    stats: ShareStats,
}

impl Default for ShareIndex {
    fn default() -> Self {
        Self::new()
    }
}

impl ShareIndex {
    /// Create an empty share index.
    pub fn new() -> Self {
        Self {
            directories: HashMap::new(),
            files_by_path: HashMap::new(),
            stats: ShareStats {
                total_files: 0,
                total_folders: 0,
                last_indexed: None,
                total_size: 0,
            },
        }
    }

    /// Scan directories and build the share index.
    ///
    /// This is a potentially long-running operation that scans all files
    /// in the specified directories and extracts metadata.
    pub async fn scan(dirs: &[PathBuf], include_hidden: bool) -> Result<Self> {
        let mut index = ShareIndex::new();
        let mut total_size: u64 = 0;

        for base_dir in dirs {
            if !base_dir.exists() {
                tracing::warn!(path = ?base_dir, "Share directory does not exist, skipping");
                continue;
            }

            let base_dir_name = base_dir
                .file_name()
                .map(|s| s.to_string_lossy().to_string())
                .unwrap_or_else(|| "shares".to_string());

            let walker = WalkDir::new(base_dir)
                .follow_links(false)
                .into_iter()
                .filter_entry(|e| {
                    if include_hidden {
                        true
                    } else {
                        // Don't filter the root entry (depth 0), only filter hidden files/dirs within
                        e.depth() == 0
                            || !e
                                .file_name()
                                .to_str()
                                .map(|s| s.starts_with('.'))
                                .unwrap_or(false)
                    }
                });

            for entry in walker.filter_map(|e| e.ok()) {
                let path = entry.path();

                if path.is_file() {
                    if let Some(shared_file) =
                        Self::process_file(path, base_dir, &base_dir_name).await
                    {
                        total_size += shared_file.size;

                        // Get directory path for grouping
                        let dir_path = path
                            .parent()
                            .map(|p| Self::make_virtual_path(p, base_dir, &base_dir_name))
                            .unwrap_or_else(|| base_dir_name.clone());

                        // Store file
                        index
                            .files_by_path
                            .insert(shared_file.virtual_path.clone(), shared_file.clone());

                        index
                            .directories
                            .entry(dir_path)
                            .or_default()
                            .push(shared_file);
                    }
                }
            }
        }

        index.stats = ShareStats {
            total_files: index.files_by_path.len() as u64,
            total_folders: index.directories.len() as u64,
            last_indexed: Some(Instant::now()),
            total_size,
        };

        tracing::info!(
            files = index.stats.total_files,
            folders = index.stats.total_folders,
            size_mb = total_size / 1024 / 1024,
            "Share index scan complete"
        );

        Ok(index)
    }

    /// Process a single file and extract its metadata.
    async fn process_file(path: &Path, base_dir: &Path, base_dir_name: &str) -> Option<SharedFile> {
        let metadata = tokio::fs::metadata(path).await.ok()?;
        let size = metadata.len();

        let extension = path
            .extension()
            .and_then(|e| e.to_str())
            .map(|e| e.to_lowercase())
            .unwrap_or_default();

        let virtual_path = Self::make_virtual_path(path, base_dir, base_dir_name);

        // Extract audio attributes for supported formats
        let attributes = if Self::is_audio_file(&extension) {
            Self::extract_audio_attributes(path)
        } else {
            FileAttributes::default()
        };

        Some(SharedFile {
            path: path.to_path_buf(),
            virtual_path,
            size,
            extension,
            attributes,
        })
    }

    /// Create a virtual path for Soulseek (uses backslashes).
    fn make_virtual_path(path: &Path, base_dir: &Path, base_dir_name: &str) -> String {
        let relative = path.strip_prefix(base_dir).unwrap_or(path);

        // Soulseek uses backslashes in paths
        let relative_str = relative.to_string_lossy().replace('/', "\\");

        if relative_str.is_empty() {
            base_dir_name.to_string()
        } else {
            format!("{}\\{}", base_dir_name, relative_str)
        }
    }

    /// Check if a file extension is a supported audio format.
    fn is_audio_file(extension: &str) -> bool {
        matches!(
            extension,
            "mp3" | "flac" | "ogg" | "m4a" | "aac" | "wav" | "wma" | "ape" | "opus" | "aiff"
        )
    }

    /// Extract audio attributes from a file using lofty.
    fn extract_audio_attributes(path: &Path) -> FileAttributes {
        let probe_result = match Probe::open(path) {
            Ok(probe) => probe.read(),
            Err(e) => {
                tracing::trace!(path = ?path, error = %e, "Failed to probe audio file");
                return FileAttributes::default();
            }
        };

        let tagged_file = match probe_result {
            Ok(f) => f,
            Err(e) => {
                tracing::trace!(path = ?path, error = %e, "Failed to read audio metadata");
                return FileAttributes::default();
            }
        };

        let properties = tagged_file.properties();

        FileAttributes {
            bitrate: Some(properties.audio_bitrate().unwrap_or(0)),
            duration: Some(properties.duration().as_secs() as u32),
            sample_rate: properties.sample_rate(),
            bit_depth: properties.bit_depth().map(|b| b as u32),
            vbr: None, // lofty doesn't directly expose VBR info
        }
    }

    /// Get a file by its virtual path.
    pub fn get_file(&self, virtual_path: &str) -> Option<&SharedFile> {
        // Normalize path separators
        let normalized = virtual_path.replace('/', "\\");
        self.files_by_path.get(&normalized)
    }

    /// Search for files matching a query.
    ///
    /// The query is split into terms and each term must be present in the filename.
    pub fn search(&self, query: &str) -> Vec<&SharedFile> {
        let terms: Vec<String> = query
            .to_lowercase()
            .split_whitespace()
            .map(String::from)
            .collect();

        if terms.is_empty() {
            return Vec::new();
        }

        self.files_by_path
            .values()
            .filter(|file| {
                let path_lower = file.virtual_path.to_lowercase();
                terms.iter().all(|term| path_lower.contains(term))
            })
            .collect()
    }

    /// Convert the index to protocol SharedDirectories format.
    pub fn to_protocol_directories(&self) -> SharedDirectories {
        let dirs: Vec<Directory> = self
            .directories
            .iter()
            .map(|(dir_name, files)| {
                let protocol_files: Vec<ProtocolFile> = files
                    .iter()
                    .map(|f| {
                        // Get filename from virtual path
                        let name = f
                            .virtual_path
                            .rsplit('\\')
                            .next()
                            .unwrap_or(&f.virtual_path)
                            .to_string();

                        ProtocolFile {
                            name,
                            size: f.size,
                            extension: f.extension.clone(),
                            attributes: f.attributes.to_protocol_attributes(),
                        }
                    })
                    .collect();

                Directory {
                    name: dir_name.clone(),
                    files: protocol_files,
                }
            })
            .collect();

        SharedDirectories { dirs }
    }

    /// Get share statistics.
    pub fn stats(&self) -> &ShareStats {
        &self.stats
    }

    /// Get the number of shared files.
    pub fn file_count(&self) -> u64 {
        self.stats.total_files
    }

    /// Get the number of shared folders.
    pub fn folder_count(&self) -> u64 {
        self.stats.total_folders
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_empty_scan() {
        let temp_dir = TempDir::new().unwrap();
        let index = ShareIndex::scan(&[temp_dir.path().to_path_buf()], false)
            .await
            .unwrap();

        assert_eq!(index.file_count(), 0);
        assert_eq!(index.folder_count(), 0);
    }

    #[tokio::test]
    async fn test_scan_with_files() {
        let temp_dir = TempDir::new().unwrap();

        // Create test files using synchronous fs since walkdir is synchronous
        fs::write(temp_dir.path().join("test.txt"), b"hello").unwrap();
        fs::write(temp_dir.path().join("music.mp3"), b"fake mp3").unwrap();

        let index = ShareIndex::scan(&[temp_dir.path().to_path_buf()], false)
            .await
            .unwrap();

        assert_eq!(index.file_count(), 2);
        assert_eq!(index.folder_count(), 1);
    }

    #[tokio::test]
    async fn test_search() {
        let temp_dir = TempDir::new().unwrap();

        fs::write(temp_dir.path().join("artist_album_song.mp3"), b"fake").unwrap();
        fs::write(temp_dir.path().join("other_file.txt"), b"other").unwrap();

        let index = ShareIndex::scan(&[temp_dir.path().to_path_buf()], false)
            .await
            .unwrap();

        let results = index.search("artist song");
        assert_eq!(results.len(), 1);
        assert!(results[0].virtual_path.contains("artist_album_song"));

        let results = index.search("nonexistent");
        assert_eq!(results.len(), 0);
    }

    #[tokio::test]
    async fn test_hidden_files() {
        let temp_dir = TempDir::new().unwrap();

        fs::write(temp_dir.path().join("visible.txt"), b"visible").unwrap();
        fs::write(temp_dir.path().join(".hidden"), b"hidden").unwrap();

        // Without hidden files
        let index = ShareIndex::scan(&[temp_dir.path().to_path_buf()], false)
            .await
            .unwrap();
        assert_eq!(index.file_count(), 1);

        // With hidden files
        let index = ShareIndex::scan(&[temp_dir.path().to_path_buf()], true)
            .await
            .unwrap();
        assert_eq!(index.file_count(), 2);
    }

    #[test]
    fn test_virtual_path() {
        let base = PathBuf::from("/home/user/music");
        let file = PathBuf::from("/home/user/music/Artist/Album/song.mp3");

        let virtual_path = ShareIndex::make_virtual_path(&file, &base, "music");
        assert_eq!(virtual_path, "music\\Artist\\Album\\song.mp3");
    }

    #[test]
    fn test_is_audio_file() {
        assert!(ShareIndex::is_audio_file("mp3"));
        assert!(ShareIndex::is_audio_file("flac"));
        assert!(ShareIndex::is_audio_file("ogg"));
        assert!(!ShareIndex::is_audio_file("txt"));
        assert!(!ShareIndex::is_audio_file("pdf"));
    }
}
