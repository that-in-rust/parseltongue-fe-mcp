use super::Tool;
use crate::mcp::{ToolCallResult, ToolDefinition};
use ast_surgeon_core::operations::{ChangeDescription, Operation, OperationError};
use ast_surgeon_lang::registry::detect_language;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::path::Path;

pub struct SurgeonTool;

#[derive(Deserialize)]
struct SurgeonParams {
    operations: Vec<Value>,
    #[serde(default)]
    dry_run: bool,
}

#[derive(Serialize)]
struct SurgeonResult {
    status: String,
    files_modified: Vec<String>,
    changes: Vec<FileChanges>,
    warnings: Vec<String>,
    dry_run: bool,
}

#[derive(Serialize)]
struct FileChanges {
    file: String,
    changes: Vec<ChangeDescription>,
    warnings: Vec<String>,
}

impl SurgeonTool {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait::async_trait]
impl Tool for SurgeonTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "fe_surgeon".into(),
            description: "Apply structured code operations instead of rewriting entire files. \
                Operations: rename_symbol, add_import, remove_import, update_import_paths, \
                add_parameter, remove_parameter, make_async, wrap_in_block, \
                extract_to_variable. Faster and safer than generating modified source text — \
                no syntax errors possible. Each operation must specify a 'file' field."
                .into(),
            input_schema: json!({
                "type": "object",
                "required": ["operations"],
                "properties": {
                    "operations": {
                        "type": "array",
                        "items": {
                            "type": "object",
                            "required": ["op", "file"],
                            "properties": {
                                "op": {
                                    "type": "string",
                                    "enum": [
                                        "rename_symbol", "add_import", "remove_import",
                                        "update_import_paths", "add_parameter", "remove_parameter",
                                        "make_async", "wrap_in_block", "extract_to_variable"
                                    ]
                                },
                                "file": {"type": "string", "description": "Target file (relative to project root)."}
                            }
                        },
                        "description": "Array of AST operations to apply. See operation-specific fields in the enum variants."
                    },
                    "dry_run": {
                        "type": "boolean",
                        "default": false,
                        "description": "Preview changes without writing to disk."
                    }
                }
            }),
        }
    }

    async fn call(&self, params: Value, project_root: &Path) -> ToolCallResult {
        let params: SurgeonParams = match serde_json::from_value(params) {
            Ok(p) => p,
            Err(e) => return ToolCallResult::error(format!("Invalid parameters: {e}")),
        };

        // Group operations by file
        let mut ops_by_file: HashMap<String, Vec<Value>> = HashMap::new();
        for op_value in &params.operations {
            let file = match op_value.get("file").and_then(Value::as_str) {
                Some(f) => f.to_string(),
                None => {
                    return ToolCallResult::error(
                        "Every operation must have a 'file' field".into(),
                    );
                }
            };
            ops_by_file.entry(file).or_default().push(op_value.clone());
        }

        let mut result = SurgeonResult {
            status: "success".into(),
            files_modified: Vec::new(),
            changes: Vec::new(),
            warnings: Vec::new(),
            dry_run: params.dry_run,
        };

        // Process each file
        for (file_path, op_values) in &ops_by_file {
            let abs_path = project_root.join(file_path);

            // Read source
            let source = match std::fs::read_to_string(&abs_path) {
                Ok(s) => s,
                Err(e) => {
                    result.status = "error".into();
                    result.warnings.push(format!("{file_path}: Failed to read: {e}"));
                    continue;
                }
            };

            // Detect language from file extension
            let lang = match detect_language(file_path) {
                Ok(l) => l,
                Err(_) => {
                    result.warnings.push(format!(
                        "{file_path}: Unsupported file type, skipping"
                    ));
                    continue;
                }
            };
            let ts_language = lang.ts_language();

            // Parse operations from JSON into the Operation enum
            let ops: Vec<Operation> = match op_values
                .iter()
                .map(|v| serde_json::from_value(v.clone()))
                .collect::<Result<Vec<_>, _>>()
            {
                Ok(ops) => ops,
                Err(e) => {
                    result.status = "error".into();
                    result
                        .warnings
                        .push(format!("{file_path}: Invalid operation: {e}"));
                    continue;
                }
            };

            // Parse source with tree-sitter
            let tree = match ast_surgeon_core::validate::parse_best_effort(&source, &ts_language) {
                Ok(t) => t,
                Err(e) => {
                    result.status = "error".into();
                    result
                        .warnings
                        .push(format!("{file_path}: Parse error: {e:?}"));
                    continue;
                }
            };

            // Execute operations
            match ast_surgeon_core::execute_operations(&source, &tree, &ops, &ts_language) {
                Ok(op_result) => {
                    // Write back if not dry_run
                    if !params.dry_run {
                        if let Err(e) = std::fs::write(&abs_path, &op_result.content) {
                            result.status = "error".into();
                            result
                                .warnings
                                .push(format!("{file_path}: Failed to write: {e}"));
                            continue;
                        }
                    }

                    result.files_modified.push(file_path.clone());
                    result.changes.push(FileChanges {
                        file: file_path.clone(),
                        changes: op_result.changes,
                        warnings: op_result.warnings,
                    });
                }
                Err(e) => {
                    result.status = "error".into();
                    let msg = match &e {
                        OperationError::TargetNotFound { description } => {
                            format!("{file_path}: Target not found: {description}")
                        }
                        OperationError::AmbiguousMatch {
                            description, count, ..
                        } => {
                            format!(
                                "{file_path}: Ambiguous match ({count} found): {description}"
                            )
                        }
                        OperationError::InvalidResult { errors } => {
                            format!(
                                "{file_path}: Operation produced invalid syntax ({} errors)",
                                errors.len()
                            )
                        }
                        _ => format!("{file_path}: {e}"),
                    };
                    result.warnings.push(msg);
                }
            }
        }

        match serde_json::to_string_pretty(&result) {
            Ok(json) => ToolCallResult::text(json),
            Err(e) => ToolCallResult::error(format!("Serialization error: {e}")),
        }
    }
}
