//! Main OrderGateway implementation

use crate::auth::AuthManager;
use crate::config::GatewayConfig;
use crate::error::{GatewayError, GatewayResult};
use crate::market_data_broadcaster::MarketDataBroadcaster;
use crate::rate_limiter::RateLimiter;
use crate::websocket_handler::WebSocketHandler;

use std::net::SocketAddr;
use std::sync::Arc;
use tokio::net::TcpListener;
use tokio::sync::RwLock;
use tracing::{error, info, warn};

// OrderRouter integration
use order_router::{
    CoordError as OrderRouterCoordError, OrderRouter, ReadyAtTick as OrderRouterReadyAtTick,
    RouterConfig, SymbolCoordinatorApi as OrderRouterApi,
};
use player_registry::PlayerRegistry;
use symbol_coordinator::{CoordError, SymbolCoordinator, SymbolCoordinatorApi};
use account_service::AccountService;

/// Adapter to make SymbolCoordinator compatible with OrderRouter's trait
struct SymbolCoordinatorAdapter {
    coordinator: Arc<SymbolCoordinator>,
}

impl OrderRouterApi for SymbolCoordinatorAdapter {
    fn ensure_active(
        &self,
        symbol_id: u32,
    ) -> Result<OrderRouterReadyAtTick, OrderRouterCoordError> {
        match self.coordinator.ensure_active(symbol_id) {
            Ok(ready_at) => {
                // Convert between the different ReadyAtTick types
                Ok(OrderRouterReadyAtTick {
                    next_tick: ready_at.next_tick,
                    queue_writer: order_router::OrderQueueWriter {
                        queue: ready_at.queue_writer.queue,
                    },
                })
            }
            Err(CoordError::Capacity) => Err(OrderRouterCoordError::Capacity),
            Err(CoordError::Faulted) => Err(OrderRouterCoordError::Faulted),
            Err(CoordError::Unknown) => Err(OrderRouterCoordError::Unknown),
        }
    }

    fn release_if_idle(&self, symbol_id: u32) {
        self.coordinator.release_if_idle(symbol_id);
    }
}

/// Main OrderGateway service
pub struct OrderGateway {
    /// Gateway configuration
    config: GatewayConfig,

    /// Authentication manager
    auth_manager: Arc<AuthManager>,

    /// Rate limiter
    rate_limiter: Arc<RateLimiter>,

    /// Market data broadcaster
    market_data_broadcaster: Arc<MarketDataBroadcaster>,

    /// Order router for routing orders to trading engines
    order_router: Arc<RwLock<OrderRouter>>,

    /// Symbol coordinator for managing trading engines
    symbol_coordinator: Arc<SymbolCoordinator>,

    /// Player registry for mapping player names to symbol IDs
    player_registry: Arc<RwLock<PlayerRegistry>>,

    /// Account service for balance and position validation
    account_service: Arc<AccountService>,

    /// Connection count
    connection_count: Arc<RwLock<usize>>,

    /// Running state
    is_running: Arc<RwLock<bool>>,
}

impl OrderGateway {
    /// Create a new OrderGateway
    pub fn new(
        config: GatewayConfig,
        symbol_coordinator: Arc<SymbolCoordinator>,
        player_registry: PlayerRegistry,
        account_service: Arc<AccountService>,
    ) -> Self {
        let auth_manager = Arc::new(AuthManager::new(account_service.clone()));
        let rate_limiter = Arc::new(RateLimiter::new(config.rate_limits.clone()));
        let market_data_broadcaster = Arc::new(MarketDataBroadcaster::new());

        // Create OrderRouter with default configuration
        let router_config = RouterConfig::default();
        let mut order_router = OrderRouter::new(router_config);

        // Set the SymbolCoordinator in the OrderRouter using the adapter
        let adapter = SymbolCoordinatorAdapter { coordinator: symbol_coordinator.clone() };
        let coordinator_box: Box<dyn OrderRouterApi> = Box::new(adapter);
        order_router.set_coordinator(coordinator_box);

        Self {
            config,
            auth_manager,
            rate_limiter,
            market_data_broadcaster,
            order_router: Arc::new(RwLock::new(order_router)),
            symbol_coordinator,
            player_registry: Arc::new(RwLock::new(player_registry)),
            account_service,
            connection_count: Arc::new(RwLock::new(0)),
            is_running: Arc::new(RwLock::new(false)),
        }
    }

    /// Start the OrderGateway server
    pub async fn start(&self) -> GatewayResult<()> {
        let addr = self
            .config
            .server_addr()
            .map_err(|e| GatewayError::Config(format!("Invalid server address: {e}")))?;

        info!("Starting OrderGateway on {}", addr);

        // Create TCP listener
        let listener = TcpListener::bind(addr).await?;

        // Mark as running
        {
            let mut running = self.is_running.write().await;
            *running = true;
        }

        info!("OrderGateway started successfully on {}", addr);

        // Start background tasks
        let _cleanup_task = self.start_cleanup_task();
        let _market_data_task = self.start_market_data_task();

        // Main connection loop
        loop {
            match listener.accept().await {
                Ok((stream, peer_addr)) => {
                    if let Err(e) = self.handle_connection(stream, peer_addr).await {
                        error!("Failed to handle connection from {}: {}", peer_addr, e);
                    }
                }
                Err(e) => {
                    error!("Failed to accept connection: {}", e);
                }
            }
        }
    }

    /// Handle a new WebSocket connection
    async fn handle_connection(
        &self,
        stream: tokio::net::TcpStream,
        peer_addr: SocketAddr,
    ) -> GatewayResult<()> {
        info!("New connection from {}", peer_addr);

        // Check connection limit
        {
            let count = self.connection_count.read().await;
            if *count >= self.config.server.max_connections {
                warn!("Connection limit reached, rejecting connection from {}", peer_addr);
                return Err(GatewayError::Connection("Connection limit exceeded".to_string()));
            }
        }

        // Increment connection count
        {
            let mut count = self.connection_count.write().await;
            *count += 1;
        }

        // Create WebSocket handler
        let mut handler = WebSocketHandler::new(
            peer_addr,
            self.auth_manager.clone(),
            self.rate_limiter.clone(),
            self.market_data_broadcaster.clone(),
            self.order_router.clone(),
            self.symbol_coordinator.clone(),
            self.player_registry.clone(),
            self.account_service.clone(),
        );

        // Handle the connection
        let connection_count = self.connection_count.clone();
        tokio::spawn(async move {
            if let Err(e) = handler.handle(stream).await {
                error!("WebSocket handler error: {}", e);
            }

            // Decrement connection count
            let mut count = connection_count.write().await;
            *count = count.saturating_sub(1);
        });

        Ok(())
    }

    /// Start the cleanup task for expired sessions
    fn start_cleanup_task(&self) -> tokio::task::JoinHandle<()> {
        let auth_manager = self.auth_manager.clone();
        let is_running = self.is_running.clone();

        tokio::spawn(async move {
            let mut interval = tokio::time::interval(std::time::Duration::from_secs(60));

            loop {
                interval.tick().await;

                // Check if we should stop
                {
                    let running = is_running.read().await;
                    if !*running {
                        break;
                    }
                }

                // Clean up expired sessions (1 hour timeout)
                auth_manager.cleanup_expired_sessions(3600).await;
            }
        })
    }

    /// Start the market data broadcasting task
    fn start_market_data_task(&self) -> tokio::task::JoinHandle<()> {
        let broadcaster = self.market_data_broadcaster.clone();
        let is_running = self.is_running.clone();

        tokio::spawn(async move {
            let mut interval = tokio::time::interval(std::time::Duration::from_millis(100));

            loop {
                interval.tick().await;

                // Check if we should stop
                {
                    let running = is_running.read().await;
                    if !*running {
                        break;
                    }
                }

                // Broadcast market data updates
                if let Err(e) = broadcaster.broadcast_updates().await {
                    error!("Market data broadcast error: {}", e);
                }
            }
        })
    }

    /// Stop the OrderGateway
    pub async fn stop(&self) -> GatewayResult<()> {
        info!("Stopping OrderGateway...");

        // Mark as not running
        {
            let mut running = self.is_running.write().await;
            *running = false;
        }

        // Note: Individual connections will close when their tasks complete
        // We don't need to explicitly close them here since they're handled in separate tasks

        info!("OrderGateway stopped");
        Ok(())
    }

    /// Get the number of active connections
    pub async fn connection_count(&self) -> usize {
        let count = self.connection_count.read().await;
        *count
    }

    /// Get the market data broadcaster
    pub fn market_data_broadcaster(&self) -> Arc<MarketDataBroadcaster> {
        self.market_data_broadcaster.clone()
    }
}
