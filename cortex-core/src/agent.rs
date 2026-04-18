use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use tracing::{debug, info, warn};

use crate::nats_bus::{BrainThinkRequest, CortexBus, TaskStatus};
use crate::permissions::PermissionPolicy;
use crate::tools::ToolRegistry;

/// An entry in the agent's short-term history.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentStep {
    pub thought: String,
    pub action: Option<String>,
    pub observation: Option<String>,
}

use std::sync::Arc;

/// The Agent Orchestrator manages the autonomous Think-Act-Observe loop.
pub struct Agent {
    bus: Arc<CortexBus>,
    registry: Arc<ToolRegistry>,
    policy: Arc<PermissionPolicy>,
    max_steps: usize,
    role: Option<String>,
}

impl Agent {
    pub fn new(bus: Arc<CortexBus>, registry: Arc<ToolRegistry>, policy: Arc<PermissionPolicy>) -> Self {
        Self {
            bus,
            registry,
            policy,
            max_steps: 10,
            role: None,
        }
    }

    /// Set the specialized role for this agent (e.g., "devops").
    pub fn with_role(mut self, role: &str) -> Self {
        self.role = Some(role.to_string());
        self
    }

    /// Set the maximum number of steps allowed for a single task.
    pub fn with_max_steps(mut self, steps: usize) -> Self {
        self.max_steps = steps;
        self
    }

    /// Run the autonomous loop for a given prompt.
    pub async fn run(&self, prompt: &str) -> Result<AgentResult> {
        info!("Agent starting task: \"{}\"", prompt);
        
        let mut history: Vec<AgentStep> = Vec::new();
        let mut current_prompt = prompt.to_string();
        let mut steps_count = 0;

        loop {
            if steps_count >= self.max_steps {
                warn!("Max steps ({}) reached for agent task", self.max_steps);
                break;
            }
            steps_count += 1;

            debug!("Agent loop step {}: calling brain", steps_count);

            // 1. Call the brain via NATS
            let req = BrainThinkRequest {
                prompt: current_prompt.clone(),
                model: None,
                include_memory: true,
                stream: false,
                metadata: None,
                role: self.role.clone(),
            };

            let result = self.bus.brain_think(&req).await?;
            if result.status != TaskStatus::Success {
                return Err(anyhow!("Brain failed: {:?}", result.error));
            }

            let brain_output: Value = serde_json::from_str(&result.output)?;
            let response_text = brain_output.get("response").and_then(|v| v.as_str()).unwrap_or("");
            let tool_call = brain_output.get("tool_call");

            // 2. record the thought
            let mut step = AgentStep {
                thought: response_text.to_string(),
                action: None,
                observation: None,
            };

            // 3. Handle tool call
            if let Some(call) = tool_call.filter(|v| !v.is_null()) {
                let tool_name = call.get("tool").and_then(|v| v.as_str()).unwrap_or("");
                let tool_args = call.get("args").cloned().unwrap_or(json!({}));
                
                info!("Agent acting: {} with args {}", tool_name, tool_args);
                step.action = Some(format!("{}({})", tool_name, tool_args));

                // Audit: Tool Execution Start
                let _ = self.bus.publish_audit_log(
                    "agent", 
                    "tool_execute_start", 
                    json!({ 
                        "tool": tool_name, 
                        "args": tool_args,
                        "role": self.role.clone().unwrap_or_else(|| "default".to_string())
                    }),
                    None
                ).await;

                // Execute tool
                match self.registry.execute(tool_name, tool_args, &self.policy).await {
                    Ok(output) => {
                        let obs = if output.success {
                            output.content
                        } else {
                            output.error.unwrap_or_else(|| "Unknown error".to_string())
                        };
                        
                        step.observation = Some(obs.clone());
                        history.push(step);

                        // Audit: Tool Execution Success
                        let _ = self.bus.publish_audit_log(
                            "agent", 
                            "tool_execute_success", 
                            json!({ "tool": tool_name, "output_len": obs.len() }),
                            None
                        ).await;

                        // Update prompt for the next iteration (Act-Observe logic)
                        // We append the observation to give context to the brain.
                        current_prompt = format!(
                            "{}\n\n[OBSERVATION from {}]\n{}",
                            prompt, tool_name, obs
                        );
                    }
                    Err(e) => {
                        warn!("Tool execution error: {}", e);
                        step.observation = Some(format!("Error: {}", e));
                        history.push(step);
                        break;
                    }
                }
            } else {
                // No tool call -> final answer reached
                info!("Agent finished task.");
                history.push(step);
                break;
            }
        }

        let final_answer = history.last().map(|s| s.thought.clone()).unwrap_or_default();

        Ok(AgentResult {
            final_answer,
            steps: history,
        })
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct AgentResult {
    pub final_answer: String,
    pub steps: Vec<AgentStep>,
}
