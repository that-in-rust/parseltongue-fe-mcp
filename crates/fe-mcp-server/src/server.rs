use crate::mcp::*;
use crate::tools::ToolRegistry;
use serde_json::Value;
use std::path::PathBuf;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};

/// Run the MCP server: read JSON-RPC from stdin, write responses to stdout.
pub async fn run(project_root: PathBuf) -> Result<(), Box<dyn std::error::Error>> {
    let registry = ToolRegistry::new(&project_root);

    let stdin = tokio::io::stdin();
    let mut stdout = tokio::io::stdout();
    let reader = BufReader::new(stdin);
    let mut lines = reader.lines();

    tracing::info!("MCP server ready, reading from stdin");

    while let Some(line) = lines.next_line().await? {
        let line = line.trim().to_string();
        if line.is_empty() {
            continue;
        }

        let request: JsonRpcRequest = match serde_json::from_str(&line) {
            Ok(r) => r,
            Err(e) => {
                let resp = JsonRpcResponse::error(None, -32700, format!("Parse error: {e}"));
                write_response(&mut stdout, &resp).await?;
                continue;
            }
        };

        let response = handle_request(&request, &registry, &project_root).await;

        if let Some(resp) = response {
            write_response(&mut stdout, &resp).await?;
        }
    }

    Ok(())
}

async fn handle_request(
    req: &JsonRpcRequest,
    registry: &ToolRegistry,
    project_root: &PathBuf,
) -> Option<JsonRpcResponse> {
    match req.method.as_str() {
        "initialize" => {
            let result = InitializeResult {
                protocol_version: "2024-11-05".into(),
                capabilities: ServerCapabilities {
                    tools: ToolsCapability {
                        list_changed: false,
                    },
                },
                server_info: ServerInfo {
                    name: "fe-tools".into(),
                    version: env!("CARGO_PKG_VERSION").into(),
                },
            };
            Some(JsonRpcResponse::success(
                req.id.clone(),
                serde_json::to_value(result).unwrap(),
            ))
        }

        // Notifications — no response expected
        "notifications/initialized" | "initialized" => None,

        "tools/list" => {
            let defs = registry.definitions();
            let result = ToolsListResult { tools: defs };
            Some(JsonRpcResponse::success(
                req.id.clone(),
                serde_json::to_value(result).unwrap(),
            ))
        }

        "tools/call" => {
            let name = req.params.get("name").and_then(Value::as_str).unwrap_or("");
            let arguments = req
                .params
                .get("arguments")
                .cloned()
                .unwrap_or(Value::Object(serde_json::Map::new()));

            let result = registry.call(name, arguments, project_root).await;
            Some(JsonRpcResponse::success(
                req.id.clone(),
                serde_json::to_value(result).unwrap(),
            ))
        }

        _ => {
            tracing::debug!("Unknown method: {}", req.method);
            Some(JsonRpcResponse::error(
                req.id.clone(),
                -32601,
                format!("Method not found: {}", req.method),
            ))
        }
    }
}

async fn write_response(
    stdout: &mut tokio::io::Stdout,
    resp: &JsonRpcResponse,
) -> Result<(), Box<dyn std::error::Error>> {
    let json = serde_json::to_string(resp)?;
    stdout.write_all(json.as_bytes()).await?;
    stdout.write_all(b"\n").await?;
    stdout.flush().await?;
    Ok(())
}
