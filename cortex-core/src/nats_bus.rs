use anyhow::Result;
use async_nats::Client;
use serde::{Deserialize, Serialize};
use std::time::Duration;
use tracing::info;

/// A task request flowing through the NATS bus.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskRequest {
    pub id: String,
    pub prompt: String,
    pub tool: Option<String>,
    pub args: Option<serde_json::Value>,
}

/// A task result flowing back through the NATS bus.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskResult {
    pub id: String,
    pub status: TaskStatus,
    pub output: String,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum TaskStatus {
    Success,
    Error,
    Denied,
}

// ── Memory-specific request/response types ──────────────────

/// Request to store a memory via cortex.memory.store
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryStoreRequest {
    pub content: String,
    pub wing: String,
    pub room: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<serde_json::Value>,
}

/// Request to search memory via cortex.memory.search
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemorySearchRequest {
    pub query: String,
    #[serde(default = "default_top_k")]
    pub top_k: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub wing: Option<String>,
}

fn default_top_k() -> usize {
    5
}

/// Request to ingest text via cortex.memory.ingest
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryIngestRequest {
    pub text: String,
    pub wing: String,
    pub room: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<serde_json::Value>,
}

/// Request to list memories via cortex.memory.list
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryListRequest {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub wing: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub room: Option<String>,
    #[serde(default = "default_limit")]
    pub limit: usize,
}

fn default_limit() -> usize {
    50
}

/// Request to delete a memory via cortex.memory.delete
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryDeleteRequest {
    pub memory_id: String,
}

/// Request LLM thinking via cortex.brain.think
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BrainThinkRequest {
    pub prompt: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    #[serde(default = "default_true")]
    pub include_memory: bool,
    #[serde(default)]
    pub stream: bool,
}

fn default_true() -> bool {
    true
}

/// The NATS message bus — the nervous system of Cortex OS.
/// All components (CLI, TUI, Core, Memory) communicate through this.
pub struct CortexBus {
    client: Client,
}

impl CortexBus {
    /// Connect to the NATS server with optional auth token.
    pub async fn connect(url: &str, token: Option<&str>) -> Result<Self> {
        let opts = if let Some(t) = token {
            async_nats::ConnectOptions::new().token(t.into())
        } else {
            async_nats::ConnectOptions::new()
        };

        let client = opts.connect(url).await?;
        info!("Connected to NATS at {url}");
        Ok(Self { client })
    }

    /// Publish a task request.
    pub async fn publish_task(&self, subject: &str, task: &TaskRequest) -> Result<()> {
        let payload = serde_json::to_vec(task)?;
        self.client
            .publish(String::from(subject), payload.into())
            .await?;
        Ok(())
    }

    /// Publish a task result.
    pub async fn publish_result(&self, subject: &str, result: &TaskResult) -> Result<()> {
        let payload = serde_json::to_vec(result)?;
        self.client
            .publish(String::from(subject), payload.into())
            .await?;
        Ok(())
    }

    /// Subscribe to a subject and return a stream of messages.
    pub async fn subscribe(
        &self,
        subject: &str,
    ) -> Result<async_nats::Subscriber> {
        let sub = self.client.subscribe(String::from(subject)).await?;
        info!("Subscribed to {subject}");
        Ok(sub)
    }

    /// Send a request and wait for a reply (request/reply pattern).
    ///
    /// This is the primary way the Rust side communicates with the Python
    /// memory/brain service. The Python NATS bridge subscribes to the
    /// subject and responds with a `TaskResult`.
    pub async fn request(
        &self,
        subject: &str,
        payload: &[u8],
        timeout: Duration,
    ) -> Result<TaskResult> {
        let response = tokio::time::timeout(timeout, async {
            self.client
                .request(String::from(subject), payload.to_vec().into())
                .await
        })
        .await
        .map_err(|_| anyhow::anyhow!("NATS request to '{subject}' timed out after {timeout:?}"))?
        .map_err(|e| anyhow::anyhow!("NATS request to '{subject}' failed: {e}"))?;

        let result: TaskResult = serde_json::from_slice(&response.payload)?;
        Ok(result)
    }

    /// Request a memory store operation.
    pub async fn memory_store(&self, req: &MemoryStoreRequest) -> Result<TaskResult> {
        let payload = serde_json::to_vec(req)?;
        self.request("cortex.memory.store", &payload, Duration::from_secs(5))
            .await
    }

    /// Request a memory search operation.
    pub async fn memory_search(&self, req: &MemorySearchRequest) -> Result<TaskResult> {
        let payload = serde_json::to_vec(req)?;
        self.request("cortex.memory.search", &payload, Duration::from_secs(5))
            .await
    }

    /// Request a memory ingest operation.
    pub async fn memory_ingest(&self, req: &MemoryIngestRequest) -> Result<TaskResult> {
        let payload = serde_json::to_vec(req)?;
        self.request("cortex.memory.ingest", &payload, Duration::from_secs(10))
            .await
    }

    /// Request a memory list operation.
    pub async fn memory_list(&self, req: &MemoryListRequest) -> Result<TaskResult> {
        let payload = serde_json::to_vec(req)?;
        self.request("cortex.memory.list", &payload, Duration::from_secs(5))
            .await
    }

    /// Request a memory delete operation.
    pub async fn memory_delete(&self, req: &MemoryDeleteRequest) -> Result<TaskResult> {
        let payload = serde_json::to_vec(req)?;
        self.request("cortex.memory.delete", &payload, Duration::from_secs(5))
            .await
    }

    /// Request LLM thinking with memory context.
    pub async fn brain_think(&self, req: &BrainThinkRequest) -> Result<TaskResult> {
        let payload = serde_json::to_vec(req)?;
        // LLM calls can take a while — longer timeout
        self.request("cortex.brain.think", &payload, Duration::from_secs(120))
            .await
    }

    /// Request a health check from the Python brain service.
    pub async fn brain_health(&self) -> Result<TaskResult> {
        self.request("cortex.brain.health", b"{}", Duration::from_secs(5))
            .await
    }

    /// Get a reference to the underlying NATS client.
    #[must_use]
    pub fn client(&self) -> &Client {
        &self.client
    }
}
