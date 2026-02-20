pub mod batch;
pub mod surgeon;
pub mod verify;

use crate::mcp::{ToolCallResult, ToolDefinition};
use serde_json::Value;
use std::path::Path;

/// Trait that every MCP tool implements.
#[async_trait::async_trait]
pub trait Tool: Send + Sync {
    fn definition(&self) -> ToolDefinition;

    async fn call(&self, params: Value, project_root: &Path) -> ToolCallResult;
}

/// Registry of all available tools.
pub struct ToolRegistry {
    tools: Vec<Box<dyn Tool>>,
}

impl ToolRegistry {
    pub fn new(project_root: &Path) -> Self {
        let mut tools: Vec<Box<dyn Tool>> = Vec::new();
        tools.push(Box::new(verify::VerifyTool::new(project_root)));
        tools.push(Box::new(batch::BatchTool::new(project_root)));
        tools.push(Box::new(surgeon::SurgeonTool::new()));
        Self { tools }
    }

    pub fn definitions(&self) -> Vec<ToolDefinition> {
        self.tools.iter().map(|t| t.definition()).collect()
    }

    pub async fn call(&self, name: &str, params: Value, project_root: &Path) -> ToolCallResult {
        for tool in &self.tools {
            if tool.definition().name == name {
                return tool.call(params, project_root).await;
            }
        }
        ToolCallResult::error(format!("Unknown tool: {name}"))
    }
}
