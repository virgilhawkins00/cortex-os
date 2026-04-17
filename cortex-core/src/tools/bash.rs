use anyhow::Result;
use serde_json::Value;
use tracing::info;

use crate::permissions::{PermissionPolicy, PermissionVerdict};
use crate::sandbox::Sandbox;

use super::{Tool, ToolOutput};

/// Executes shell commands inside the sandbox.
pub struct BashTool {
    sandbox: Sandbox,
}

impl BashTool {
    #[must_use]
    pub fn new(sandbox: Sandbox) -> Self {
        Self { sandbox }
    }
}

#[async_trait::async_trait]
impl Tool for BashTool {
    fn name(&self) -> &str {
        "bash"
    }

    fn description(&self) -> &str {
        "Execute a shell command in a sandboxed environment with timeout"
    }

    async fn execute(&self, args: Value, policy: &PermissionPolicy) -> Result<ToolOutput> {
        let cmd = args
            .get("command")
            .and_then(Value::as_str)
            .ok_or_else(|| anyhow::anyhow!("Missing 'command' argument"))?;

        // Permission check
        match policy.check_bash(cmd) {
            PermissionVerdict::Allowed => {}
            PermissionVerdict::Denied(reason) => {
                return Ok(ToolOutput {
                    success: false,
                    content: String::new(),
                    error: Some(reason),
                });
            }
        }

        info!("Executing bash: {cmd}");
        let result = self.sandbox.exec_bash(cmd).await?;

        if result.timed_out {
            return Ok(ToolOutput {
                success: false,
                content: String::new(),
                error: Some(result.stderr),
            });
        }

        Ok(ToolOutput {
            success: result.exit_code == 0,
            content: result.stdout,
            error: if result.stderr.is_empty() {
                None
            } else {
                Some(result.stderr)
            },
        })
    }
}
