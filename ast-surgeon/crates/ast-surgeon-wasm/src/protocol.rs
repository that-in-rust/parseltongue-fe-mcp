//! JSON request/response types for the WASM boundary.

use ast_surgeon_core::operations::{ChangeDescription, Operation};
use serde::{Deserialize, Serialize};

/// Request to process a single file.
#[derive(Debug, Deserialize)]
pub struct SingleFileRequest {
    /// The file content as a string.
    pub content: String,
    /// Language identifier: "typescript", "tsx", "javascript", "jsx", "css"
    pub language: String,
    /// Operations to apply.
    pub operations: Vec<Operation>,
    /// If true, compute edits but don't apply -- return a preview.
    #[serde(default)]
    pub dry_run: bool,
}

/// Response from processing a single file.
#[derive(Debug, Serialize)]
pub struct SingleFileResponse {
    pub error: bool,
    /// The new file content (None if dry_run or error).
    pub content: Option<String>,
    pub changes: Vec<ChangeDescription>,
    pub warnings: Vec<String>,
    pub operation_errors: Vec<OperationErrorDetail>,
    /// If dry_run, the number of edits that would be applied.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub edit_count: Option<usize>,
    /// "applied" | "preview" | "error"
    pub status: String,
}

/// Request to process multiple files in a batch.
#[derive(Debug, Deserialize)]
pub struct BatchRequest {
    pub files: Vec<BatchFileEntry>,
    #[serde(default)]
    pub dry_run: bool,
}

/// A single file entry in a batch request.
#[derive(Debug, Deserialize)]
pub struct BatchFileEntry {
    pub path: String,
    pub content: String,
    pub language: String,
    pub operations: Vec<Operation>,
}

/// Response from processing multiple files.
#[derive(Debug, Serialize)]
pub struct BatchResponse {
    pub results: Vec<BatchFileResult>,
    pub errors: Vec<BatchFileError>,
    pub total_edits: usize,
    /// "applied" | "preview" | "partial" | "error"
    pub status: String,
}

/// Result for one file in a batch.
#[derive(Debug, Serialize)]
pub struct BatchFileResult {
    pub path: String,
    pub content: String,
    pub changes: Vec<ChangeDescription>,
    pub warnings: Vec<String>,
    pub edits_applied: usize,
}

/// Error for one file in a batch.
#[derive(Debug, Serialize)]
pub struct BatchFileError {
    pub path: String,
    pub error: String,
    pub code: String,
}

/// Details about a failed operation.
#[derive(Debug, Serialize)]
pub struct OperationErrorDetail {
    pub operation_index: usize,
    pub code: String,
    pub message: String,
}

/// Generic error response.
#[derive(Debug, Serialize)]
pub struct ErrorResponse {
    pub error: bool,
    pub message: String,
    pub code: String,
}
