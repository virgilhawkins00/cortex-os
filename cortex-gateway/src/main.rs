mod adapter;
mod manager;

use crate::adapter::discord::DiscordAdapter;
use crate::adapter::telegram::TelegramAdapter;
use crate::manager::GatewayManager;
use anyhow::Result;
use std::sync::Arc;
use cortex_core::nats_bus::CortexBus;
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> Result<()> {
    dotenvy::dotenv().ok();
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env().add_directive(tracing::Level::INFO.into()))
        .init();

    let nats_url = std::env::var("NATS_URL").unwrap_or_else(|_| "nats://127.0.0.1:4222".to_string());
    let bus = CortexBus::connect(&nats_url, None, None).await?;
    let bus_arc = Arc::new(bus);
    
    // Swarm Setup
    let sandbox = cortex_core::sandbox::Sandbox::default();
    let registry = Arc::new(cortex_core::tools::ToolRegistry::with_defaults(sandbox, Arc::clone(&bus_arc)));
    let policy = Arc::new(cortex_core::permissions::PermissionPolicy::full());
    let swarm = cortex_core::swarm::SwarmManager::new(Arc::clone(&bus_arc), Arc::clone(&registry), Arc::clone(&policy));
    
    // Start delegation listener in background
    let swarm_clone = swarm.clone();
    tokio::spawn(async move {
        if let Err(e) = swarm_clone.run_delegation_listener().await {
            tracing::error!("Swarm delegation listener failed: {}", e);
        }
    });

    let mut manager = GatewayManager::new(bus_arc);

    if let Ok(token) = std::env::var("TELEGRAM_BOT_TOKEN") {
        manager.add_adapter(Arc::new(TelegramAdapter::new(token)));
    }

    if let Ok(token) = std::env::var("DISCORD_TOKEN") {
        manager.add_adapter(Arc::new(DiscordAdapter::new(token)));
    }

    manager.run().await?;

    Ok(())
}
