//! Waiver Exchange Production Service
//!
//! This is the main entry point for the Waiver Exchange trading platform.
//! It initializes all components, starts the SimulationClock, and provides
//! graceful shutdown handling.

use anyhow::{Context, Result};
use std::sync::Arc;
use tracing::{error, info};

use waiver_exchange_service::{
    graceful_shutdown, initialize_logging, load_configuration, setup_signal_handlers, ServiceState,
};

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize logging first
    initialize_logging()?;

    info!("Starting Waiver Exchange Service v{}", env!("CARGO_PKG_VERSION"));
    info!("Initializing production trading platform...");

    // Load configuration
    let config = load_configuration().context("Failed to load configuration")?;
    info!("Configuration loaded successfully");

    // Create service state
    let service_state = Arc::new(ServiceState::new(config).await?);
    info!("Service state initialized");

    // Recover system state from persistence
    service_state.recover_system_state().await?;
    info!("System state recovery completed");

    // Setup signal handlers for graceful shutdown
    let shutdown_signal = setup_signal_handlers(service_state.clone())?;
    info!("Signal handlers configured");

    // Start the simulation clock in a separate task
    info!("Starting SimulationClock...");
    let clock_handle = {
        let state = service_state.clone();
        tokio::spawn(async move {
            if let Err(e) = state.start_simulation_clock().await {
                error!("SimulationClock failed: {}", e);
            }
        })
    };

    // Start the OrderGateway in a separate task
    info!("Starting OrderGateway...");
    let gateway_handle = {
        let state = service_state.clone();
        tokio::spawn(async move {
            if let Err(e) = state.start_order_gateway().await {
                error!("OrderGateway failed: {}", e);
            }
        })
    };

    // Wait for shutdown signal
    info!("Waiver Exchange Service is running. Press Ctrl+C to shutdown gracefully.");
    let _ = shutdown_signal.await;

    // Graceful shutdown
    info!("Shutdown signal received. Initiating graceful shutdown...");
    graceful_shutdown(service_state, clock_handle, gateway_handle).await?;

    info!("Waiver Exchange Service shutdown complete");
    Ok(())
}
