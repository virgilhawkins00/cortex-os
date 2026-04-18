use crate::adapter::{ChannelAdapter, IncomingMessage};
use anyhow::Result;
use async_trait::async_trait;
use teloxide::prelude::*;
use tokio::sync::mpsc::Sender;
use tracing::{error, info};

pub struct TelegramAdapter {
    token: String,
}

impl TelegramAdapter {
    pub fn new(token: String) -> Self {
        Self { token }
    }
}

#[async_trait]
impl ChannelAdapter for TelegramAdapter {
    fn name(&self) -> &str {
        "Telegram"
    }

    async fn listen(&self, tx: Sender<IncomingMessage>) -> Result<()> {
        let bot = Bot::new(&self.token);
        info!("Starting Telegram listener...");

        let handler = dptree::entry().branch(
            Update::filter_message().endpoint(move |bot: Bot, msg: Message, tx: Sender<IncomingMessage>| async move {
                if let Some(text) = msg.text() {
                    let incoming = IncomingMessage {
                        platform: "telegram".to_string(),
                        channel_id: msg.chat.id.to_string(),
                        user_id: msg.from().map(|u| u.id.to_string()).unwrap_or_default(),
                        text: text.to_string(),
                    };
                    if let Err(e) = tx.send(incoming).await {
                        error!("Failed to forward Telegram message to dispatcher: {}", e);
                    }
                }
                respond(())
            }),
        );

        Dispatcher::builder(bot, handler)
            .dependencies(dptree::deps![tx])
            .enable_ctrlc_handler()
            .build()
            .dispatch()
            .await;

        Ok(())
    }

    async fn send_message(&self, target_id: &str, text: &str) -> Result<()> {
        let bot = Bot::new(&self.token);
        let chat_id: i64 = target_id.parse()?;
        bot.send_message(ChatId(chat_id), text).await?;
        Ok(())
    }
}
