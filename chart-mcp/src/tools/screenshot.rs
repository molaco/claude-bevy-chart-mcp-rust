use anyhow::Result;
use serde_json::{json, Value};
use crate::brp::BrpClient;
use crate::schema::ToolSchema;
use super::Tool;

pub struct ScreenshotTool;

impl Tool for ScreenshotTool {
    fn name(&self) -> &str {
        "chart_screenshot"
    }

    fn schema(&self) -> ToolSchema {
        ToolSchema {
            name: self.name().to_string(),
            description: "Take a screenshot of the Bevy chart application".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "path": {
                        "type": "string",
                        "description": "Optional file path for the screenshot (default: auto-generated)"
                    }
                },
                "required": []
            }),
        }
    }

    fn execute(&self, brp: &BrpClient, params: Option<Value>) -> Result<Value> {
        // Call BRP to initiate screenshot (returns immediately with path)
        let response = brp.call("chart/screenshot", params)?;

        // Extract the file path from the response
        let path = response
            .get("path")
            .and_then(|p| p.as_str())
            .ok_or_else(|| anyhow::anyhow!("No path in BRP response: {:?}", response))?;

        // Poll for the file to exist (max 5 seconds, check every 100ms)
        let start = std::time::Instant::now();
        while !std::path::Path::new(path).exists() {
            std::thread::sleep(std::time::Duration::from_millis(100));
            if start.elapsed() > std::time::Duration::from_secs(5) {
                return Err(anyhow::anyhow!("Timeout waiting for screenshot file: {}", path));
            }
        }

        Ok(response)
    }
}
