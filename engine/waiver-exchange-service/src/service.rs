//! Service state management and component initialization

use anyhow::{Context, Result};
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{error, info, warn};

use crate::config::ServiceConfig;
use account_service::{AccountService, AccountServiceConfig};
use execution_manager::ExecutionManager;
use order_gateway::{GatewayConfig, OrderGateway};
use order_router::OrderRouter;
use persistence::PersistenceBackend;
use player_registry::PlayerRegistry;
use simulation_clock::SimulationClock;
use symbol_coordinator::{SymbolCoordinator, SymbolCoordinatorApi};

/// Service state containing all initialized components
pub struct ServiceState {
    /// Service configuration
    pub config: ServiceConfig,

    /// SimulationClock instance
    pub simulation_clock: Arc<RwLock<Option<SimulationClock>>>,

    /// SymbolCoordinator instance
    pub symbol_coordinator: Arc<SymbolCoordinator>,

    /// ExecutionManager instance
    pub execution_manager: Arc<ExecutionManager>,

    /// OrderRouter instance
    pub order_router: Arc<OrderRouter>,

    /// Persistence backend instance
    pub persistence: Arc<dyn PersistenceBackend + Send + Sync>,

    /// OrderGateway instance
    pub order_gateway: Arc<RwLock<Option<OrderGateway>>>,

    /// AccountService instance
    pub account_service: Arc<AccountService>,

    /// PlayerRegistry instance
    pub player_registry: Arc<RwLock<Option<PlayerRegistry>>>,

    /// Service running state
    pub is_running: Arc<RwLock<bool>>,
}

// ServiceState is automatically Send + Sync if all its fields are Send + Sync

impl ServiceState {
    /// Create a new service state with all components initialized
    pub async fn new(config: ServiceConfig) -> Result<Self> {
        info!("Initializing service components...");

        // Create data directory if it doesn't exist
        if !config.service.data_dir.exists() {
            std::fs::create_dir_all(&config.service.data_dir)
                .context("Failed to create data directory")?;
        }

        // Initialize Persistence backend first
        info!("Initializing Persistence backend...");
        let mut persistence_backend =
            persistence::LocalPersistence::new(config.persistence.clone())
                .context("Failed to create Persistence backend")?;

        // Initialize the persistence backend
        persistence_backend
            .initialize()
            .await
            .context("Failed to initialize Persistence backend")?;

        let persistence: Arc<dyn PersistenceBackend> = Arc::new(persistence_backend);

        // Initialize AccountService first (needed by ExecutionManager)
        info!("Initializing AccountService...");
        let account_service_config = AccountServiceConfig::from_env()
            .context("Failed to load AccountService configuration")?;
        let account_svc = Arc::new(
            AccountService::new(account_service_config)
                .await
                .context("Failed to create AccountService")?
        );
        info!("AccountService initialized successfully");

        // Initialize ExecutionManager with persistence integration
        info!("Initializing ExecutionManager...");
        let execution_manager = Arc::new(ExecutionManager::new_with_persistence(
            config.execution_manager.clone(),
            persistence.clone(),
            account_svc.clone(),
        ));

        // Initialize SymbolCoordinator (with ExecutionManager reference)
        info!("Initializing SymbolCoordinator...");
        let symbol_coordinator = Arc::new(SymbolCoordinator::new(
            config.symbol_coordinator.clone(),
            execution_manager.clone(),
        ));

        // Initialize OrderRouter
        info!("Initializing OrderRouter...");
        let order_router = Arc::new(OrderRouter::new(config.order_router.clone()));

        // Initialize SimulationClock
        info!("Initializing SimulationClock...");
        let simulation_clock = SimulationClock::new(
            symbol_coordinator.clone(),
            execution_manager.clone(),
            order_router.clone(),
            persistence.clone(),
            config.clock.clone(),
        )
        .context("Failed to create SimulationClock")?;

        // Initialize PlayerRegistry
        info!("Initializing PlayerRegistry...");
        let mut player_registry = PlayerRegistry::new();
        player_registry
            .load_from_file("data/players/season_projections_2025.json")
            .await
            .map_err(|e| anyhow::anyhow!("Failed to load player data: {}", e))?;
        info!("PlayerRegistry loaded with {} players", player_registry.symbol_count());

        // Initialize OrderGateway
        info!("Initializing OrderGateway...");
        let gateway_config = GatewayConfig::default();
        let order_gateway = OrderGateway::new(
            gateway_config,
            symbol_coordinator.clone(),
            player_registry,
            account_svc.clone()
        );

        let service_state = Self {
            config,
            simulation_clock: Arc::new(RwLock::new(Some(simulation_clock))),
            symbol_coordinator,
            execution_manager,
            order_router,
            persistence,
            order_gateway: Arc::new(RwLock::new(Some(order_gateway))),
            account_service: account_svc,
            player_registry: Arc::new(RwLock::new(None)), // PlayerRegistry is now owned by OrderGateway
            is_running: Arc::new(RwLock::new(false)),
        };

        info!("Service components initialized successfully");
        Ok(service_state)
    }

    /// Recover system state from persistence
    pub async fn recover_system_state(&self) -> Result<u64> {
        info!("Starting system state recovery...");

        // Use the already-initialized persistence backend
        let snapshot = self
            .persistence
            .load_latest_snapshot()
            .await
            .context("Failed to load latest snapshot")?;

        let recovered_tick = if let Some(snapshot) = snapshot {
            info!("Found snapshot at tick {}, recovering state...", snapshot.tick);
            info!("Snapshot ID: {}", snapshot.id);
            info!("Snapshot timestamp: {}", snapshot.timestamp);

            // Restore SymbolCoordinator state from snapshot
            info!("Snapshot contains {} symbols", snapshot.state.active_symbols.len());
            info!("Snapshot contains {} order books", snapshot.state.order_books.len());

            // Log order book details
            for (symbol_id, order_book) in &snapshot.state.order_books {
                info!(
                    "Order book for symbol {}: {} buy orders, {} sell orders",
                    symbol_id,
                    order_book.buy_orders.len(),
                    order_book.sell_orders.len()
                );

                // Log specific order details
                for (price, qty) in &order_book.buy_orders {
                    info!("  Buy order: price={}, qty={}", price, qty);
                }
                for (price, qty) in &order_book.sell_orders {
                    info!("  Sell order: price={}, qty={}", price, qty);
                }
            }

            // Register all symbols from the snapshot with SymbolCoordinator
            for &symbol_id in &snapshot.state.active_symbols {
                // Ensure the symbol is active in SymbolCoordinator
                if let Err(e) = self.symbol_coordinator.ensure_active(symbol_id) {
                    warn!("Failed to restore symbol {}: {:?}", symbol_id, e);
                } else {
                    info!("Restored symbol {}", symbol_id);

                    // Restore order book state for this symbol
                    if let Some(order_book) = snapshot.state.order_books.get(&symbol_id) {
                        if let Err(e) = self.symbol_coordinator.restore_order_book_state(
                            symbol_id,
                            &order_book.buy_orders,
                            &order_book.sell_orders,
                            order_book.last_trade_price,
                            order_book.last_trade_quantity,
                            order_book.last_trade_timestamp,
                        ) {
                            warn!(
                                "Failed to restore order book state for symbol {}: {:?}",
                                symbol_id, e
                            );
                        } else {
                            info!("Restored order book state for symbol {}", symbol_id);
                        }
                    }
                }
            }

            // TODO: Replay WAL from snapshot tick to current
            // For now, we'll just log that we would replay WAL
            info!("Would replay WAL from tick {} to current", snapshot.tick);

            // Return the next tick to continue from
            snapshot.tick + 1
        } else {
            info!("No snapshot found, starting with clean state");
            0 // Start from tick 0
        };

        info!("System state recovery completed, will continue from tick {}", recovered_tick);
        Ok(recovered_tick)
    }

    /// Recreate SimulationClock with the correct initial tick
    pub async fn recreate_simulation_clock(&self, initial_tick: u64) -> Result<()> {
        info!("Recreating SimulationClock with initial tick: {}", initial_tick);

        let new_clock = SimulationClock::new_with_initial_tick(
            self.symbol_coordinator.clone(),
            self.execution_manager.clone(),
            self.order_router.clone(),
            self.persistence.clone(),
            self.config.clock.clone(),
            initial_tick,
        )
        .context("Failed to recreate SimulationClock")?;

        let mut clock_guard = self.simulation_clock.write().await;
        *clock_guard = Some(new_clock);

        info!("SimulationClock recreated successfully");
        Ok(())
    }

    /// Start the simulation clock
    pub async fn start_simulation_clock(&self) -> Result<()> {
        info!("Starting SimulationClock...");

        let mut clock_guard = self.simulation_clock.write().await;
        if let Some(clock) = clock_guard.take() {
            // Mark service as running
            {
                let mut running = self.is_running.write().await;
                *running = true;
            }

            // Start the clock in continuous mode
            if let Err(e) = clock.run_clock_loop().await {
                error!("SimulationClock failed to start: {}", e);

                // Mark service as not running
                {
                    let mut running = self.is_running.write().await;
                    *running = false;
                }

                return Err(e.into());
            }

            info!("SimulationClock stopped");
        } else {
            warn!("SimulationClock was already started or not available");
        }

        Ok(())
    }

    /// Stop the simulation clock
    pub async fn stop_simulation_clock(&self) -> Result<()> {
        info!("Stopping SimulationClock...");

        // Mark service as not running
        {
            let mut running = self.is_running.write().await;
            *running = false;
        }

        // The SimulationClock will stop when it detects the running flag is false
        info!("SimulationClock stop signal sent");
        Ok(())
    }

    /// Start the OrderGateway
    pub async fn start_order_gateway(&self) -> Result<()> {
        info!("Starting OrderGateway...");

        let mut gateway_guard = self.order_gateway.write().await;
        if let Some(gateway) = gateway_guard.take() {
            // Start the gateway in a separate task
            tokio::spawn(async move {
                if let Err(e) = gateway.start().await {
                    error!("OrderGateway failed: {}", e);
                }
            });

            info!("OrderGateway started successfully");
        } else {
            warn!("OrderGateway was already started or not available");
        }

        Ok(())
    }

    /// Stop the OrderGateway
    pub async fn stop_order_gateway(&self) -> Result<()> {
        info!("Stopping OrderGateway...");

        // TODO: Implement proper OrderGateway shutdown
        // For now, the gateway will stop when the service shuts down

        info!("OrderGateway stop signal sent");
        Ok(())
    }

    /// Check if the service is running
    pub async fn is_running(&self) -> bool {
        let running = self.is_running.read().await;
        *running
    }

    /// Get service health status
    pub async fn health_check(&self) -> ServiceHealth {
        let is_running = self.is_running().await;

        if is_running {
            ServiceHealth::Healthy
        } else {
            ServiceHealth::Unhealthy
        }
    }

    /// Graceful shutdown of all components
    pub async fn shutdown(&self) -> Result<()> {
        info!("Initiating graceful shutdown...");

        // Stop the simulation clock
        self.stop_simulation_clock().await?;

        // Shutdown persistence
        info!("Shutting down persistence backend...");
        // Note: PersistenceBackend doesn't have a shutdown method in the current interface
        // This would need to be added if needed

        info!("Graceful shutdown complete");
        Ok(())
    }
}

/// Service health status
#[derive(Debug, Clone, PartialEq)]
pub enum ServiceHealth {
    Healthy,
    Unhealthy,
    Degraded,
}

impl ServiceHealth {
    pub fn is_healthy(&self) -> bool {
        matches!(self, ServiceHealth::Healthy)
    }
}
