use anyhow::Result;
use async_trait::async_trait;
use serde_json::Value;
use std::process::Stdio;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::Command;

use crate::permissions::PermissionPolicy;
use crate::tools::{Tool, ToolOutput};

/// A tool that wraps an MCP server call.
/// In this basic version, it launches the server per call.
/// Future versions will maintain persistent background processes.
pub struct McpTool {
    pub name: String,
    pub description: String,
    pub command: String,
    pub args: Vec<String>,
}

#[async_trait]
impl Tool for McpTool {
    fn name(&self) -> &str {
        &self.name
    }

    fn description(&self) -> &str {
        &self.description
    }

    async fn execute(&self, args: Value, _policy: &PermissionPolicy) -> Result<ToolOutput> {
        let mut child = Command::new(&self.command)
            .args(&self.args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()?;

        let mut stdin = child.stdin.take().unwrap();
        let stdout = child.stdout.take().unwrap();
        let mut reader = BufReader::new(stdout).lines();

        // 1. Send call_tool request
        let request = serde_json::json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "tools/call",
            "params": {
                "name": self.name,
                "arguments": args
            }
        });

        stdin.write_all(format!("{}\n", request).as_bytes()).await?;

        // 2. Read response
        if let Some(line) = reader.next_line().await? {
            let response: Value = serde_json::from_str(&line)?;
            if let Some(error) = response.get("error") {
                return Ok(ToolOutput {
                    success: false,
                    content: String::new(),
                    error: Some(error.to_string()),
                });
            }

            let result = response.get("result").ok_or_else(|| anyhow::anyhow!("Missing result in MCP response"))?;
            let content = result.get("content").and_then(|c| c.as_array()).map(|arr| {
                arr.iter().map(|item| item["text"].as_str().unwrap_or("")).collect::<Vec<_>>().join("\n")
            }).unwrap_or_default();

            Ok(ToolOutput {
                success: true,
                content,
                error: None,
            })
        } else {
            anyhow::bail!("MCP server closed unexpectedly")
        }
    }
}
