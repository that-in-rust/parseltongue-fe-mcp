use crate::edit_set::{EditChange, ValidatedCreate, ValidatedEdit};
use crate::error::BatchError;
use crate::file_ops::{atomic_create, atomic_write, FileBackupSet};
use crate::staging::StagingArea;
use crate::types::{BatchErrorDetail, BatchInput, BatchResult, BatchStatus};
use fe_verify::types::VerificationSummary;
use std::marker::PhantomData;
use std::path::{Path, PathBuf};

// ── Typestate markers ──────────────────────────────────────────────

/// Transaction has been created and inputs validated, but nothing is staged yet.
pub struct Pending;

/// Changes have been written to the staging area and cross-validated.
pub struct Staged;

/// Changes have been applied to the working directory (backups exist).
pub struct Applied;

/// Transaction committed successfully — backups discarded.
pub struct Committed;

/// Transaction rolled back — files restored from backups.
pub struct RolledBack;

// ── Transaction ────────────────────────────────────────────────────

pub struct Transaction<State = Pending> {
    project_root: PathBuf,
    edits: Vec<ValidatedEdit>,
    creates: Vec<ValidatedCreate>,
    verify: bool,
    rollback_on_failure: bool,
    staging: Option<StagingArea>,
    backups: Option<FileBackupSet>,
    _state: PhantomData<State>,
}

// ── Pending → Staged ───────────────────────────────────────────────

impl Transaction<Pending> {
    /// Create a new transaction from BatchInput.
    /// Validates all file paths, checks for conflicts.
    pub fn new(project_root: PathBuf, input: BatchInput) -> Result<Self, BatchError> {
        let verify = input.verify_enabled();
        let rollback_on_failure = input.rollback_on_failure();

        let (edits, creates) = crate::edit_set::validate_input(&project_root, &input)?;

        Ok(Transaction {
            project_root,
            edits,
            creates,
            verify,
            rollback_on_failure,
            staging: None,
            backups: None,
            _state: PhantomData,
        })
    }

    /// Stage changes: write to a shadow directory.
    pub fn stage(self) -> Result<Transaction<Staged>, BatchError> {
        let mut staging = StagingArea::new()?;

        for edit in &self.edits {
            match &edit.change {
                EditChange::FullContent(content) => {
                    staging.stage_edit(&edit.relative_path, content)?;
                }
                EditChange::AstOperations(_ops) => {
                    // Phase 5: read original, apply AST ops, stage result
                    return Err(BatchError::Internal(
                        "AST operations not yet implemented. Use 'content' field instead."
                            .to_string(),
                    ));
                }
            }
        }

        for create in &self.creates {
            staging.stage_create(&create.relative_path, &create.content)?;
        }

        Ok(Transaction {
            project_root: self.project_root,
            edits: self.edits,
            creates: self.creates,
            verify: self.verify,
            rollback_on_failure: self.rollback_on_failure,
            staging: Some(staging),
            backups: None,
            _state: PhantomData,
        })
    }
}

// ── Staged → Applied ───────────────────────────────────────────────

impl Transaction<Staged> {
    /// Apply staged changes to the working directory.
    /// Creates backups of all affected files first.
    pub fn apply(self) -> Result<Transaction<Applied>, BatchError> {
        let mut backups = FileBackupSet::new(&self.project_root)?;
        let staging = self.staging.as_ref().expect("staging must exist in Staged state");

        // Backup all files that will be edited
        for edit in &self.edits {
            backups.backup_file(&edit.absolute_path)?;
        }

        // Apply edits from staging
        for edit in &self.edits {
            let staged_content = staging
                .read_staged(&edit.relative_path)
                .ok_or_else(|| BatchError::Internal(format!(
                    "Staged content missing for {}",
                    edit.relative_path
                )))?;

            if let Err(e) = atomic_write(&edit.absolute_path, staged_content.as_bytes()) {
                // Rollback the files we've already written
                tracing::error!("Write failed for {}, initiating rollback: {e}", edit.relative_path);
                let _ = backups.restore_all();
                return Err(e);
            }
        }

        // Apply creates from staging
        for create in &self.creates {
            let staged_content = staging
                .read_staged(&create.relative_path)
                .ok_or_else(|| BatchError::Internal(format!(
                    "Staged content missing for {}",
                    create.relative_path
                )))?;

            if let Err(e) = atomic_create(&create.absolute_path, staged_content.as_bytes()) {
                // Rollback everything
                tracing::error!("Create failed for {}, initiating rollback: {e}", create.relative_path);
                let _ = backups.restore_all();
                return Err(e);
            }
            backups.record_creation(&create.absolute_path);
        }

        Ok(Transaction {
            project_root: self.project_root,
            edits: self.edits,
            creates: self.creates,
            verify: self.verify,
            rollback_on_failure: self.rollback_on_failure,
            staging: self.staging,
            backups: Some(backups),
            _state: PhantomData,
        })
    }
}

// ── Applied → Committed | RolledBack ───────────────────────────────

impl Transaction<Applied> {
    /// Whether verification was requested.
    pub fn verify_enabled(&self) -> bool {
        self.verify
    }

    /// Whether rollback on failure was requested.
    pub fn rollback_on_failure(&self) -> bool {
        self.rollback_on_failure
    }

    /// Get the project root.
    pub fn project_root(&self) -> &Path {
        &self.project_root
    }

    /// Get affected file paths (for verification).
    pub fn affected_files(&self) -> Vec<PathBuf> {
        let mut files = Vec::new();
        for edit in &self.edits {
            files.push(edit.absolute_path.clone());
        }
        for create in &self.creates {
            files.push(create.absolute_path.clone());
        }
        files
    }

    /// Commit: discard backups, clean staging area.
    pub fn commit(self) -> Transaction<Committed> {
        if let Some(backups) = self.backups {
            backups.discard();
        }
        Transaction {
            project_root: self.project_root,
            edits: self.edits,
            creates: self.creates,
            verify: self.verify,
            rollback_on_failure: self.rollback_on_failure,
            staging: None,
            backups: None,
            _state: PhantomData,
        }
    }

    /// Rollback: restore all files from backups, remove created files.
    pub fn rollback(self) -> Result<Transaction<RolledBack>, BatchError> {
        if let Some(ref backups) = self.backups {
            backups.restore_all()?;
        }
        Ok(Transaction {
            project_root: self.project_root,
            edits: self.edits,
            creates: self.creates,
            verify: self.verify,
            rollback_on_failure: self.rollback_on_failure,
            staging: None,
            backups: None,
            _state: PhantomData,
        })
    }
}

// ── Result builders ────────────────────────────────────────────────

impl Transaction<Committed> {
    pub fn into_result(self, verification: Option<VerificationSummary>) -> BatchResult {
        BatchResult {
            status: BatchStatus::Success,
            files_modified: self.edits.iter().map(|e| e.relative_path.clone()).collect(),
            files_created: self.creates.iter().map(|c| c.relative_path.clone()).collect(),
            verification,
            errors: Vec::new(),
            rolled_back: false,
        }
    }

    pub fn into_result_with_warnings(
        self,
        verification: Option<VerificationSummary>,
    ) -> BatchResult {
        BatchResult {
            status: BatchStatus::VerificationFailed,
            files_modified: self.edits.iter().map(|e| e.relative_path.clone()).collect(),
            files_created: self.creates.iter().map(|c| c.relative_path.clone()).collect(),
            verification,
            errors: Vec::new(),
            rolled_back: false,
        }
    }
}

impl Transaction<RolledBack> {
    pub fn into_result(self, verification: Option<VerificationSummary>) -> BatchResult {
        BatchResult {
            status: BatchStatus::RolledBack,
            files_modified: Vec::new(),
            files_created: Vec::new(),
            verification,
            errors: Vec::new(),
            rolled_back: true,
        }
    }

    pub fn into_error_result(self, error: BatchError) -> BatchResult {
        BatchResult {
            status: BatchStatus::RolledBack,
            files_modified: Vec::new(),
            files_created: Vec::new(),
            verification: None,
            errors: vec![BatchErrorDetail {
                file: None,
                phase: "verify".to_string(),
                message: error.to_string(),
            }],
            rolled_back: true,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{BatchInput, CreateOperation, EditOperation};
    use std::fs;

    fn make_input(edits: Vec<EditOperation>, creates: Vec<CreateOperation>) -> BatchInput {
        BatchInput {
            edits: if edits.is_empty() { None } else { Some(edits) },
            creates: if creates.is_empty() { None } else { Some(creates) },
            verify: Some(false),
            rollback_on_failure: Some(true),
        }
    }

    #[test]
    fn test_transaction_new_validates_input() {
        let dir = tempfile::tempdir().unwrap();
        let input = BatchInput {
            edits: None,
            creates: None,
            verify: Some(false),
            rollback_on_failure: Some(true),
        };
        let err = Transaction::new(dir.path().to_path_buf(), input).unwrap_err();
        assert!(matches!(err, BatchError::EmptyTransaction));
    }

    #[test]
    fn test_transaction_stage_writes_to_staging() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("file.ts"), "original").unwrap();

        let input = make_input(
            vec![EditOperation {
                file: "file.ts".to_string(),
                content: Some("new content".to_string()),
                operations: None,
            }],
            vec![],
        );

        let txn = Transaction::new(dir.path().to_path_buf(), input).unwrap();
        let staged = txn.stage().unwrap();

        // The staging area should have the new content
        let staging = staged.staging.as_ref().unwrap();
        assert_eq!(staging.read_staged("file.ts").unwrap(), "new content");

        // But the original file should be untouched
        assert_eq!(fs::read_to_string(dir.path().join("file.ts")).unwrap(), "original");
    }

    #[test]
    fn test_transaction_apply_creates_backups() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("file.ts"), "original").unwrap();

        let input = make_input(
            vec![EditOperation {
                file: "file.ts".to_string(),
                content: Some("new content".to_string()),
                operations: None,
            }],
            vec![],
        );

        let txn = Transaction::new(dir.path().to_path_buf(), input).unwrap();
        let applied = txn.stage().unwrap().apply().unwrap();

        // Backups should exist
        assert!(applied.backups.is_some());
        assert_eq!(applied.backups.as_ref().unwrap().backed_up_paths().len(), 1);
    }

    #[test]
    fn test_transaction_apply_writes_to_working_dir() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("file.ts"), "original").unwrap();

        let input = make_input(
            vec![EditOperation {
                file: "file.ts".to_string(),
                content: Some("new content".to_string()),
                operations: None,
            }],
            vec![],
        );

        let txn = Transaction::new(dir.path().to_path_buf(), input).unwrap();
        let _applied = txn.stage().unwrap().apply().unwrap();

        assert_eq!(
            fs::read_to_string(dir.path().join("file.ts")).unwrap(),
            "new content"
        );
    }

    #[test]
    fn test_transaction_commit_discards_backups() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("file.ts"), "original").unwrap();

        let input = make_input(
            vec![EditOperation {
                file: "file.ts".to_string(),
                content: Some("committed").to_string(),
                operations: None,
            }],
            vec![],
        );

        let txn = Transaction::new(dir.path().to_path_buf(), input).unwrap();
        let committed = txn.stage().unwrap().apply().unwrap().commit();

        // File should have the new content
        assert_eq!(
            fs::read_to_string(dir.path().join("file.ts")).unwrap(),
            "committed"
        );

        // Backups should be gone
        assert!(committed.backups.is_none());
    }

    #[test]
    fn test_transaction_rollback_restores_files() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("file.ts"), "original").unwrap();

        let input = make_input(
            vec![EditOperation {
                file: "file.ts".to_string(),
                content: Some("will be rolled back".to_string()),
                operations: None,
            }],
            vec![],
        );

        let txn = Transaction::new(dir.path().to_path_buf(), input).unwrap();
        let applied = txn.stage().unwrap().apply().unwrap();

        // Verify the write happened
        assert_eq!(
            fs::read_to_string(dir.path().join("file.ts")).unwrap(),
            "will be rolled back"
        );

        // Rollback
        let _rolled_back = applied.rollback().unwrap();
        assert_eq!(
            fs::read_to_string(dir.path().join("file.ts")).unwrap(),
            "original"
        );
    }

    #[test]
    fn test_transaction_rollback_removes_created_files() {
        let dir = tempfile::tempdir().unwrap();

        let input = make_input(
            vec![],
            vec![CreateOperation {
                file: "new_file.ts".to_string(),
                content: "will be removed".to_string(),
            }],
        );

        let txn = Transaction::new(dir.path().to_path_buf(), input).unwrap();
        let applied = txn.stage().unwrap().apply().unwrap();

        assert!(dir.path().join("new_file.ts").exists());

        let _rolled_back = applied.rollback().unwrap();
        assert!(!dir.path().join("new_file.ts").exists());
    }
}
