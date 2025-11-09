use anyhow::Result;
use serde_json::Value;
use crate::brp::BrpClient;
use crate::schema::ToolSchema;

pub mod screenshot;

pub trait Tool {
    fn name(&self) -> &str;
    fn schema(&self) -> ToolSchema;
    fn execute(&self, brp: &BrpClient, params: Option<Value>) -> Result<Value>;
}

pub struct ToolRegistry {
    tools: Vec<Box<dyn Tool>>,
}

impl ToolRegistry {
    pub fn new() -> Self {
        let mut registry = Self { tools: vec![] };
        registry.register(Box::new(screenshot::ScreenshotTool));
        registry
    }

    pub fn register(&mut self, tool: Box<dyn Tool>) {
        self.tools.push(tool);
    }

    pub fn get(&self, name: &str) -> Option<&dyn Tool> {
        self.tools.iter()
            .find(|t| t.name() == name)
            .map(|b| b.as_ref())
    }

    pub fn list_schemas(&self) -> Vec<ToolSchema> {
        self.tools.iter().map(|t| t.schema()).collect()
    }
}
