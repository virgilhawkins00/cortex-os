use anyhow::Result;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::sync::Arc;
use tracing::info;

use crate::nats_bus::CortexBus;
use crate::permissions::PermissionPolicy;
use crate::tools::{Tool, ToolOutput};

/// The DelegateTool allows an agent to request assistance from another specialized agent.
pub struct DelegateTool {
    bus: Arc<CortexBus>,
}

impl DelegateTool {
    pub fn new(bus: Arc<CortexBus>) -> Self {
        Self { bus }
    }
}

#[derive(Serialize, Deserialize)]
struct DelegateArgs {
    role: String,
    task: String,
}

#[async_trait::async_trait]
impl Tool for DelegateTool {
    fn name(&self) -> &str {
        "delegate_task"
    }

    fn description(&self) -> &str {
        "Request another specialized agent to perform a sub-task. 
         Args: { 'role': 'devops|architect|sec_spec|software_engineer', 'task': 'description' }"
    }

    async fn execute(&self, args: Value, _policy: &PermissionPolicy) -> Result<ToolOutput> {
        let args: DelegateArgs = serde_json::from_value(args)?;
        
        info!("Delegated task to {}: {}", args.role, args.task);

        // Publish delegation request and wait for reply
        let request = json!({
            "role": args.role,
            "task": args.task,
        });

        match self.bus.request_bytes("cortex.swarm.delegate", &serde_json::to_vec(&request)?).await {
            Ok(resp_data) => {
                let response: Value = serde_json::from_slice(&resp_data)?;
                let output = response["final_answer"].as_str().unwrap_or("No answer provided");
                Ok(ToolOutput {
                    success: true,
                    content: format!("Result from {}: {}", args.role, output),
                    error: None,
                })
            }
            Err(e) => {
                Ok(ToolOutput {
                    success: false,
                    content: "".to_string(),
                    error: Some(format!("Delegation failed: {}", e)),
                })
            }
        }
    }
}
