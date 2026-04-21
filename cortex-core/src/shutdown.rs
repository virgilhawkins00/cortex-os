//! Graceful Shutdown — CancellationToken-based lifecycle management.
//!
//! Provides a system-wide shutdown signal that propagates to all active
//! Swarm agents, NATS connections, and background tasks when Ctrl+C is
//! pressed or an unrecoverable error is detected.

use std::sync::Arc;
use tokio_util::sync::CancellationToken;
use tracing::{info, warn};

/// Central shutdown coordinator for the Cortex OS runtime.
#[derive(Clone)]
pub struct ShutdownController {
    token: CancellationToken,
}

impl ShutdownController {
    /// Create a new shutdown controller.
    pub fn new() -> Self {
        Self {
            token: CancellationToken::new(),
        }
    }

    /// Returns a clone of the cancellation token for use in async tasks.
    pub fn token(&self) -> CancellationToken {
        self.token.clone()
    }

    /// Returns true if shutdown has been requested.
    pub fn is_shutting_down(&self) -> bool {
        self.token.is_cancelled()
    }

    /// Trigger an immediate shutdown of all systems.
    pub fn trigger(&self) {
        warn!("Shutdown triggered — cancelling all active tasks");
        self.token.cancel();
    }

    /// Spawn a background task that listens for Ctrl+C and triggers shutdown.
    /// This should be called once from main().
    pub fn spawn_signal_handler(self: &Arc<Self>) {
        let controller = Arc::clone(self);
        tokio::spawn(async move {
            match tokio::signal::ctrl_c().await {
                Ok(()) => {
                    info!("Ctrl+C received — initiating graceful shutdown");
                    controller.trigger();
                }
                Err(e) => {
                    warn!("Failed to listen for Ctrl+C: {}", e);
                }
            }
        });
    }
}

impl Default for ShutdownController {
    fn default() -> Self {
        Self::new()
    }
}
