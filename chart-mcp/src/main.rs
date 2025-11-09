mod brp;
mod schema;
mod tools;

use anyhow::Result;
use schema::{MpcRequest, MpcResponse};
use serde_json::json;
use std::io::{self, BufRead, Write};
use tools::ToolRegistry;

fn main() -> Result<()> {
    let brp = brp::BrpClient::new("127.0.0.1", 15702);
    let registry = ToolRegistry::new();

    let stdin = io::stdin();
    let mut stdout = io::stdout();

    for line in stdin.lock().lines() {
        let line = line?;
        let request: MpcRequest = serde_json::from_str(&line)?;

        // Skip notifications (they don't have an id)
        if request.id.is_none() {
            continue;
        }

        let response = match request.method.as_str() {
            "initialize" => {
                MpcResponse {
                    jsonrpc: "2.0".to_string(),
                    result: json!({
                        "protocolVersion": "2024-11-05",
                        "capabilities": {
                            "tools": {}
                        },
                        "serverInfo": {
                            "name": "chart-mcp",
                            "version": "0.1.0"
                        }
                    }),
                    id: request.id.unwrap(),
                }
            }
            "tools/list" => {
                MpcResponse {
                    jsonrpc: "2.0".to_string(),
                    result: json!({
                        "tools": registry.list_schemas()
                    }),
                    id: request.id.unwrap(),
                }
            }
            "tools/call" => {
                let tool_name = request.params
                    .as_ref()
                    .and_then(|p| p.get("name"))
                    .and_then(|n| n.as_str())
                    .unwrap_or("");

                let tool_params = request.params
                    .as_ref()
                    .and_then(|p| p.get("arguments"))
                    .cloned();

                let result = registry.get(tool_name)
                    .ok_or_else(|| anyhow::anyhow!("Tool not found: {}", tool_name))?
                    .execute(&brp, tool_params)?;

                MpcResponse {
                    jsonrpc: "2.0".to_string(),
                    result: json!({
                        "content": [
                            {
                                "type": "text",
                                "text": serde_json::to_string_pretty(&result)?
                            }
                        ]
                    }),
                    id: request.id.unwrap(),
                }
            }
            _ => {
                return Err(anyhow::anyhow!("Unknown method: {}", request.method));
            }
        };

        serde_json::to_writer(&mut stdout, &response)?;
        stdout.write_all(b"\n")?;
        stdout.flush()?;
    }

    Ok(())
}
