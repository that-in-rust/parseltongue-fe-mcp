use serde::{Deserialize, Serialize};

/// Top-level input to the fe_batch tool.
#[derive(Debug, Deserialize)]
pub struct BatchInput {
    /// Files to edit (must already exist).
    pub edits: Option<Vec<EditOperation>>,

    /// Files to create (must NOT already exist).
    pub creates: Option<Vec<CreateOperation>>,

    /// Run verification (lint/types/tests) after applying changes. Default: true.
    pub verify: Option<bool>,

    /// Rollback all changes if verification fails. Default: true.
    pub rollback_on_failure: Option<bool>,
}

impl BatchInput {
    pub fn verify_enabled(&self) -> bool {
        self.verify.unwrap_or(true)
    }

    pub fn rollback_on_failure(&self) -> bool {
        self.rollback_on_failure.unwrap_or(true)
    }
}

#[derive(Debug, Deserialize)]
pub struct EditOperation {
    /// Path to the file to edit (relative to project root).
    pub file: String,

    /// Full replacement content for the file.
    pub content: Option<String>,

    /// AST operations to apply instead of full content replacement.
    /// Mutually exclusive with `content`.
    pub operations: Option<Vec<AstOperation>>,
}

#[derive(Debug, Deserialize)]
pub struct CreateOperation {
    /// Path for the new file (relative to project root).
    pub file: String,

    /// Content for the new file.
    pub content: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct AstOperation {
    pub op: String,
    pub target: Option<String>,
    pub args: Option<serde_json::Value>,
}

/// Result returned from fe_batch.
#[derive(Debug, Serialize)]
pub struct BatchResult {
    pub status: BatchStatus,
    pub files_modified: Vec<String>,
    pub files_created: Vec<String>,
    pub verification: Option<fe_verify::types::VerificationSummary>,
    pub errors: Vec<BatchErrorDetail>,
    pub rolled_back: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum BatchStatus {
    Success,
    VerificationFailed,
    RolledBack,
    Error,
}

#[derive(Debug, Serialize)]
pub struct BatchErrorDetail {
    pub file: Option<String>,
    pub phase: String,
    pub message: String,
}
