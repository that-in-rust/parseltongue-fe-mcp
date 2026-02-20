use super::Tool;
use crate::mcp::{ToolCallResult, ToolDefinition};
use fe_batch::types::BatchInput;
use fe_batch::Transaction;
use fe_verify::detection;
use fe_verify::pipeline::VerificationPipeline;
use serde_json::{json, Value};
use std::path::Path;

pub struct BatchTool {
    pipeline: VerificationPipeline,
}

impl BatchTool {
    pub fn new(project_root: &Path) -> Self {
        let tools = detection::detect_tools(project_root);
        let pipeline = VerificationPipeline::from_detected(tools);
        Self { pipeline }
    }
}

#[async_trait::async_trait]
impl Tool for BatchTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "fe_batch".into(),
            description: "Apply changes to multiple files atomically. If any file fails \
                verification (lint/types/tests), ALL changes are rolled back. Use for \
                coordinated changes: component + test + story, or renaming across multiple \
                files. Includes built-in verification — no need to call fe_verify separately."
                .into(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "edits": {
                        "type": "array",
                        "items": {
                            "type": "object",
                            "required": ["file"],
                            "properties": {
                                "file": {"type": "string", "description": "Path to existing file (relative to project root)."},
                                "content": {"type": "string", "description": "Full replacement content."},
                                "operations": {
                                    "type": "array",
                                    "description": "AST operations instead of full content replacement.",
                                    "items": {
                                        "type": "object",
                                        "properties": {
                                            "op": {"type": "string"},
                                            "target": {"type": "string"},
                                            "args": {"type": "object"}
                                        }
                                    }
                                }
                            }
                        },
                        "description": "Files to edit (must already exist). Provide either content or operations, not both."
                    },
                    "creates": {
                        "type": "array",
                        "items": {
                            "type": "object",
                            "required": ["file", "content"],
                            "properties": {
                                "file": {"type": "string", "description": "Path for the new file (relative to project root)."},
                                "content": {"type": "string", "description": "Content for the new file."}
                            }
                        },
                        "description": "Files to create (must NOT already exist)."
                    },
                    "verify": {
                        "type": "boolean",
                        "default": true,
                        "description": "Run verification (lint/types/tests) after applying changes."
                    },
                    "rollback_on_failure": {
                        "type": "boolean",
                        "default": true,
                        "description": "Rollback all changes if verification fails."
                    }
                }
            }),
        }
    }

    async fn call(&self, params: Value, project_root: &Path) -> ToolCallResult {
        let input: BatchInput = match serde_json::from_value(params) {
            Ok(i) => i,
            Err(e) => return ToolCallResult::error(format!("Invalid parameters: {e}")),
        };

        let should_verify = input.verify_enabled();
        let should_rollback = input.rollback_on_failure();

        // Transaction lifecycle: new → stage → apply → verify → commit/rollback
        let txn = match Transaction::new(project_root.to_path_buf(), input) {
            Ok(t) => t,
            Err(e) => return ToolCallResult::error(format!("Validation failed: {e}")),
        };

        let txn = match txn.stage() {
            Ok(t) => t,
            Err(e) => return ToolCallResult::error(format!("Staging failed: {e}")),
        };

        let txn = match txn.apply() {
            Ok(t) => t,
            Err(e) => return ToolCallResult::error(format!("Apply failed: {e}")),
        };

        // Optionally run verification
        if should_verify {
            let affected_owned = txn.affected_files();
            let affected: Vec<&Path> = affected_owned.iter().map(|p| p.as_path()).collect();

            match self.pipeline.run(txn.project_root(), &affected).await {
                Ok(summary) => {
                    if summary.is_passing() {
                        let result = txn.commit().into_result(Some(summary));
                        return to_tool_result(&result);
                    }
                    // Verification failed
                    if should_rollback {
                        match txn.rollback() {
                            Ok(rolled_back) => {
                                let result = rolled_back.into_result(Some(summary));
                                return to_tool_result(&result);
                            }
                            Err(e) => {
                                return ToolCallResult::error(format!("Rollback failed: {e}"));
                            }
                        }
                    }
                    // No rollback requested — commit despite failures
                    let result = txn.commit().into_result_with_warnings(Some(summary));
                    return to_tool_result(&result);
                }
                Err(e) => {
                    // Verification errored — commit anyway (files already written)
                    let result = txn.commit().into_result(None);
                    tracing::warn!("Verification error (changes committed anyway): {e}");
                    return to_tool_result(&result);
                }
            }
        }

        // No verification requested
        let result = txn.commit().into_result(None);
        to_tool_result(&result)
    }
}

fn to_tool_result(result: &fe_batch::BatchResult) -> ToolCallResult {
    match serde_json::to_string_pretty(result) {
        Ok(json) => ToolCallResult::text(json),
        Err(e) => ToolCallResult::error(format!("Serialization error: {e}")),
    }
}
