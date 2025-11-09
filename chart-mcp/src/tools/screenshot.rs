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
        brp.call("chart/screenshot", params)
    }
}
