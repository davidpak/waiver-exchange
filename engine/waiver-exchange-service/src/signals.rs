//! Signal handling for graceful shutdown

use anyhow::Result;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::oneshot;
use tokio::time::timeout;
use tracing::{error, info, warn};

use crate::service::ServiceState;

/// Setup signal handlers for graceful shutdown
pub fn setup_signal_handlers(_service_state: Arc<ServiceState>) -> Result<oneshot::Receiver<()>> {
    let (shutdown_tx, shutdown_rx) = oneshot::channel();

    // Handle Ctrl+C (SIGINT)
    tokio::spawn(async move {
        if let Err(e) = tokio::signal::ctrl_c().await {
            error!("Failed to listen for Ctrl+C signal: {}", e);
            return;
        }

        info!("Ctrl+C signal received");
        let _ = shutdown_tx.send(());
    });

    // Handle SIGTERM (Unix only)
    #[cfg(unix)]
    {
        let shutdown_tx = shutdown_tx;
        tokio::spawn(async move {
            use signal_hook::consts::SIGTERM;
            use std::sync::atomic::{AtomicBool, Ordering};
            use std::sync::Arc;

            let shutdown_flag = Arc::new(AtomicBool::new(false));
            let shutdown_flag_clone = shutdown_flag.clone();

            // Register signal handler
            if let Err(e) = signal_hook::flag::register(SIGTERM, shutdown_flag_clone) {
                error!("Failed to register SIGTERM handler: {}", e);
                return;
            }

            // Poll for signal
            loop {
                if shutdown_flag.load(Ordering::Relaxed) {
                    info!("SIGTERM signal received");
                    let _ = shutdown_tx.send(());
                    break;
                }
                tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
            }
        });
    }

    Ok(shutdown_rx)
}

/// Graceful shutdown handler
pub async fn graceful_shutdown(
    service_state: Arc<ServiceState>,
    clock_handle: tokio::task::JoinHandle<()>,
    gateway_handle: tokio::task::JoinHandle<()>,
) -> Result<()> {
    info!("Starting graceful shutdown...");

    // Stop the simulation clock
    if let Err(e) = service_state.stop_simulation_clock().await {
        error!("Failed to stop simulation clock: {}", e);
    }

    // Stop the OrderGateway
    if let Err(e) = service_state.stop_order_gateway().await {
        error!("Failed to stop OrderGateway: {}", e);
    }

    // Wait for the clock task to complete with timeout
    let shutdown_timeout = Duration::from_secs(service_state.config.service.shutdown_timeout_secs);
    match timeout(shutdown_timeout, clock_handle).await {
        Ok(Ok(())) => {
            info!("SimulationClock stopped gracefully");
        }
        Ok(Err(e)) => {
            error!("SimulationClock task failed: {}", e);
        }
        Err(_) => {
            warn!("SimulationClock did not stop within timeout, forcing shutdown");
        }
    }

    // Wait for the gateway task to complete with timeout
    match timeout(shutdown_timeout, gateway_handle).await {
        Ok(Ok(())) => {
            info!("OrderGateway stopped gracefully");
        }
        Ok(Err(e)) => {
            error!("OrderGateway task failed: {}", e);
        }
        Err(_) => {
            warn!("OrderGateway did not stop within timeout, forcing shutdown");
        }
    }

    // Shutdown other components
    if let Err(e) = service_state.shutdown().await {
        error!("Failed to shutdown service components: {}", e);
    }

    info!("Graceful shutdown complete");
    Ok(())
}
