//! ast-surgeon-wasm: WASM entry points for the TypeScript MCP server.
//!
//! Thin shell that deserializes JSON requests, calls core functions,
//! and serializes JSON responses. All logic lives in ast-surgeon-core.

use wasm_bindgen::prelude::*;

mod protocol;

// Expose Rust's allocator as C-compatible malloc/free/calloc/realloc.
// tree-sitter's C code needs these when compiled for wasm32-unknown-unknown.
mod c_alloc {
    use std::alloc::{alloc, alloc_zeroed, dealloc, realloc, Layout};

    #[no_mangle]
    pub unsafe extern "C" fn malloc(size: usize) -> *mut u8 {
        if size == 0 {
            return std::ptr::null_mut();
        }
        let layout = Layout::from_size_align_unchecked(size, 8);
        alloc(layout)
    }

    #[no_mangle]
    pub unsafe extern "C" fn free(ptr: *mut u8) {
        if ptr.is_null() {
            return;
        }
        // We don't know the original size, but dealloc requires a layout.
        // This is a best-effort stub -- in practice tree-sitter allocs are
        // paired with frees of the same size.
        let layout = Layout::from_size_align_unchecked(1, 8);
        dealloc(ptr, layout);
    }

    #[no_mangle]
    pub unsafe extern "C" fn calloc(count: usize, size: usize) -> *mut u8 {
        let total = count.wrapping_mul(size);
        if total == 0 {
            return std::ptr::null_mut();
        }
        let layout = Layout::from_size_align_unchecked(total, 8);
        alloc_zeroed(layout)
    }

    #[no_mangle]
    pub unsafe extern "C" fn realloc(ptr: *mut u8, new_size: usize) -> *mut u8 {
        if ptr.is_null() {
            return malloc(new_size);
        }
        if new_size == 0 {
            free(ptr);
            return std::ptr::null_mut();
        }
        // We don't have the original size, use 1 as placeholder.
        let old_layout = Layout::from_size_align_unchecked(1, 8);
        realloc(ptr, old_layout, new_size)
    }
}

/// Initialize panic hook for better error messages in JS console.
#[wasm_bindgen(start)]
pub fn init() {
    console_error_panic_hook::set_once();
}

/// Process a single file with one or more operations.
///
/// Input: JSON string matching `SingleFileRequest`
/// Output: JSON string matching `SingleFileResponse`
#[wasm_bindgen]
pub fn process_file(request_json: &str) -> String {
    match process_file_impl(request_json) {
        Ok(response) => response,
        Err(e) => {
            serde_json::to_string(&protocol::ErrorResponse {
                error: true,
                message: e.to_string(),
                code: "PROCESSING_ERROR".to_string(),
            })
            .unwrap_or_else(|_| {
                r#"{"error":true,"message":"serialization failed","code":"INTERNAL"}"#.to_string()
            })
        }
    }
}

/// Process multiple files in a batch.
///
/// Input: JSON string matching `BatchRequest`
/// Output: JSON string matching `BatchResponse`
#[wasm_bindgen]
pub fn process_batch(request_json: &str) -> String {
    match process_batch_impl(request_json) {
        Ok(response) => response,
        Err(e) => {
            serde_json::to_string(&protocol::ErrorResponse {
                error: true,
                message: e.to_string(),
                code: "PROCESSING_ERROR".to_string(),
            })
            .unwrap_or_else(|_| {
                r#"{"error":true,"message":"serialization failed","code":"INTERNAL"}"#.to_string()
            })
        }
    }
}

fn process_file_impl(request_json: &str) -> Result<String, Box<dyn std::error::Error>> {
    let request: protocol::SingleFileRequest = serde_json::from_str(request_json)?;

    let lang = ast_surgeon_lang::SupportedLanguage::from_str(&request.language)
        .map_err(|e| format!("Unsupported language: {}", e))?;

    let ts_language = lang.ts_language();

    // Parse the source
    let tree = ast_surgeon_core::validate::parse_best_effort(&request.content, &ts_language)
        .map_err(|e| format!("Parse failed: {}", e))?;

    // Execute operations
    let result = ast_surgeon_core::execute_operations(
        &request.content,
        &tree,
        &request.operations,
        &ts_language,
    );

    match result {
        Ok(op_result) => {
            let edit_count = op_result.changes.len();
            let response = protocol::SingleFileResponse {
                error: false,
                content: if request.dry_run {
                    None
                } else {
                    Some(op_result.content)
                },
                changes: op_result.changes,
                warnings: op_result.warnings,
                operation_errors: vec![],
                edit_count: if request.dry_run {
                    Some(edit_count)
                } else {
                    None
                },
                status: if request.dry_run {
                    "preview".to_string()
                } else {
                    "applied".to_string()
                },
            };
            Ok(serde_json::to_string(&response)?)
        }
        Err(e) => {
            let response = protocol::SingleFileResponse {
                error: true,
                content: None,
                changes: vec![],
                warnings: vec![],
                operation_errors: vec![protocol::OperationErrorDetail {
                    operation_index: 0,
                    code: error_code(&e),
                    message: e.to_string(),
                }],
                edit_count: None,
                status: "error".to_string(),
            };
            Ok(serde_json::to_string(&response)?)
        }
    }
}

fn process_batch_impl(request_json: &str) -> Result<String, Box<dyn std::error::Error>> {
    let request: protocol::BatchRequest = serde_json::from_str(request_json)?;

    let mut results = Vec::new();
    let mut errors = Vec::new();
    let mut total_edits = 0;

    for entry in &request.files {
        let lang = match ast_surgeon_lang::SupportedLanguage::from_str(&entry.language) {
            Ok(l) => l,
            Err(e) => {
                errors.push(protocol::BatchFileError {
                    path: entry.path.clone(),
                    error: e.to_string(),
                    code: "UNSUPPORTED_LANGUAGE".to_string(),
                });
                continue;
            }
        };

        let ts_language = lang.ts_language();

        let tree = match ast_surgeon_core::validate::parse_best_effort(&entry.content, &ts_language)
        {
            Ok(t) => t,
            Err(e) => {
                errors.push(protocol::BatchFileError {
                    path: entry.path.clone(),
                    error: format!("Parse failed: {}", e),
                    code: "PARSE_ERROR".to_string(),
                });
                continue;
            }
        };

        match ast_surgeon_core::execute_operations(
            &entry.content,
            &tree,
            &entry.operations,
            &ts_language,
        ) {
            Ok(op_result) => {
                let edits_count = op_result.changes.len();
                total_edits += edits_count;
                results.push(protocol::BatchFileResult {
                    path: entry.path.clone(),
                    content: if request.dry_run {
                        entry.content.clone()
                    } else {
                        op_result.content
                    },
                    changes: op_result.changes,
                    warnings: op_result.warnings,
                    edits_applied: edits_count,
                });
            }
            Err(e) => {
                errors.push(protocol::BatchFileError {
                    path: entry.path.clone(),
                    error: e.to_string(),
                    code: error_code(&e),
                });
            }
        }
    }

    let status = if errors.is_empty() {
        if request.dry_run {
            "preview"
        } else {
            "applied"
        }
    } else if results.is_empty() {
        "error"
    } else {
        "partial"
    };

    let response = protocol::BatchResponse {
        results,
        errors,
        total_edits,
        status: status.to_string(),
    };

    Ok(serde_json::to_string(&response)?)
}

/// Map operation errors to error codes.
fn error_code(e: &ast_surgeon_core::operations::OperationError) -> String {
    use ast_surgeon_core::operations::OperationError;
    match e {
        OperationError::TargetNotFound { .. } => "SYMBOL_NOT_FOUND".to_string(),
        OperationError::AmbiguousMatch { .. } => "AMBIGUOUS_MATCH".to_string(),
        OperationError::EditConflict(_) => "EDIT_CONFLICT".to_string(),
        OperationError::InvalidResult { .. } => "INVALID_RESULT".to_string(),
        OperationError::SourceHasErrors { .. } => "SOURCE_HAS_ERRORS".to_string(),
        OperationError::UnsupportedLanguage { .. } => "UNSUPPORTED_LANGUAGE".to_string(),
        OperationError::InvalidParams { .. } => "INVALID_PARAMS".to_string(),
    }
}
