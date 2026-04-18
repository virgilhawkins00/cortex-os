use crate::adapter::{ChannelAdapter, IncomingMessage};
use anyhow::Result;
use async_trait::async_trait;
use serenity::all::{GatewayIntents, Message};
use serenity::prelude::*;
use std::sync::Arc;
use tokio::sync::mpsc::Sender;
use tracing::{error, info};

pub struct DiscordAdapter {
    token: String,
}

impl DiscordAdapter {
    pub fn new(token: String) -> Self {
        Self { token }
    }
}

struct Handler {
    tx: Sender<IncomingMessage>,
}

#[async_trait]
impl EventHandler for Handler {
    async fn message(&self, _ctx: Context, msg: Message) {
        if msg.author.bot {
            return;
        }

        let incoming = IncomingMessage {
            platform: "discord".to_string(),
            channel_id: msg.channel_id.to_string(),
            user_id: msg.author.id.to_string(),
            text: msg.content.clone(),
        };

        if let Err(e) = self.tx.send(incoming).await {
            error!("Failed to forward Discord message to dispatcher: {}", e);
        }
    }

    async fn ready(&self, _: Context, ready: serenity::all::Ready) {
        info!("Discord adapter ready: {}", ready.user.name);
    }
}

#[async_trait]
impl ChannelAdapter for DiscordAdapter {
    fn name(&self) -> &str {
        "Discord"
    }

    async fn listen(&self, tx: Sender<IncomingMessage>) -> Result<()> {
        let intents = GatewayIntents::GUILD_MESSAGES 
            | GatewayIntents::DIRECT_MESSAGES 
            | GatewayIntents::MESSAGE_CONTENT;

        let mut client = Client::builder(&self.token, intents)
            .event_handler(Handler { tx })
            .await?;

        info!("Starting Discord listener...");
        client.start().await?;
        Ok(())
    }

    async fn send_message(&self, target_id: &str, text: &str) -> Result<()> {
        // This requires a bit of context or a standalone HTTP client in serenity
        let http = serenity::http::Http::new(&self.token);
        let channel_id: u64 = target_id.parse()?;
        
        // Serenity 0.12+ use newtypes or structs for IDs
        let cid = serenity::model::id::ChannelId::new(channel_id);
        cid.say(&http, text).await?;
        Ok(())
    }
}
