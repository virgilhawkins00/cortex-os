pub mod bash;
pub mod file;
pub mod tree;
pub mod web;

use anyhow::Result;
use serde_json::Value;
use std::collections::HashMap;

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
    /// Create a new registry with all built-in tools.
    pub fn with_defaults(sandbox: Sandbox) -> Self {
        let mut tools: HashMap<String, Box<dyn Tool>> = HashMap::new();

        let bash_tool = bash::BashTool::new(sandbox);
        tools.insert(bash_tool.name().to_string(), Box::new(bash_tool));

        let file_read = file::FileReadTool;
        tools.insert(file_read.name().to_string(), Box::new(file_read));

        let file_write = file::FileWriteTool;
        tools.insert(file_write.name().to_string(), Box::new(file_write));

        let tree_tool = tree::FileTreeTool;
        tools.insert(tree_tool.name().to_string(), Box::new(tree_tool));

        let web_read = web::WebReadTool::new();
        tools.insert(web_read.name().to_string(), Box::new(web_read));

        let web_search = web::WebSearchTool::new();
        tools.insert(web_search.name().to_string(), Box::new(web_search));

        Self { tools }
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
