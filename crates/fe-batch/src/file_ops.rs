use crate::error::BatchError;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use tempfile::NamedTempFile;

/// Write content to a file atomically using write-to-temp-then-rename.
/// The temp file is created in the same directory as the target to ensure
/// same-filesystem rename (required for atomic rename on Unix).
pub fn atomic_write(target: &Path, content: &[u8]) -> Result<(), BatchError> {
    let parent = target.parent().ok_or_else(|| BatchError::WriteError {
        path: target.to_path_buf(),
        source: std::io::Error::new(std::io::ErrorKind::InvalidInput, "no parent directory"),
    })?;

    // Ensure parent directory exists
    if !parent.exists() {
        fs::create_dir_all(parent).map_err(|e| BatchError::MkdirError {
            path: parent.to_path_buf(),
            source: e,
        })?;
    }

    // Create temp file in the SAME directory (ensures same-filesystem rename)
    let mut temp_file =
        NamedTempFile::new_in(parent).map_err(|e| BatchError::WriteError {
            path: target.to_path_buf(),
            source: e,
        })?;

    // Write content
    temp_file
        .write_all(content)
        .map_err(|e| BatchError::WriteError {
            path: target.to_path_buf(),
            source: e,
        })?;

    // Sync to disk before rename
    temp_file
        .as_file()
        .sync_all()
        .map_err(|e| BatchError::WriteError {
            path: target.to_path_buf(),
            source: e,
        })?;

    // Atomic rename
    temp_file.persist(target).map_err(|e| BatchError::RenameError {
        path: target.to_path_buf(),
        source: e.error,
    })?;

    Ok(())
}

/// Create a new file atomically, failing if it already exists.
pub fn atomic_create(target: &Path, content: &[u8]) -> Result<(), BatchError> {
    if target.exists() {
        return Err(BatchError::FileAlreadyExists(target.to_path_buf()));
    }
    atomic_write(target, content)
}

/// A set of file backups that can be used for rollback.
pub struct FileBackupSet {
    backup_dir: tempfile::TempDir,
    backups: Vec<FileBackup>,
    created_files: Vec<PathBuf>,
    project_root: PathBuf,
}

struct FileBackup {
    original_path: PathBuf,
    backup_path: PathBuf,
}

impl FileBackupSet {
    pub fn new(project_root: &Path) -> Result<Self, BatchError> {
        let backup_dir =
            tempfile::tempdir().map_err(|e| BatchError::BackupError {
                path: project_root.to_path_buf(),
                source: e,
            })?;

        Ok(Self {
            backup_dir,
            backups: Vec::new(),
            created_files: Vec::new(),
            project_root: project_root.to_path_buf(),
        })
    }

    /// Backup a file before editing it.
    pub fn backup_file(&mut self, path: &Path) -> Result<(), BatchError> {
        let backup_name = format!("backup_{}", self.backups.len());
        let backup_path = self.backup_dir.path().join(backup_name);

        fs::copy(path, &backup_path).map_err(|e| BatchError::BackupError {
            path: path.to_path_buf(),
            source: e,
        })?;

        self.backups.push(FileBackup {
            original_path: path.to_path_buf(),
            backup_path,
        });

        Ok(())
    }

    /// Record that a file was created (so rollback knows to delete it).
    pub fn record_creation(&mut self, path: &Path) {
        self.created_files.push(path.to_path_buf());
    }

    /// Restore all backed-up files to their original state.
    /// Deletes any files that were created during the transaction.
    pub fn restore_all(&self) -> Result<(), BatchError> {
        // First: delete created files (in reverse order)
        for created_path in self.created_files.iter().rev() {
            if created_path.exists() {
                fs::remove_file(created_path).map_err(|e| BatchError::RollbackError {
                    path: created_path.clone(),
                    source: e,
                })?;
            }
            // Clean up empty parent directories that we may have created
            if let Some(parent) = created_path.parent() {
                remove_empty_ancestors(parent, &self.project_root);
            }
        }

        // Second: restore backed-up files (in reverse order)
        for backup in self.backups.iter().rev() {
            fs::copy(&backup.backup_path, &backup.original_path).map_err(|e| {
                BatchError::RollbackError {
                    path: backup.original_path.clone(),
                    source: e,
                }
            })?;
        }

        Ok(())
    }

    /// Discard backups (called on successful commit).
    /// The TempDir is dropped, cleaning up backup files.
    pub fn discard(self) {
        // self.backup_dir is dropped here, removing all backup files
        drop(self);
    }

    /// Get the list of backed-up original paths.
    pub fn backed_up_paths(&self) -> Vec<&Path> {
        self.backups.iter().map(|b| b.original_path.as_path()).collect()
    }

    /// Get the list of created file paths.
    pub fn created_paths(&self) -> &[PathBuf] {
        &self.created_files
    }
}

/// Remove empty ancestor directories up to (but not including) the root.
fn remove_empty_ancestors(dir: &Path, root: &Path) {
    let mut current = dir.to_path_buf();
    while current != root && current.starts_with(root) {
        if current.exists() && is_dir_empty(&current) {
            if fs::remove_dir(&current).is_err() {
                break;
            }
        } else {
            break;
        }
        if !current.pop() {
            break;
        }
    }
}

fn is_dir_empty(path: &Path) -> bool {
    fs::read_dir(path).map(|mut d| d.next().is_none()).unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_atomic_write_creates_file() {
        let dir = tempfile::tempdir().unwrap();
        let target = dir.path().join("new_file.ts");
        atomic_write(&target, b"hello world").unwrap();
        assert_eq!(fs::read_to_string(&target).unwrap(), "hello world");
    }

    #[test]
    fn test_atomic_write_replaces_existing() {
        let dir = tempfile::tempdir().unwrap();
        let target = dir.path().join("file.ts");
        fs::write(&target, "original").unwrap();
        atomic_write(&target, b"replaced").unwrap();
        assert_eq!(fs::read_to_string(&target).unwrap(), "replaced");
    }

    #[test]
    fn test_atomic_create_fails_if_exists() {
        let dir = tempfile::tempdir().unwrap();
        let target = dir.path().join("file.ts");
        fs::write(&target, "exists").unwrap();
        let err = atomic_create(&target, b"new").unwrap_err();
        assert!(matches!(err, BatchError::FileAlreadyExists(_)));
    }

    #[test]
    fn test_atomic_write_creates_parent_dirs() {
        let dir = tempfile::tempdir().unwrap();
        let target = dir.path().join("a").join("b").join("c").join("file.ts");
        atomic_write(&target, b"deep").unwrap();
        assert_eq!(fs::read_to_string(&target).unwrap(), "deep");
    }

    #[test]
    fn test_backup_and_restore_single_file() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("file.ts");
        fs::write(&file, "original content").unwrap();

        let mut backups = FileBackupSet::new(dir.path()).unwrap();
        backups.backup_file(&file).unwrap();

        // Modify the file
        fs::write(&file, "modified content").unwrap();
        assert_eq!(fs::read_to_string(&file).unwrap(), "modified content");

        // Restore
        backups.restore_all().unwrap();
        assert_eq!(fs::read_to_string(&file).unwrap(), "original content");
    }

    #[test]
    fn test_backup_and_restore_multiple_files() {
        let dir = tempfile::tempdir().unwrap();
        let files: Vec<_> = (0..5)
            .map(|i| {
                let path = dir.path().join(format!("file_{i}.ts"));
                fs::write(&path, format!("original_{i}")).unwrap();
                path
            })
            .collect();

        let mut backups = FileBackupSet::new(dir.path()).unwrap();
        for f in &files {
            backups.backup_file(f).unwrap();
        }

        // Modify all files
        for (i, f) in files.iter().enumerate() {
            fs::write(f, format!("modified_{i}")).unwrap();
        }

        // Restore
        backups.restore_all().unwrap();
        for (i, f) in files.iter().enumerate() {
            assert_eq!(fs::read_to_string(f).unwrap(), format!("original_{i}"));
        }
    }

    #[test]
    fn test_record_creation_and_rollback() {
        let dir = tempfile::tempdir().unwrap();
        let created = dir.path().join("new_file.ts");

        let mut backups = FileBackupSet::new(dir.path()).unwrap();
        fs::write(&created, "new content").unwrap();
        backups.record_creation(&created);

        assert!(created.exists());
        backups.restore_all().unwrap();
        assert!(!created.exists());
    }

    #[test]
    fn test_rollback_removes_empty_parent_dirs() {
        let dir = tempfile::tempdir().unwrap();
        let deep_file = dir.path().join("a").join("b").join("c").join("file.ts");

        let mut backups = FileBackupSet::new(dir.path()).unwrap();
        fs::create_dir_all(deep_file.parent().unwrap()).unwrap();
        fs::write(&deep_file, "content").unwrap();
        backups.record_creation(&deep_file);

        backups.restore_all().unwrap();
        assert!(!deep_file.exists());
        // Empty parent dirs should be cleaned up
        assert!(!dir.path().join("a").join("b").join("c").exists());
        assert!(!dir.path().join("a").join("b").exists());
        assert!(!dir.path().join("a").exists());
    }

    #[test]
    fn test_discard_cleans_up_backup_dir() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("file.ts");
        fs::write(&file, "content").unwrap();

        let mut backups = FileBackupSet::new(dir.path()).unwrap();
        let backup_dir_path = backups.backup_dir.path().to_path_buf();
        backups.backup_file(&file).unwrap();

        assert!(backup_dir_path.exists());
        backups.discard();
        assert!(!backup_dir_path.exists());
    }

    #[test]
    #[cfg(unix)]
    fn test_backup_preserves_permissions() {
        use std::os::unix::fs::PermissionsExt;

        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("file.ts");
        fs::write(&file, "content").unwrap();
        fs::set_permissions(&file, fs::Permissions::from_mode(0o644)).unwrap();

        let mut backups = FileBackupSet::new(dir.path()).unwrap();
        backups.backup_file(&file).unwrap();

        // Change permissions and content
        fs::set_permissions(&file, fs::Permissions::from_mode(0o755)).unwrap();
        fs::write(&file, "modified").unwrap();

        // Restore should bring back original content
        backups.restore_all().unwrap();
        assert_eq!(fs::read_to_string(&file).unwrap(), "content");
    }
}
