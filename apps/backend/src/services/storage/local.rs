//! Local filesystem mount implementation.
//!
//! Provides storage operations for local filesystem directories.

#![allow(dead_code)]

use async_trait::async_trait;
use std::path::{Path, PathBuf};

use crate::error::{AppError, Result};

use super::Mount;

/// Local filesystem mount.
///
/// Provides access to a directory on the local filesystem for storing media files.
pub struct LocalMount {
    name: String,
    root: PathBuf,
}

impl LocalMount {
    /// Creates a new local mount with the given name and root path.
    pub fn new(name: String, root: PathBuf) -> Self {
        Self { name, root }
    }
}

#[async_trait]
impl Mount for LocalMount {
    fn name(&self) -> &str {
        &self.name
    }

    fn mount_type(&self) -> &str {
        "local"
    }

    fn root(&self) -> &Path {
        &self.root
    }

    async fn available(&self) -> bool {
        self.root.exists() && self.root.is_dir()
    }

    async fn free_space(&self) -> Result<u64> {
        // Use fs2 crate equivalent - statvfs on Unix
        #[cfg(unix)]
        {
            // Get filesystem stats using libc statvfs
            let path_cstr = std::ffi::CString::new(self.root.to_string_lossy().as_bytes())
                .map_err(|e| AppError::Internal(format!("Invalid path: {}", e)))?;

            let mut stat: libc::statvfs = unsafe { std::mem::zeroed() };
            let result = unsafe { libc::statvfs(path_cstr.as_ptr(), &mut stat) };

            if result != 0 {
                let err = std::io::Error::last_os_error();
                return Err(AppError::Internal(format!(
                    "Failed to get filesystem stats for {:?}: {}",
                    self.root, err
                )));
            }

            // Free space = available blocks * block size
            // Use f_bavail (available to non-root) rather than f_bfree
            let free_bytes = stat.f_bavail as u64 * stat.f_frsize;
            Ok(free_bytes)
        }

        #[cfg(not(unix))]
        {
            // For non-Unix platforms, return a large value as a fallback
            // In production, you'd use platform-specific APIs
            tracing::warn!("free_space not implemented for this platform, returning placeholder");
            Ok(u64::MAX)
        }
    }

    async fn exists(&self, path: &Path) -> bool {
        self.root.join(path).exists()
    }

    async fn write_file(&self, source: &Path, dest: &Path) -> Result<()> {
        let full_dest = self.root.join(dest);

        // Create parent directories
        if let Some(parent) = full_dest.parent() {
            tokio::fs::create_dir_all(parent).await.map_err(|e| {
                AppError::Internal(format!("Failed to create directory {:?}: {}", parent, e))
            })?;
        }

        // Copy file
        tokio::fs::copy(source, &full_dest).await.map_err(|e| {
            AppError::Internal(format!(
                "Failed to copy file from {:?} to {:?}: {}",
                source, full_dest, e
            ))
        })?;

        tracing::debug!(
            source = ?source,
            dest = ?full_dest,
            "File written successfully"
        );

        Ok(())
    }

    async fn delete_file(&self, path: &Path) -> Result<()> {
        let full_path = self.root.join(path);

        tokio::fs::remove_file(&full_path).await.map_err(|e| {
            AppError::Internal(format!("Failed to delete file {:?}: {}", full_path, e))
        })?;

        tracing::debug!(path = ?full_path, "File deleted");
        Ok(())
    }

    async fn create_dir_all(&self, path: &Path) -> Result<()> {
        let full_path = self.root.join(path);

        tokio::fs::create_dir_all(&full_path).await.map_err(|e| {
            AppError::Internal(format!("Failed to create directory {:?}: {}", full_path, e))
        })?;

        tracing::debug!(path = ?full_path, "Directory created");
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    fn create_test_mount() -> (TempDir, LocalMount) {
        let temp = TempDir::new().unwrap();
        let mount = LocalMount::new("test".to_string(), temp.path().to_path_buf());
        (temp, mount)
    }

    #[tokio::test]
    async fn test_local_mount_name() {
        let (_temp, mount) = create_test_mount();
        assert_eq!(mount.name(), "test");
        assert_eq!(mount.mount_type(), "local");
    }

    #[tokio::test]
    async fn test_local_mount_available() {
        let (_temp, mount) = create_test_mount();
        assert!(mount.available().await);
    }

    #[tokio::test]
    async fn test_local_mount_available_nonexistent() {
        let mount = LocalMount::new("test".to_string(), PathBuf::from("/nonexistent/path"));
        assert!(!mount.available().await);
    }

    #[tokio::test]
    async fn test_local_mount_exists() {
        let (temp, mount) = create_test_mount();
        let file_path = temp.path().join("test.txt");
        fs::write(&file_path, "content").unwrap();

        assert!(mount.exists(Path::new("test.txt")).await);
        assert!(!mount.exists(Path::new("nonexistent.txt")).await);
    }

    #[tokio::test]
    async fn test_local_mount_write_file() {
        let (temp, mount) = create_test_mount();

        // Create source file
        let source_temp = TempDir::new().unwrap();
        let source_path = source_temp.path().join("source.txt");
        fs::write(&source_path, "test content").unwrap();

        // Write to mount
        mount
            .write_file(&source_path, Path::new("subdir/dest.txt"))
            .await
            .unwrap();

        // Verify file exists and has correct content
        let dest_path = temp.path().join("subdir/dest.txt");
        assert!(dest_path.exists());
        assert_eq!(fs::read_to_string(&dest_path).unwrap(), "test content");
    }

    #[tokio::test]
    async fn test_local_mount_delete_file() {
        let (temp, mount) = create_test_mount();

        // Create file
        let file_path = temp.path().join("to_delete.txt");
        fs::write(&file_path, "content").unwrap();
        assert!(file_path.exists());

        // Delete file
        mount.delete_file(Path::new("to_delete.txt")).await.unwrap();
        assert!(!file_path.exists());
    }

    #[tokio::test]
    async fn test_local_mount_create_dir_all() {
        let (temp, mount) = create_test_mount();

        mount.create_dir_all(Path::new("a/b/c")).await.unwrap();

        let dir_path = temp.path().join("a/b/c");
        assert!(dir_path.exists());
        assert!(dir_path.is_dir());
    }

    #[tokio::test]
    #[cfg(unix)]
    async fn test_local_mount_free_space() {
        let (_temp, mount) = create_test_mount();

        let free = mount.free_space().await.unwrap();
        // Should return some positive value
        assert!(free > 0);
    }
}
