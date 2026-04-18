use anyhow::Result;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tracing::{info, warn};

use crate::permissions::PermissionPolicy;
use crate::tools::ToolRegistry;

/// A single step in a workflow.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowStep {
    pub name: String,
    pub tool: String,
    pub args: Value,
    pub description: Option<String>,
}

/// A complete workflow definition.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Workflow {
    pub name: String,
    pub version: String,
    pub steps: Vec<WorkflowStep>,
}

pub struct WorkflowRunner<'a> {
    registry: &'a ToolRegistry,
    policy: &'a PermissionPolicy,
}

impl<'a> WorkflowRunner<'a> {
    pub fn new(registry: &'a ToolRegistry, policy: &'a PermissionPolicy) -> Self {
        Self { registry, policy }
    }

    /// Execute a workflow sequence.
    pub async fn execute(&self, workflow: &Workflow) -> Result<WorkflowReport> {
        info!("Starting workflow: {} (v{})", workflow.name, workflow.version);
        
        let mut results = Vec::new();
        let mut overall_success = true;

        for step in &workflow.steps {
            info!("Executing workflow step: {}", step.name);
            if let Some(desc) = &step.description {
                info!("  Description: {}", desc);
            }

            match self.registry.execute(&step.tool, step.args.clone(), self.policy).await {
                Ok(output) => {
                    results.push(StepResult {
                        name: step.name.clone(),
                        success: output.success,
                        content: output.content,
                        error: output.error.clone(),
                    });

                    if !output.success {
                        warn!("Workflow step '{}' failed: {:?}", step.name, output.error);
                        overall_success = false;
                        // For now, we halt on failure to keep it simple
                        break;
                    }
                }
                Err(e) => {
                    results.push(StepResult {
                        name: step.name.clone(),
                        success: false,
                        content: String::new(),
                        error: Some(e.to_string()),
                    });
                    overall_success = false;
                    break;
                }
            }
        }

        Ok(WorkflowReport {
            workflow_name: workflow.name.clone(),
            success: overall_success,
            step_results: results,
        })
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct StepResult {
    pub name: String,
    pub success: bool,
    pub content: String,
    pub error: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct WorkflowReport {
    pub workflow_name: String,
    pub success: bool,
    pub step_results: Vec<StepResult>,
}
