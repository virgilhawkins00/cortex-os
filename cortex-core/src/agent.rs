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

/// The Agent Orchestrator manages the autonomous Think-Act-Observe loop.
pub struct Agent<'a> {
    bus: &'a CortexBus,
    registry: &'a ToolRegistry,
    policy: &'a PermissionPolicy,
    max_steps: usize,
}

impl<'a> Agent<'a> {
    pub fn new(bus: &'a CortexBus, registry: &'a ToolRegistry, policy: &'a PermissionPolicy) -> Self {
        Self {
            bus,
            registry,
            policy,
            max_steps: 10,
        }
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

                // Execute tool
                match self.registry.execute(tool_name, tool_args, self.policy).await {
                    Ok(output) => {
                        let obs = if output.success {
                            output.content
                        } else {
                            output.error.unwrap_or_else(|| "Unknown error".to_string())
                        };
                        
                        step.observation = Some(obs.clone());
                        history.push(step);

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
