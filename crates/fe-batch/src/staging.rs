use crate::error::BatchError;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use tempfile::TempDir;

/// A temporary directory mirroring the project structure for pre-application work.
///
/// The staging area allows:
/// 1. Writing all changes to temp files before touching the working directory
/// 2. Running cross-file validation on the staged state
/// 3. Easy cleanup on failure (just drop the TempDir)
pub struct StagingArea {
    temp_dir: TempDir,
    staged_files: Vec<StagedFile>,
}

#[derive(Debug)]
pub struct StagedFile {
    pub relative_path: String,
    pub staged_path: PathBuf,
    pub is_new: bool,
    pub content: String,
}

impl StagingArea {
    pub fn new() -> Result<Self, BatchError> {
        let temp_dir = TempDir::new().map_err(BatchError::StagingError)?;
        Ok(Self {
            temp_dir,
            staged_files: Vec::new(),
        })
    }

    /// Stage an edit: write the new content to a temp file mirroring the relative path.
    pub fn stage_edit(&mut self, relative_path: &str, content: &str) -> Result<(), BatchError> {
        self.stage_file(relative_path, content, false)
    }

    /// Stage a create: write the new file content to a temp file.
    pub fn stage_create(&mut self, relative_path: &str, content: &str) -> Result<(), BatchError> {
        self.stage_file(relative_path, content, true)
    }

    fn stage_file(
        &mut self,
        relative_path: &str,
        content: &str,
        is_new: bool,
    ) -> Result<(), BatchError> {
        let staged_path = self.temp_dir.path().join(relative_path);

        // Create parent directories in the staging area
        if let Some(parent) = staged_path.parent() {
            fs::create_dir_all(parent).map_err(|e| BatchError::StagingError(e))?;
        }

        // Write content to staged file
        let mut file = fs::File::create(&staged_path).map_err(|e| BatchError::StagingError(e))?;
        file.write_all(content.as_bytes())
            .map_err(|e| BatchError::StagingError(e))?;

        self.staged_files.push(StagedFile {
            relative_path: relative_path.to_string(),
            staged_path,
            is_new,
            content: content.to_string(),
        });

        Ok(())
    }

    /// Get the staged content for a file.
    pub fn read_staged(&self, relative_path: &str) -> Option<&str> {
        self.staged_files
            .iter()
            .find(|f| f.relative_path == relative_path)
            .map(|f| f.content.as_str())
    }

    /// List all staged files.
    pub fn staged_files(&self) -> &[StagedFile] {
        &self.staged_files
    }

    /// Get the staging temp directory path (for debugging/testing).
    pub fn path(&self) -> &Path {
        self.temp_dir.path()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_staging_area_create_and_read() {
        let mut staging = StagingArea::new().unwrap();
        staging.stage_edit("file.ts", "export const x = 1;").unwrap();

        let content = staging.read_staged("file.ts").unwrap();
        assert_eq!(content, "export const x = 1;");
    }

    #[test]
    fn test_staging_area_multiple_files() {
        let mut staging = StagingArea::new().unwrap();
        staging.stage_edit("a.ts", "content a").unwrap();
        staging.stage_edit("b.ts", "content b").unwrap();
        staging.stage_create("c.ts", "content c").unwrap();

        assert_eq!(staging.staged_files().len(), 3);
        assert_eq!(staging.read_staged("a.ts").unwrap(), "content a");
        assert_eq!(staging.read_staged("b.ts").unwrap(), "content b");
        assert_eq!(staging.read_staged("c.ts").unwrap(), "content c");
    }

    #[test]
    fn test_staging_area_nested_paths() {
        let mut staging = StagingArea::new().unwrap();
        staging
            .stage_create("src/components/deep/Component.tsx", "export default function() {}")
            .unwrap();

        let staged = &staging.staged_files()[0];
        assert!(staged.staged_path.exists());
        assert!(staged.is_new);

        let on_disk = fs::read_to_string(&staged.staged_path).unwrap();
        assert_eq!(on_disk, "export default function() {}");
    }

    #[test]
    fn test_staging_area_cleanup_on_drop() {
        let staging = StagingArea::new().unwrap();
        let path = staging.path().to_path_buf();
        assert!(path.exists());
        drop(staging);
        assert!(!path.exists());
    }
}
