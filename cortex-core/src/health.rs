//! Health Monitor — Circuit Breaker for LLM infrastructure.
//!
//! Periodically pings the Python Brain service via NATS to verify that
//! the LLM engine (Ollama or external APIs) is responsive. If consecutive
//! failures exceed a threshold, triggers an infrastructure-level shutdown
//! to prevent agents from spinning in infinite error loops.

use std::sync::Arc;
use std::time::Duration;
use tokio::time;
use tracing::{error, info, warn};

use crate::nats_bus::CortexBus;
use crate::shutdown::ShutdownController;

/// Maximum consecutive health check failures before triggering circuit breaker.
const MAX_CONSECUTIVE_FAILURES: u32 = 5;
/// Interval between health check pings.
const HEALTH_CHECK_INTERVAL: Duration = Duration::from_secs(30);

/// Background health monitor that watches the Brain service.
pub struct HealthMonitor {
    bus: Arc<CortexBus>,
    shutdown: Arc<ShutdownController>,
}

impl HealthMonitor {
    pub fn new(bus: Arc<CortexBus>, shutdown: Arc<ShutdownController>) -> Self {
        Self { bus, shutdown }
    }

    /// Spawn the monitor as a background task. It will periodically ping
    /// `cortex.brain.health` and count failures.
    pub fn spawn(self) {
        tokio::spawn(async move {
            let mut consecutive_failures: u32 = 0;

            loop {
                // Respect shutdown signal
                if self.shutdown.is_shutting_down() {
                    info!("Health monitor shutting down");
                    return;
                }

                // Ping the brain service
                match self.ping_brain().await {
                    Ok(true) => {
                        if consecutive_failures > 0 {
                            info!(
                                "Brain service recovered after {} failures",
                                consecutive_failures
                            );
                        }
                        consecutive_failures = 0;
                    }
                    Ok(false) => {
                        consecutive_failures += 1;
                        warn!(
                            "Brain health check failed ({}/{})",
                            consecutive_failures, MAX_CONSECUTIVE_FAILURES
                        );
                    }
                    Err(e) => {
                        consecutive_failures += 1;
                        warn!(
                            "Brain health check error ({}/{}): {}",
                            consecutive_failures, MAX_CONSECUTIVE_FAILURES, e
                        );
                    }
                }

                // Circuit breaker: if too many failures, trigger shutdown
                if consecutive_failures >= MAX_CONSECUTIVE_FAILURES {
                    error!(
                        "Circuit breaker tripped — Brain service unreachable after {} attempts. Halting all agents.",
                        consecutive_failures
                    );
                    self.shutdown.trigger();
                    return;
                }

                // Wait before next check, but also listen for shutdown
                let cancel = self.shutdown.token();
                tokio::select! {
                    _ = time::sleep(HEALTH_CHECK_INTERVAL) => {},
                    _ = cancel.cancelled() => {
                        info!("Health monitor received shutdown signal");
                        return;
                    }
                }
            }
        });
    }

    /// Send a lightweight ping to the Brain via NATS request/reply.
    async fn ping_brain(&self) -> anyhow::Result<bool> {
        let response = tokio::time::timeout(
            Duration::from_secs(5),
            self.bus.client().request("cortex.brain.health", "ping".into()),
        )
        .await;

        match response {
            Ok(Ok(msg)) => {
                let body = String::from_utf8_lossy(&msg.payload);
                Ok(body.contains("ok") || body.contains("healthy"))
            }
            Ok(Err(e)) => Err(anyhow::anyhow!("NATS request failed: {}", e)),
            Err(_) => Err(anyhow::anyhow!("Brain health check timed out")),
        }
    }
}
