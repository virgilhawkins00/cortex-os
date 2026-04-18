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

use crate::registry::AgentRegistry;
use crate::squad::{Squad, ActiveSquad, ActiveSquadAgent};

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
    agent_registry: Arc<AgentRegistry>,
    policy: Arc<PermissionPolicy>,
    active_agents: Arc<Mutex<HashMap<Uuid, String>>>, // ID -> Role
    active_squads: Arc<Mutex<HashMap<Uuid, ActiveSquad>>>, // ID -> Squad State
}

impl SwarmManager {
    pub fn new(
        bus: Arc<CortexBus>,
        registry: Arc<ToolRegistry>,
        agent_registry: Arc<AgentRegistry>,
        policy: Arc<PermissionPolicy>,
    ) -> Self {
        Self {
            bus,
            registry,
            agent_registry,
            policy,
            active_agents: Arc::new(Mutex::new(HashMap::new())),
            active_squads: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// Spawn a new specialized agent to perform a task.
    pub async fn spawn_agent(&self, role: &str, goal: &str) -> Result<AgentResult> {
        let id = Uuid::new_v4();
        {
            let mut agents = self.active_agents.lock().await;
            agents.insert(id, role.to_string());
        }

        // 1. Prepare registry and specialization
        let (agent_registry_arc, specialization) = self.prepare_agent_registry(role);

        let mut agent = Agent::new(self.bus.clone(), agent_registry_arc, self.policy.clone())
            .with_role(role);
        
        if let Some(spec) = specialization {
            agent = agent.with_specialization(&spec);
        }
        
        // Run the agent loop
        let result = agent.run(goal).await?;

        // Cleanup
        {
            let mut agents = self.active_agents.lock().await;
            agents.remove(&id);
        }

        Ok(result)
    }

    /// Spawn a squad of agents in parallel.
    pub async fn spawn_squad(&self, squad_name: &str) -> Result<Uuid> {
        let squad = {
            let squads = self.agent_registry.squads.read().unwrap();
            squads.get(squad_name).cloned().ok_or_else(|| anyhow::anyhow!("Squad not found: {}", squad_name))?
        };

        let squad_id = Uuid::new_v4();
        let mut active_squad = ActiveSquad {
            id: squad_id,
            name: squad.name.clone(),
            agents: Vec::new(),
        };

        for agent_def in &squad.agents {
            let agent_id = Uuid::new_v4();
            active_squad.agents.push(ActiveSquadAgent {
                id: agent_id,
                role: agent_def.role.clone(),
                goal: agent_def.goal.clone(),
                status: "starting".into(),
            });
        }

        let agent_ids: Vec<Uuid> = active_squad.agents.iter().map(|a| a.id).collect();

        {
            let mut squads = self.active_squads.lock().await;
            squads.insert(squad_id, active_squad);
        }

        // Spawn actual tasks
        for (i, agent_def) in squad.agents.into_iter().enumerate() {
            let manager = self.clone();
            let squad_id = squad_id;
            let agent_id = agent_ids[i];
            
            tokio::spawn(async move {
                {
                    let mut squads = manager.active_squads.lock().await;
                    if let Some(s) = squads.get_mut(&squad_id) {
                        if let Some(a) = s.agents.iter_mut().find(|a| a.id == agent_id) {
                            a.status = "running".into();
                        }
                    }
                }

                let _ = manager.spawn_agent(&agent_def.role, &agent_def.goal).await;

                {
                    let mut squads = manager.active_squads.lock().await;
                    if let Some(s) = squads.get_mut(&squad_id) {
                        if let Some(a) = s.agents.iter_mut().find(|a| a.id == agent_id) {
                            a.status = "finished".into();
                        }
                    }
                }
            });
        }

        Ok(squad_id)
    }

    /// Helper to prepare a ToolRegistry with default, global, and specialized tools.
    fn prepare_agent_registry(&self, role: &str) -> (Arc<ToolRegistry>, Option<String>) {
        let mut reg = ToolRegistry::with_defaults(crate::sandbox::Sandbox::default(), self.bus.clone());

        // 1. Add Global MCP servers
        let global_mcps = self.agent_registry.global_mcp_servers.read().unwrap();
        for mcp in global_mcps.iter() {
            let tool = crate::tools::mcp::McpTool {
                name: mcp.name.clone(),
                description: format!("Global MCP Tool: {}", mcp.name),
                command: mcp.command.clone(),
                args: mcp.args.clone(),
            };
            reg.register(Box::new(tool));
        }

        // 2. Get configuration for this role if it exists
        if let Some(config) = self.agent_registry.get_config(role) {
            // Add Role-specific MCP servers
            if let Some(ref mcps) = config.mcp_servers {
                for mcp in mcps {
                    // Avoid double-registering if it's already in global
                    if !global_mcps.iter().any(|m| m.name == mcp.name) {
                        let tool = crate::tools::mcp::McpTool {
                            name: mcp.name.clone(),
                            description: format!("Role MCP Tool: {}", mcp.name),
                            command: mcp.command.clone(),
                            args: mcp.args.clone(),
                        };
                        reg.register(Box::new(tool));
                    }
                }
            }

            // Add Discover Scripts as tools
            for script_path in config.discovered_scripts {
                let name = script_path.file_stem()
                    .and_then(|s| s.to_str())
                    .unwrap_or("unknown_script")
                    .to_string();
                
                let ext = script_path.extension()
                    .and_then(|s| s.to_str())
                    .unwrap_or("");
                
                let (interpreter, desc) = match ext {
                    "sh" => (None, "Shell script"),
                    "py" => (Some("python3".to_string()), "Python script"),
                    "js" => (Some("node".to_string()), "Node.js script"),
                    _ => (None, "Script"),
                };

                let tool = crate::tools::script::ScriptTool {
                    name: format!("sc_{}", name), // Prefix for scripts
                    description: format!("Custom {}: {}", desc, name),
                    script_path,
                    interpreter,
                };
                reg.register(Box::new(tool));
            }

            (Arc::new(reg), Some(config.specialization.clone()))
        } else {
            (Arc::new(reg), None)
        }
    }

    /// Start the delegation listener loop.
    pub async fn run_delegation_listener(&self) -> Result<()> {
        let mut sub = self.bus.subscribe("cortex.swarm.delegate").await?;
        info!("Swarm delegation listener active on cortex.swarm.delegate");

        while let Some(msg) = sub.next().await {
            let manager = self.clone();

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

                // Spawn agent for the task logic
                let id = Uuid::new_v4();
                {
                    let mut agents = manager.active_agents.lock().await;
                    agents.insert(id, role.to_string());
                }

                // Discovery logic consolidated in prepare_agent_registry
                let (final_registry_arc, specialization) = manager.prepare_agent_registry(role);

                let mut agent = Agent::new(manager.bus.clone(), final_registry_arc, manager.policy.clone()).with_role(role);
                if let Some(spec) = specialization {
                    agent = agent.with_specialization(&spec);
                }

                let result = agent.run(task).await;

                // Cleanup
                {
                    let mut agents = manager.active_agents.lock().await;
                    agents.remove(&id);
                }

                // Send reply
                if let Some(reply) = msg.reply {
                    let reply_subject = reply.to_string();
                    match result {
                        Ok(res) => {
                            let _ = manager.bus.publish(&reply_subject, serde_json::to_vec(&res).unwrap()).await;
                        }
                        Err(e) => {
                            let err_resp = json!({ "status": "error", "error": e.to_string() });
                            let _ = manager.bus.publish(&reply_subject, serde_json::to_vec(&err_resp).unwrap()).await;
                        }
                    }
                }
            });
        }
        Ok(())
    }

    /// Run a listener that reports the current swarm status over NATS.
    pub async fn run_status_listener(&self) -> Result<()> {
        let mut sub = self.bus.subscribe("cortex.swarm.status").await?;
        info!("Swarm status listener active on cortex.swarm.status");

        while let Some(msg) = sub.next().await {
            if let Some(reply) = msg.reply {
                let agents = self.list_active().await;
                let squads = self.active_squads.lock().await.clone();

                // Convert Map<Uuid, String> to Vec<SwarmAgent> for serialization
                let swarm_agents: Vec<Value> = agents.into_iter()
                    .map(|(id, role)| json!({ "id": id.to_string(), "role": role }))
                    .collect();
                
                let squad_list: Vec<Value> = squads.into_values()
                    .map(|s| serde_json::to_value(s).unwrap())
                    .collect();

                let resp = json!({
                    "agents": swarm_agents,
                    "squads": squad_list,
                    "count": swarm_agents.len()
                });
                
                let _ = self.bus.publish(&reply.to_string(), serde_json::to_vec(&resp).unwrap()).await;
            }
        }
        Ok(())
    }

    /// Get list of active agents (ID and Role).
    pub async fn list_active(&self) -> HashMap<Uuid, String> {
        self.active_agents.lock().await.clone()
    }
}
