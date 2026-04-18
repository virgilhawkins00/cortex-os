pub mod discord;
pub mod telegram;

use anyhow::Result;
use async_trait::async_trait;

#[async_trait]
pub trait ChannelAdapter: Send + Sync {
    /// The display name of the platform (e.g. "Discord")
    fn name(&self) -> &str;

    /// Start listening for messages on this platform
    async fn listen(&self, tx: tokio::sync::mpsc::Sender<IncomingMessage>) -> Result<()>;

    /// Send a message back to a specific channel/user on this platform
    async fn send_message(&self, target_id: &str, text: &str) -> Result<()>;
}

#[derive(Debug, Clone)]
pub struct IncomingMessage {
    pub platform: String,
    pub channel_id: String,
    pub user_id: String,
    pub text: String,
}
