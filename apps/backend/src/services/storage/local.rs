//! Local filesystem mount implementation.
//!
//! Provides storage operations for local filesystem directories.

#![allow(dead_code)]

use async_trait::async_trait;
use std::path::{Component, Path, PathBuf};

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

    /// Validates that a path is safe and doesn't escape the mount root.
    ///
    /// Checks for:
    /// - Path traversal attempts using ".."
    /// - Symlinks that could escape the root
    ///
    /// Returns the full validated path if safe.
    async fn validate_path(&self, path: &Path) -> Result<PathBuf> {
        // Check for path traversal attempts
        for component in path.components() {
            if matches!(component, Component::ParentDir) {
                return Err(AppError::BadRequest(
                    "Path cannot contain parent directory references (..)".to_string(),
                ));
            }
        }

        let full_path = self.root.join(path);

        // Check each path component for symlinks that could escape root
        let mut current = self.root.clone();
        for component in path.components() {
            match component {
                Component::Normal(part) => {
                    current.push(part);
                    // Check if this path segment is a symlink
                    if let Ok(metadata) = tokio::fs::symlink_metadata(&current).await {
                        if metadata.is_symlink() {
                            // Resolve the symlink and check if it escapes root
                            if let Ok(resolved) = tokio::fs::canonicalize(&current).await {
                                let canonical_root =
                                    tokio::fs::canonicalize(&self.root).await.map_err(|e| {
                                        AppError::Internal(format!(
                                            "Failed to canonicalize root: {}",
                                            e
                                        ))
                                    })?;
                                if !resolved.starts_with(&canonical_root) {
                                    return Err(AppError::BadRequest(
                                        "Path contains symlink escaping mount root".to_string(),
                                    ));
                                }
                            }
                        }
                    }
                }
                Component::CurDir => {}
                _ => {
                    return Err(AppError::BadRequest(format!(
                        "Invalid path component: {:?}",
                        component
                    )));
                }
            }
        }

        Ok(full_path)
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
        // Use async metadata check instead of blocking is_dir()
        match tokio::fs::metadata(&self.root).await {
            Ok(metadata) => metadata.is_dir(),
            Err(_) => false,
        }
    }

    async fn free_space(&self) -> Result<u64> {
        // Use fs2 crate equivalent - statvfs on Unix
        #[cfg(unix)]
        {
            // Get filesystem stats using libc statvfs
            let path_cstr = std::ffi::CString::new(self.root.to_string_lossy().as_bytes())
                .map_err(|e| AppError::Internal(format!("Invalid path: {}", e)))?;

            // SAFETY: statvfs struct can be safely zero-initialized as it contains
            // only primitive integer types. The struct lifetime is contained within
            // this function and is only written to by the statvfs call.
            let mut stat: libc::statvfs = unsafe { std::mem::zeroed() };

            // SAFETY: path_cstr is a valid CString pointer that lives for the duration
            // of this call. stat is a valid mutable reference to a statvfs struct.
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
            // Use checked arithmetic to prevent overflow
            (stat.f_bavail as u64)
                .checked_mul(stat.f_frsize)
                .ok_or_else(|| AppError::Internal("Overflow calculating free space".to_string()))
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
        match self.validate_path(path).await {
            Ok(full_path) => tokio::fs::metadata(&full_path).await.is_ok(),
            Err(_) => false,
        }
    }

    async fn write_file(&self, source: &Path, dest: &Path) -> Result<()> {
        let full_dest = self.validate_path(dest).await?;

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
        let full_path = self.validate_path(path).await?;

        tokio::fs::remove_file(&full_path).await.map_err(|e| {
            AppError::Internal(format!("Failed to delete file {:?}: {}", full_path, e))
        })?;

        tracing::debug!(path = ?full_path, "File deleted");
        Ok(())
    }

    async fn create_dir_all(&self, path: &Path) -> Result<()> {
        let full_path = self.validate_path(path).await?;

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

    #[tokio::test]
    async fn test_path_traversal_prevention() {
        let (_temp, mount) = create_test_mount();

        // Should reject paths with parent directory traversal
        let result = mount
            .write_file(Path::new("/tmp/test"), Path::new("../escape"))
            .await;
        assert!(result.is_err());

        let result = mount
            .write_file(Path::new("/tmp/test"), Path::new("foo/../../bar"))
            .await;
        assert!(result.is_err());

        let result = mount.delete_file(Path::new("../etc/passwd")).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_path_traversal_in_exists() {
        let (_temp, mount) = create_test_mount();

        // Should return false for path traversal attempts (doesn't expose error)
        assert!(!mount.exists(Path::new("../escape")).await);
    }

    #[tokio::test]
    #[cfg(unix)]
    async fn test_symlink_escape_prevention() {
        let (temp, mount) = create_test_mount();

        // Create a symlink pointing outside the mount
        let external_dir = TempDir::new().unwrap();
        let external_file = external_dir.path().join("external.txt");
        fs::write(&external_file, "external content").unwrap();

        let symlink_path = temp.path().join("escape_link");
        std::os::unix::fs::symlink(external_dir.path(), &symlink_path).unwrap();

        // Should reject operations through symlink that escapes root
        let result = mount.exists(Path::new("escape_link/external.txt")).await;
        assert!(!result);
    }
}
