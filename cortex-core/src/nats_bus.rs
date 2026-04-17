use anyhow::Result;
use async_nats::Client;
use serde::{Deserialize, Serialize};
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

    /// Get a reference to the underlying NATS client.
    #[must_use]
    pub fn client(&self) -> &Client {
        &self.client
    }
}
