use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;
use uuid::Uuid;
use serde_json::{json, Value};
use tracing::{info, error};
use futures::StreamExt;

use crate::agent::{Agent, AgentResult};
use crate::nats_bus::CortexBus;
use crate::permissions::PermissionPolicy;
use crate::tools::ToolRegistry;

/// An active agent in the swarm.
pub struct SwarmAgent {
    pub id: Uuid,
    pub role: String,
}

/// The SwarmManager orchestrates multiple agents working together.
#[derive(Clone)]
pub struct SwarmManager {
    bus: Arc<CortexBus>,
    registry: Arc<ToolRegistry>,
    policy: Arc<PermissionPolicy>,
    active_agents: Arc<Mutex<HashMap<Uuid, String>>>, // ID -> Role
}

impl SwarmManager {
    pub fn new(bus: Arc<CortexBus>, registry: Arc<ToolRegistry>, policy: Arc<PermissionPolicy>) -> Self {
        Self {
            bus,
            registry,
            policy,
            active_agents: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// Spawn a new specialized agent to perform a task.
    pub async fn spawn_agent(&self, role: &str, goal: &str) -> Result<AgentResult> {
        let id = Uuid::new_v4();
        {
            let mut agents = self.active_agents.lock().await;
            agents.insert(id, role.to_string());
        }

        let agent = Agent::new(self.bus.clone(), self.registry.clone(), self.policy.clone())
            .with_role(role);
        
        // Run the agent loop
        let result = agent.run(goal).await?;

        // Cleanup
        {
            let mut agents = self.active_agents.lock().await;
            agents.remove(&id);
        }

        Ok(result)
    }

    /// Start the delegation listener loop.
    pub async fn run_delegation_listener(&self) -> Result<()> {
        let mut sub = self.bus.subscribe("cortex.swarm.delegate").await?;
        info!("Swarm delegation listener active on cortex.swarm.delegate");

        while let Some(msg) = sub.next().await {
            let bus = self.bus.clone();
            let registry = self.registry.clone();
            let policy = self.policy.clone();
            let active_agents = Arc::clone(&self.active_agents);

            tokio::spawn(async move {
                let data: Value = match serde_json::from_slice(&msg.payload) {
                    Ok(d) => d,
                    Err(e) => {
                        error!("Failed to parse delegation request: {}", e);
                        return;
                    }
                };

                let role = data["role"].as_str().unwrap_or("default");
                let task = data["task"].as_str().unwrap_or("");

                info!("Swarm handling delegation: role={}, goal={}", role, task);

                // Spawn agent for the task
                let id = Uuid::new_v4();
                {
                    let mut agents = active_agents.lock().await;
                    agents.insert(id, role.to_string());
                }

                let agent = Agent::new(bus.clone(), registry.clone(), policy.clone()).with_role(role);
                let result = agent.run(task).await;

                // Cleanup
                {
                    let mut agents = active_agents.lock().await;
                    agents.remove(&id);
                }

                // Send reply
                if let Some(reply) = msg.reply {
                    let reply_subject = reply.to_string();
                    match result {
                        Ok(res) => {
                            let _ = bus.publish(&reply_subject, serde_json::to_vec(&res).unwrap()).await;
                        }
                        Err(e) => {
                            let err_resp = json!({ "status": "error", "error": e.to_string() });
                            let _ = bus.publish(&reply_subject, serde_json::to_vec(&err_resp).unwrap()).await;
                        }
                    }
                }
            });
        }
        Ok(())
    }

    /// Get list of active agents (ID and Role).
    pub async fn list_active(&self) -> HashMap<Uuid, String> {
        self.active_agents.lock().await.clone()
    }
}
