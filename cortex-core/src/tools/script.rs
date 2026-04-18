use anyhow::Result;
use async_trait::async_trait;
use serde_json::Value;
use std::path::PathBuf;
use std::process::Stdio;
use tokio::process::Command;

use crate::permissions::PermissionPolicy;
use crate::tools::{Tool, ToolOutput};

/// Represents a local script tool.
pub struct ScriptTool {
    pub name: String,
    pub description: String,
    pub script_path: PathBuf,
    pub interpreter: Option<String>,
}

#[async_trait]
impl Tool for ScriptTool {
    fn name(&self) -> &str {
        &self.name
    }

    fn description(&self) -> &str {
        &self.description
    }

    async fn execute(&self, args: Value, _policy: &PermissionPolicy) -> Result<ToolOutput> {
        let mut cmd = if let Some(ref interpreter) = self.interpreter {
            let mut c = Command::new(interpreter);
            c.arg(&self.script_path);
            c
        } else {
            Command::new(&self.script_path)
        };

        // Inject arguments as environment variables for easier script consumption
        cmd.env("CORTEX_TOOL_ARGS", serde_json::to_string(&args)?);
        
        // Populate specific fields as env vars for convenience
        if let Some(obj) = args.as_object() {
            for (k, v) in obj {
                let val_str = match v {
                    Value::String(s) => s.clone(),
                    other => other.to_string(),
                };
                cmd.env(format!("ARG_{}", k.to_uppercase()), val_str);
            }
        }

        let output = cmd
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()?
            .wait_with_output()
            .await?;

        let success = output.status.success();
        let content = String::from_utf8_lossy(&output.stdout).to_string();
        let error = if !success {
            Some(String::from_utf8_lossy(&output.stderr).to_string())
        } else {
            None
        };

        Ok(ToolOutput {
            success,
            content,
            error,
        })
    }
}
