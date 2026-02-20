use super::Tool;
use crate::mcp::{ToolCallResult, ToolDefinition};
use fe_common::git;
use fe_verify::detection;
use fe_verify::pipeline::VerificationPipeline;
use serde::Deserialize;
use serde_json::{json, Value};
use std::path::{Path, PathBuf};

pub struct VerifyTool {
    pipeline: VerificationPipeline,
}

#[derive(Deserialize, Default)]
struct VerifyParams {
    #[serde(default)]
    files: Vec<String>,
    #[serde(default)]
    checks: Vec<String>,
    #[serde(default)]
    fix: bool,
}

impl VerifyTool {
    pub fn new(project_root: &Path) -> Self {
        let tools = detection::detect_tools(project_root);
        let pipeline = VerificationPipeline::from_detected(tools);
        Self { pipeline }
    }
}

#[async_trait::async_trait]
impl Tool for VerifyTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "fe_verify".into(),
            description: "Verify frontend code changes: runs lint (ESLint/Biome), typecheck \
                (tsc), and affected tests in one call. Returns structured JSON — NOT terminal \
                output. Use this INSTEAD of running 'npm run lint', 'npx tsc', or 'npm test' \
                separately. Returns fix suggestions you can apply directly. Always call this \
                after writing or modifying any .tsx/.ts/.css file."
                .into(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "files": {
                        "type": "array",
                        "items": {"type": "string"},
                        "description": "Files to verify. Omit to verify all files changed since last commit."
                    },
                    "checks": {
                        "type": "array",
                        "items": {"enum": ["lint", "types", "tests", "all"]},
                        "default": ["all"],
                        "description": "Which checks to run."
                    },
                    "fix": {
                        "type": "boolean",
                        "default": false,
                        "description": "Auto-fix lint issues where possible."
                    }
                }
            }),
        }
    }

    async fn call(&self, params: Value, project_root: &Path) -> ToolCallResult {
        let params: VerifyParams = match serde_json::from_value(params) {
            Ok(p) => p,
            Err(e) => return ToolCallResult::error(format!("Invalid parameters: {e}")),
        };

        // Resolve files: explicit list or git-changed files
        let file_paths: Vec<PathBuf> = if params.files.is_empty() {
            match git::changed_files(project_root) {
                Ok(files) => git::filter_frontend_files(&files),
                Err(e) => {
                    tracing::warn!("Could not get git changed files: {e}");
                    Vec::new()
                }
            }
        } else {
            params.files.iter().map(PathBuf::from).collect()
        };

        let file_refs: Vec<&Path> = file_paths.iter().map(|p| p.as_path()).collect();

        match self.pipeline.run(project_root, &file_refs).await {
            Ok(summary) => {
                let json = match serde_json::to_string_pretty(&summary) {
                    Ok(j) => j,
                    Err(e) => return ToolCallResult::error(format!("Serialization error: {e}")),
                };
                ToolCallResult::text(json)
            }
            Err(e) => ToolCallResult::error(format!("Verification failed: {e}")),
        }
    }
}
