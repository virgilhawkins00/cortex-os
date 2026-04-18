use crate::adapter::{ChannelAdapter, IncomingMessage};
use cortex_core::nats_bus::{CortexBus, BrainThinkRequest, TaskStatus};
use anyhow::Result;
use std::sync::Arc;
use tokio::sync::{mpsc, Mutex};
use tracing::{info, error, warn};
use serde_json::{json, Value};
use futures_util::StreamExt;
use std::collections::HashMap;
use std::time::{Duration, Instant};

pub struct GatewayManager {
    bus: Arc<CortexBus>,
    adapters: Vec<Arc<dyn ChannelAdapter>>,
    rate_limit: Arc<Mutex<HashMap<String, (usize, Instant)>>>,
}

impl GatewayManager {
    pub fn new(bus: Arc<CortexBus>) -> Self {
        Self {
            bus,
            adapters: Vec::new(),
            rate_limit: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub fn add_adapter(&mut self, adapter: Arc<dyn ChannelAdapter>) {
        self.adapters.push(adapter);
    }

    pub async fn run(&self) -> Result<()> {
        let (tx, mut rx) = mpsc::channel(100);

        // Start all adapters in separate tasks
        for adapter in &self.adapters {
            let adapter_clone = Arc::clone(adapter);
            let tx_clone = tx.clone();
            tokio::spawn(async move {
                if let Err(e) = adapter_clone.listen(tx_clone).await {
                    error!("Adapter '{}' failed: {}", adapter_clone.name(), e);
                }
            });
        }

        info!("Gateway Manager running with {} adapters", self.adapters.len());

        // Loop for incoming messages from adapters
        while let Some(msg) = rx.recv().await {
            info!("Incoming Message from {}: {}", msg.platform, msg.text);

            // Simple Rate Limiting: 10 messages per minute
            {
                let mut limits = self.rate_limit.lock().await;
                let entry = limits.entry(msg.user_id.clone()).or_insert((0, Instant::now()));

                if entry.1.elapsed() > Duration::from_secs(60) {
                    *entry = (1, Instant::now());
                } else {
                    entry.0 += 1;
                    if entry.0 > 10 {
                        warn!("Rate limit exceeded for user {}", msg.user_id);
                        continue;
                    }
                }
            }
            
            let req = BrainThinkRequest {
                prompt: msg.text,
                model: None,
                include_memory: true,
                stream: false,
                metadata: Some(json!({
                    "platform": msg.platform,
                    "channel_id": msg.channel_id,
                    "user_id": msg.user_id,
                })),
                role: None,
            };

            let bus = Arc::clone(&self.bus);
            let adapters = self.adapters.clone();

            // Audit: Inbound message
            let _ = bus.publish_audit_log(
                "gateway",
                "message_inbound",
                json!({ "platform": msg.platform, "user_id": msg.user_id }),
                Some(&msg.user_id)
            ).await;

            tokio::spawn(async move {
                match bus.brain_think(&req).await {
                    Ok(result) => {
                         if result.status == TaskStatus::Success {
                             if let Ok(brain_output) = serde_json::from_str::<Value>(&result.output) {
                                if let Some(meta) = brain_output.get("metadata") {
                                    let platform = meta["platform"].as_str().unwrap_or(&msg.platform);
                                    let channel_id = meta["channel_id"].as_str().unwrap_or(&msg.channel_id);
                                    let response_text = brain_output["response"].as_str().unwrap_or(&result.output);

                                    if let Some(adapter) = adapters.iter().find(|a| a.name().to_lowercase() == platform.to_lowercase()) {
                                        if let Err(e) = adapter.send_message(channel_id, response_text).await {
                                            error!("Failed to send response back to {}: {}", platform, e);
                                        }
                                    }
                                }
                             }
                        } else {
                            error!("Brain returned error for gateway message: {:?}", result.error);
                        }
                    }
                    Err(e) => error!("Brain think request failed for gateway: {}", e),
                }
            });
        }

        Ok(())
    }
}
