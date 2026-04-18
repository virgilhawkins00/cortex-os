use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;

/// A definition of an agent within a squad.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SquadAgentDef {
    pub role: String,
    pub goal: String,
    pub name: Option<String>,
    pub specialization: Option<String>,
}

/// A Group of agents (Squad) that work together in parallel.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Squad {
    pub name: String,
    pub description: Option<String>,
    pub agents: Vec<SquadAgentDef>,
}

/// The runtime state of a squad agent.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActiveSquadAgent {
    pub id: Uuid,
    pub role: String,
    pub goal: String,
    pub status: String,
}

/// The runtime state of an active squad.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActiveSquad {
    pub id: Uuid,
    pub name: String,
    pub agents: Vec<ActiveSquadAgent>,
}
