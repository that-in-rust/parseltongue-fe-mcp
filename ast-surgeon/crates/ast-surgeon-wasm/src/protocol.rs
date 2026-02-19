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
}

/// Response from processing a single file.
#[derive(Debug, Serialize)]
pub struct SingleFileResponse {
    pub error: bool,
    pub content: Option<String>,
    pub changes: Vec<ChangeDescription>,
    pub warnings: Vec<String>,
    pub operation_errors: Vec<OperationErrorDetail>,
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
