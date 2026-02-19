//! ast-surgeon-wasm: WASM entry points for the TypeScript MCP server.
//!
//! Thin shell that deserializes JSON requests, calls core functions,
//! and serializes JSON responses. All logic lives in ast-surgeon-core.

use wasm_bindgen::prelude::*;

mod protocol;

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
    )
    .map_err(|e| format!("Operation failed: {}", e))?;

    let response = protocol::SingleFileResponse {
        error: false,
        content: Some(result.content),
        changes: result.changes,
        warnings: result.warnings,
        operation_errors: vec![],
    };

    Ok(serde_json::to_string(&response)?)
}
