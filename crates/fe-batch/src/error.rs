use std::path::PathBuf;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum BatchError {
    // Validation errors
    #[error("File not found for edit: {0}")]
    FileNotFound(PathBuf),

    #[error("File already exists (cannot create): {0}")]
    FileAlreadyExists(PathBuf),

    #[error("Path escapes project root: {0}")]
    PathTraversal(PathBuf),

    #[error("Duplicate file in transaction: {0}")]
    DuplicatePath(String),

    #[error("Edit specifies both 'content' and 'operations' for: {0}")]
    AmbiguousEdit(String),

    #[error("Edit specifies neither 'content' nor 'operations' for: {0}")]
    EmptyEdit(String),

    #[error("No edits or creates specified")]
    EmptyTransaction,

    // File system errors
    #[error("Failed to read file {path}: {source}")]
    ReadError {
        path: PathBuf,
        source: std::io::Error,
    },

    #[error("Failed to write file {path}: {source}")]
    WriteError {
        path: PathBuf,
        source: std::io::Error,
    },

    #[error("Failed to create directory {path}: {source}")]
    MkdirError {
        path: PathBuf,
        source: std::io::Error,
    },

    #[error("Failed to backup file {path}: {source}")]
    BackupError {
        path: PathBuf,
        source: std::io::Error,
    },

    #[error("Atomic rename failed for {path}: {source}")]
    RenameError {
        path: PathBuf,
        source: std::io::Error,
    },

    // Staging errors
    #[error("Staging area creation failed: {0}")]
    StagingError(std::io::Error),

    // Verification errors
    #[error("Verification pipeline failed: {0}")]
    VerificationError(String),

    #[error("Verification tool not found: {tool}. Install it or set verify=false.")]
    ToolNotFound { tool: String },

    // Rollback errors
    #[error("CRITICAL: Rollback failed for {path}: {source}. Manual intervention required.")]
    RollbackError {
        path: PathBuf,
        source: std::io::Error,
    },

    // Internal errors
    #[error("Internal error: {0}")]
    Internal(String),
}
