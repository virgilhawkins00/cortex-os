pub mod bash;
pub mod file;
pub mod tree;
pub mod web;
pub mod delegation;
pub mod mcp;
pub mod script;

use anyhow::Result;
use serde_json::Value;
use std::collections::HashMap;

use std::sync::Arc;
use crate::nats_bus::{CortexBus, TaskStatus};
use crate::permissions::PermissionPolicy;
use crate::sandbox::Sandbox;

/// Every tool in Cortex OS implements this trait.
#[async_trait::async_trait]
pub trait Tool: Send + Sync {
    /// The unique name of this tool (e.g. "bash", "file_read").
    fn name(&self) -> &str;

    /// A short description for the tool catalog.
    fn description(&self) -> &str;

    /// Execute the tool with the given arguments.
    async fn execute(&self, args: Value, policy: &PermissionPolicy) -> Result<ToolOutput>;
}

/// The result of a tool execution.
#[derive(Debug, Clone)]
pub struct ToolOutput {
    pub success: bool,
    pub content: String,
    pub error: Option<String>,
}

/// Central registry of all available tools.
pub struct ToolRegistry {
    tools: HashMap<String, Box<dyn Tool>>,
}

impl ToolRegistry {
    /// Create an empty registry.
    pub fn new() -> Self {
        Self { tools: HashMap::new() }
    }

    /// Create a new registry with all built-in tools.
    pub fn with_defaults(sandbox: Sandbox, bus: Arc<CortexBus>) -> Self {
        let mut registry = Self::new();

        registry.register(Box::new(bash::BashTool::new(sandbox)));
        registry.register(Box::new(file::FileReadTool));
        registry.register(Box::new(file::FileWriteTool));
        registry.register(Box::new(tree::FileTreeTool));
        registry.register(Box::new(web::WebReadTool::new()));
        registry.register(Box::new(web::WebSearchTool::new()));
        registry.register(Box::new(delegation::DelegateTool::new(bus)));

        registry
    }

    /// Register a new tool.
    pub fn register(&mut self, tool: Box<dyn Tool>) {
        self.tools.insert(tool.name().to_string(), tool);
    }

    /// List all registered tool names.
    #[must_use]
    pub fn list(&self) -> Vec<&str> {
        self.tools.keys().map(String::as_str).collect()
    }

    /// Execute a tool by name.
    pub async fn execute(
        &self,
        name: &str,
        args: Value,
        policy: &PermissionPolicy,
    ) -> Result<ToolOutput> {
        let tool = self
            .tools
            .get(name)
            .ok_or_else(|| anyhow::anyhow!("Unknown tool: {name}"))?;

        tool.execute(args, policy).await
    }
}
