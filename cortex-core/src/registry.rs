use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use anyhow::Result;
use std::fs;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpConfig {
    pub name: String,
    pub command: String,
    pub args: Vec<String>,
}

use std::sync::{Arc, RwLock};
use notify::{Watcher, RecursiveMode, Config, RecommendedWatcher};
use tokio::sync::mpsc;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentConfig {
    pub role: String,
    pub specialization: String,
    pub tools: Vec<String>,
    pub mcp_servers: Option<Vec<McpConfig>>,
    #[serde(skip)]
    pub discovered_scripts: Vec<PathBuf>,
}

use crate::squad::Squad;

pub struct AgentRegistry {
    pub agents: Arc<RwLock<HashMap<String, AgentConfig>>>, // Key is the role name
    pub squads: Arc<RwLock<HashMap<String, Squad>>>,       // Key is the squad name
    pub global_mcp_servers: Arc<RwLock<Vec<McpConfig>>>,
}

impl AgentRegistry {
    pub fn new() -> Self {
        Self {
            agents: Arc::new(RwLock::new(HashMap::new())),
            squads: Arc::new(RwLock::new(HashMap::new())),
            global_mcp_servers: Arc::new(RwLock::new(Vec::new())),
        }
    }

    /// Spawns a background task to watch the agents folder for changes.
    pub fn watch(&self, root_path: PathBuf) -> Result<()> {
        let agents = self.agents.clone();
        let squads = self.squads.clone();
        let global_mcp = self.global_mcp_servers.clone();
        let path_to_watch = root_path.clone();

        // Initial scan is usually performed by the caller before calling watch()

        tokio::spawn(async move {
            let (tx, mut rx) = mpsc::channel(10);

            let mut watcher = RecommendedWatcher::new(move |res| {
                let _ = tx.blocking_send(res);
            }, Config::default()).expect("Failed to create watcher");

            watcher.watch(&path_to_watch, RecursiveMode::Recursive).expect("Failed to watch folder");

            tracing::info!("Agent hot-reloading active for {:?}", path_to_watch);

            while let Some(res) = rx.recv().await {
                match res {
                    Ok(_) => {
                        tracing::info!("Detected change in agents folder, re-scanning...");
                        // We need a helper to perform the scan
                        if let Err(e) = AgentRegistry::scan_internal(&path_to_watch, &agents, &squads, &global_mcp) {
                            tracing::error!("Hot reload scan failed: {}", e);
                        }
                    }
                    Err(e) => tracing::error!("Watcher error: {:?}", e),
                }
            }
        });

        Ok(())
    }

    /// Scans a directory for agent subfolders.
    pub fn scan_folder(&self, root_path: &Path) -> Result<()> {
        AgentRegistry::scan_internal(root_path, &self.agents, &self.squads, &self.global_mcp_servers)
    }

    fn scan_internal(
        root_path: &Path, 
        agents_lock: &Arc<RwLock<HashMap<String, AgentConfig>>>,
        squads_lock: &Arc<RwLock<HashMap<String, Squad>>>,
        mcp_lock: &Arc<RwLock<Vec<McpConfig>>>
    ) -> Result<()> {
        if !root_path.exists() || !root_path.is_dir() {
            return Ok(());
        }

        let mut new_agents = HashMap::new();
        let mut new_squads = HashMap::new();
        let mut new_global_mcp = Vec::new();

        for entry in fs::read_dir(root_path)? {
            let entry = entry?;
            let path = entry.path();

            if path.is_dir() {
                let config_path = if path.join("config.json").exists() {
                    path.join("config.json")
                } else if path.join("agent.json").exists() {
                    path.join("agent.json")
                } else {
                    continue;
                };

                let content = fs::read_to_string(config_path)?;
                match serde_json::from_str::<AgentConfig>(&content) {
                    Ok(mut config) => {
                        let role = config.role.clone();
                        
                        // Discover scripts
                        let tools_dir = path.join("tools");
                        if tools_dir.exists() && tools_dir.is_dir() {
                            if let Ok(dir) = fs::read_dir(tools_dir) {
                                for tool_entry in dir {
                                    if let Ok(te) = tool_entry {
                                        let tp = te.path();
                                        if tp.is_file() {
                                            if let Some(ext) = tp.extension() {
                                                let ext_str = ext.to_string_lossy();
                                                if ext_str == "sh" || ext_str == "py" || ext_str == "js" {
                                                    config.discovered_scripts.push(tp);
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }

                        // Accumulate global MCP
                        if let Some(ref mcp_list) = config.mcp_servers {
                            for mcp in mcp_list {
                                if !new_global_mcp.iter().any(|m: &McpConfig| m.name == mcp.name) {
                                    new_global_mcp.push(mcp.clone());
                                }
                            }
                        }

                        new_agents.insert(role, config);
                    }
                    Err(e) => tracing::warn!("Failed to parse agent config in {:?}: {}", path, e),
                }
            }
        }

        // Discover squads
        let squads_dir = root_path.join("squads");
        if squads_dir.exists() && squads_dir.is_dir() {
            for entry in fs::read_dir(squads_dir)? {
                let entry = entry?;
                let path = entry.path();
                if path.is_file() && path.extension().map_or(false, |e| e == "json") {
                    let content = fs::read_to_string(&path)?;
                    match serde_json::from_str::<Squad>(&content) {
                        Ok(squad) => {
                            new_squads.insert(squad.name.clone(), squad);
                        }
                        Err(e) => tracing::warn!("Failed to parse squad config in {:?}: {}", path, e),
                    }
                }
            }
        }

        // Apply changes
        {
            let mut agents = agents_lock.write().map_err(|_| anyhow::anyhow!("RwLock poisoned"))?;
            let mut squads = squads_lock.write().map_err(|_| anyhow::anyhow!("RwLock poisoned"))?;
            let mut global_mcp = mcp_lock.write().map_err(|_| anyhow::anyhow!("RwLock poisoned"))?;
            
            *agents = new_agents;
            *squads = new_squads;
            *global_mcp = new_global_mcp;
        }

        tracing::info!("Scan complete: {} agents found", agents_lock.read().unwrap().len());
        Ok(())
    }

    pub fn get_config(&self, role: &str) -> Option<AgentConfig> {
        let agents = self.agents.read().ok()?;
        agents.get(role).cloned()
    }

    pub fn list_roles(&self) -> Vec<String> {
        let agents = self.agents.read().ok();
        match agents {
            Some(a) => a.keys().cloned().collect(),
            None => Vec::new(),
        }
    }
}
