# SimulationClock Design Document

## 1. Overview

The `SimulationClock` is the **system heartbeat** that drives all logical time progression in the Waiver Exchange. It serves as the central coordinator that autonomously manages tick processing, symbol lifecycle, and system coordination, transforming the system from a collection of manual components into a self-running, production-ready trading platform.

The SimulationClock operates as the **main service loop** in a single binary deployment, ensuring deterministic tick progression, coordinated symbol processing, and seamless integration with all system components. It eliminates the need for manual intervention while maintaining the high-performance, deterministic characteristics required for production trading systems.

### Role in the System

The SimulationClock sits at the **center of the system architecture**, coordinating all time-sensitive operations:

- **Tick Authority:** Single source of truth for logical time progression
- **Symbol Coordination:** Manages Whistle engine lifecycle and processing
- **System Orchestration:** Coordinates ExecutionManager, OrderGateway, and Persistence
- **Autonomous Operation:** Enables self-running production deployment
- **Deterministic Processing:** Ensures consistent, reproducible system behavior

### Core Responsibilities

- **Logical Time Management:** Drive tick progression (`T → T+1 → T+2...`) at configurable cadence
- **Symbol Processing:** Call `Whistle.tick(T)` for all active symbols in deterministic order
- **Lifecycle Coordination:** Manage symbol registration, activation, and eviction at tick boundaries
- **System Integration:** Coordinate with ExecutionManager, OrderGateway, and Persistence
- **Error Recovery:** Handle component failures while maintaining system stability
- **Performance Monitoring:** Track system performance and emit metrics to AnalyticsEngine

## 2. Architecture Overview

### 2.1 System Integration

```
┌─────────────────────────────────────────────────────────────────┐
│                    SimulationClock Service                      │
│  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐            │
│  │Tick Manager │  │Symbol       │  │Error        │            │
│  │(Main Loop)  │  │Coordinator  │  │Recovery     │            │
│  └─────────────┘  └─────────────┘  └─────────────┘            │
└─────────────────────────────────────────────────────────────────┘
                                │
                                │ Coordinates
                                │
┌─────────────────────────────────────────────────────────────────┐
│                    Core Trading System                          │
│  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐            │
│  │SymbolCoord  │  │ExecutionMgr │  │OrderGateway │            │
│  │(Engine      │  │(Event       │  │(API         │            │
│  │Lifecycle)   │  │Processing)  │  │Interface)   │            │
│  └─────────────┘  └─────────────┘  └─────────────┘            │
└─────────────────────────────────────────────────────────────────┘
                                │
                                │ Persistence Integration
                                │
┌─────────────────────────────────────────────────────────────────┐
│                    Persistence & Analytics                      │
│  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐            │
│  │WAL +        │  │Analytics    │  │Admin CLI    │            │
│  │Snapshots    │  │Engine       │  │(WebSocket   │            │
│  │(Durability) │  │(Monitoring) │  │Client)      │            │
│  └─────────────┘  └─────────────┘  └─────────────┘            │
└─────────────────────────────────────────────────────────────────┘
```

### 2.2 Component Responsibilities

| Component | Responsibility | Integration Point |
|-----------|----------------|-------------------|
| **Tick Manager** | Drive logical time progression, manage tick cadence | Main thread loop |
| **Symbol Coordinator** | Manage Whistle engine lifecycle, registration | Symbol registration API |
| **Error Recovery** | Handle component failures, maintain system stability | Error handling policies |
| **SymbolCoordinator** | Engine lifecycle, thread placement, queue management | Registration with SimulationClock |
| **ExecutionManager** | Event processing, WAL writing, fanout to sinks | Event consumption from Whistle |
| **OrderGateway** | External API, WebSocket server, authentication | Order routing to OrderRouter |
| **Persistence** | WAL writing, snapshots, recovery | Automatic integration via ExecutionManager |
| **AnalyticsEngine** | Metrics collection, monitoring, historical analysis | Metrics emission from SimulationClock |

## 3. Functional Requirements

### 3.1 Tick Management

**Tick Progression:**
- **Deterministic cadence:** Configurable tick rate (default: 1kHz = 1ms per tick)
- **Global tick authority:** Single source of truth for system time
- **Boundary enforcement:** All state changes occur at tick boundaries
- **Precise timing:** Microsecond-level timing precision for deterministic behavior

**Tick Processing:**
- **Symbol iteration:** Process all active symbols in deterministic order (by symbol ID)
- **Concurrent processing:** Process symbols in parallel using thread pool
- **Completion coordination:** Wait for all symbols to complete before advancing
- **Error isolation:** Symbol failures don't affect other symbols

### 3.2 Symbol Lifecycle Management

**Registration:**
- **Symbol activation:** Register symbols with SimulationClock before first tick
- **Queue wiring:** Ensure SPSC queues are properly connected
- **Thread placement:** Coordinate with SymbolCoordinator for optimal placement
- **State validation:** Verify symbol is ready for tick processing

**Processing:**
- **Tick delivery:** Call `Whistle.tick(T)` for each registered symbol
- **Event collection:** Gather events from all symbols for coordination
- **Completion tracking:** Ensure all symbols complete before advancing
- **Performance monitoring:** Track processing time and resource usage

**Eviction:**
- **Failure handling:** Mark failed symbols for eviction
- **Boundary enforcement:** Evict symbols only at tick boundaries
- **Resource cleanup:** Coordinate with SymbolCoordinator for cleanup
- **State consistency:** Ensure system state remains consistent

### 3.3 System Integration

**ExecutionManager Coordination:**
- **Event flow:** Whistle → ExecutionManager → WAL/Analytics/WebUI
- **Backpressure handling:** Monitor ExecutionManager queue health
- **Error propagation:** Handle ExecutionManager failures appropriately
- **Metrics emission:** Provide system-level metrics to AnalyticsEngine

**Persistence Integration:**
- **WAL coordination:** Ensure WAL writes complete before tick advancement
- **Snapshot triggering:** Coordinate snapshot creation at safe boundaries
- **Recovery support:** Enable system recovery from WAL + snapshots
- **Data consistency:** Maintain consistency between memory and persistent state

**OrderGateway Integration:**
- **Order routing:** Ensure orders flow through proper channels
- **Market data:** Coordinate real-time market data updates
- **API health:** Monitor OrderGateway health and performance
- **Client management:** Support WebSocket client connections

## 4. Technical Architecture

### 4.1 Core Data Structures

```rust
pub struct SimulationClock {
    // Core state
    current_tick: AtomicU64,
    is_running: AtomicBool,
    
    // Symbol management
    active_symbols: Arc<RwLock<BTreeMap<SymbolId, WhistleHandle>>>,
    symbol_coordinator: Arc<SymbolCoordinator>,
    
    // System integration
    execution_manager: Arc<ExecutionManager>,
    order_gateway: Arc<OrderGateway>,
    persistence: Arc<dyn PersistenceBackend>,
    
    // Configuration
    config: ClockConfig,
    
    // Threading
    thread_pool: ThreadPool,
    metrics_collector: Arc<MetricsCollector>,
}

pub struct ClockConfig {
    pub tick_cadence: Duration,           // Default: 1ms (1kHz)
    pub symbol_ordering: SymbolOrdering,  // Default: BySymbolId
    pub max_concurrent_symbols: usize,    // Default: 100
    pub error_recovery: ErrorRecovery,    // Default: Continue
    pub metrics_interval: Duration,       // Default: 1s
}

pub enum SymbolOrdering {
    BySymbolId,      // Deterministic by symbol ID
    ByActivationTime, // By when symbol was activated
    Custom(Vec<SymbolId>), // Custom ordering
}

pub enum ErrorRecovery {
    Continue,        // Continue processing other symbols
    Halt,           // Halt entire system
    Retry(usize),   // Retry failed symbol N times
}
```

### 4.2 Main Loop Architecture

```rust
impl SimulationClock {
    /// Main service loop - runs on main thread
    pub fn run_clock_loop(&self) -> Result<(), ClockError> {
        info!("Starting SimulationClock main loop");
        
        while self.is_running.load(Ordering::Relaxed) {
            let tick_start = Instant::now();
            let current_tick = self.current_tick.fetch_add(1, Ordering::SeqCst);
            
            // Process all active symbols concurrently
            self.process_tick_concurrent(current_tick)?;
            
            // Emit metrics
            self.emit_metrics(current_tick, tick_start.elapsed())?;
            
            // Wait for next tick (precise timing)
            self.wait_for_next_tick(tick_start)?;
        }
        
        info!("SimulationClock main loop stopped");
        Ok(())
    }
    
    /// Process all symbols concurrently for current tick
    fn process_tick_concurrent(&self, tick: TickId) -> Result<(), ClockError> {
        let symbols = self.get_active_symbols();
        let handles: Vec<_> = symbols.into_iter().collect();
        
        // Process symbols in parallel
        let futures: Vec<_> = handles.into_iter().map(|(symbol_id, handle)| {
            self.process_symbol_tick(symbol_id, handle, tick)
        }).collect();
        
        // Wait for all symbols to complete
        let results = futures::future::join_all(futures).await;
        
        // Handle any failures
        for result in results {
            if let Err(symbol_id) = result {
                self.handle_symbol_failure(symbol_id)?;
            }
        }
        
        Ok(())
    }
    
    /// Process a single symbol tick
    async fn process_symbol_tick(
        &self,
        symbol_id: SymbolId,
        handle: &WhistleHandle,
        tick: TickId,
    ) -> Result<(), SymbolId> {
        let start_time = Instant::now();
        
        // Call Whistle.tick() on the symbol's thread
        let events = handle.engine.tick(tick);
        
        // Send events to ExecutionManager
        if let Err(e) = self.execution_manager.process_events(symbol_id, &events) {
            error!("Failed to process events for symbol {}: {}", symbol_id, e);
            return Err(symbol_id);
        }
        
        // Record metrics
        let processing_time = start_time.elapsed();
        self.metrics_collector.record_symbol_processing(symbol_id, processing_time);
        
        Ok(())
    }
}
```

### 4.3 Symbol Registration API

```rust
impl SimulationClock {
    /// Register a symbol with the SimulationClock
    pub fn register_symbol(&self, symbol_id: SymbolId, handle: WhistleHandle) -> Result<(), ClockError> {
        let mut symbols = self.active_symbols.write().unwrap();
        
        // Validate symbol is ready
        if handle.engine.state() != EngineState::Active {
            return Err(ClockError::SymbolNotReady);
        }
        
        // Add to active symbols (deterministic ordering by ID)
        symbols.insert(symbol_id, handle);
        
        info!("Registered symbol {} with SimulationClock", symbol_id);
        Ok(())
    }
    
    /// Unregister a symbol from the SimulationClock
    pub fn unregister_symbol(&self, symbol_id: SymbolId) -> Result<(), ClockError> {
        let mut symbols = self.active_symbols.write().unwrap();
        
        if symbols.remove(&symbol_id).is_some() {
            info!("Unregistered symbol {} from SimulationClock", symbol_id);
        }
        
        Ok(())
    }
    
    /// Get all active symbols in deterministic order
    fn get_active_symbols(&self) -> Vec<(SymbolId, WhistleHandle)> {
        let symbols = self.active_symbols.read().unwrap();
        symbols.iter().map(|(id, handle)| (*id, handle.clone())).collect()
    }
}
```

### 4.4 Error Handling and Recovery

```rust
impl SimulationClock {
    /// Handle symbol processing failure
    fn handle_symbol_failure(&self, symbol_id: SymbolId) -> Result<(), ClockError> {
        match self.config.error_recovery {
            ErrorRecovery::Continue => {
                warn!("Symbol {} failed, marking for eviction", symbol_id);
                self.mark_symbol_for_eviction(symbol_id);
                Ok(())
            }
            ErrorRecovery::Halt => {
                error!("Symbol {} failed, halting system", symbol_id);
                self.stop();
                Err(ClockError::SystemHalted)
            }
            ErrorRecovery::Retry(max_retries) => {
                if self.get_retry_count(symbol_id) < max_retries {
                    warn!("Symbol {} failed, retrying", symbol_id);
                    self.increment_retry_count(symbol_id);
                    Ok(())
                } else {
                    warn!("Symbol {} failed after {} retries, evicting", symbol_id, max_retries);
                    self.mark_symbol_for_eviction(symbol_id);
                    Ok(())
                }
            }
        }
    }
    
    /// Mark symbol for eviction at next tick boundary
    fn mark_symbol_for_eviction(&self, symbol_id: SymbolId) {
        // Add to eviction queue
        self.eviction_queue.lock().unwrap().push(symbol_id);
        
        // Notify SymbolCoordinator
        if let Err(e) = self.symbol_coordinator.request_eviction(symbol_id) {
            error!("Failed to request eviction for symbol {}: {}", symbol_id, e);
        }
    }
}
```

## 5. Service Architecture

### 5.1 Single Binary Deployment

```rust
// Main service binary: waiver-exchange-service
fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logging
    env_logger::init();
    
    // Load configuration
    let config = load_config()?;
    
    // Initialize persistence
    let persistence = create_persistence_backend(&config.persistence)?;
    
    // Recover system state
    let recovered_state = persistence.recover_system_state()?;
    
    // Initialize core components
    let symbol_coordinator = Arc::new(SymbolCoordinator::new_with_state(
        config.symbol_coordinator,
        recovered_state,
    )?);
    
    let execution_manager = Arc::new(ExecutionManager::new_with_persistence(
        config.execution_manager,
        persistence.clone(),
    )?);
    
    let order_gateway = Arc::new(OrderGateway::new(config.order_gateway)?);
    
    // Initialize SimulationClock
    let simulation_clock = Arc::new(SimulationClock::new(
        symbol_coordinator,
        execution_manager,
        order_gateway,
        persistence,
        config.simulation_clock,
    )?);
    
    // Start background services
    execution_manager.start()?;
    order_gateway.start()?;
    
    // Register signal handlers for graceful shutdown
    setup_signal_handlers(simulation_clock.clone())?;
    
    // Start SimulationClock main loop (blocks forever)
    simulation_clock.run_clock_loop()?;
    
    Ok(())
}
```

### 5.2 Startup Sequence

**Phase 1: Initialization**
1. Load configuration from TOML file
2. Initialize logging and metrics
3. Create persistence backend (LocalPersistence or CloudPersistence)
4. Set up signal handlers for graceful shutdown

**Phase 2: Recovery**
1. Load latest snapshot (if exists)
2. Replay WAL from snapshot tick to current
3. Validate recovered system state
4. Initialize SymbolCoordinator with recovered state

**Phase 3: Component Initialization**
1. Initialize ExecutionManager with persistence integration
2. Initialize OrderGateway with WebSocket server
3. Initialize SimulationClock with all components
4. Register all active symbols with SimulationClock

**Phase 4: Service Startup**
1. Start ExecutionManager background processing
2. Start OrderGateway WebSocket server
3. Start AnalyticsEngine (if enabled)
4. Begin SimulationClock main loop

**Phase 5: Runtime**
1. SimulationClock drives tick processing
2. All components operate autonomously
3. Admin CLI connects as WebSocket client
4. System runs continuously until shutdown

### 5.3 Configuration Management

```toml
[simulation_clock]
tick_cadence_ms = 1                    # 1kHz default
symbol_ordering = "by_symbol_id"       # Deterministic ordering
max_concurrent_symbols = 100           # Thread pool size
error_recovery = "continue"            # Continue on symbol failure
metrics_interval_ms = 1000             # Metrics emission interval

[simulation_clock.performance]
enable_profiling = false               # Performance profiling
max_tick_duration_ms = 10              # Tick timeout
symbol_timeout_ms = 5                  # Per-symbol timeout

[simulation_clock.monitoring]
emit_metrics = true                    # Enable metrics emission
health_check_interval_ms = 5000        # Health check frequency
alert_on_failures = true               # Alert on component failures
```

## 6. Integration with Existing Components

### 6.1 SymbolCoordinator Integration

**Registration Flow:**
```rust
// SymbolCoordinator registers symbols with SimulationClock
impl SymbolCoordinator {
    pub fn register_with_simulation_clock(&self, clock: &SimulationClock) -> Result<(), CoordError> {
        let inner = self.inner.lock().map_err(|_| CoordError::Unknown)?;
        
        for (symbol_id, entry) in inner.registry.get_active_entries() {
            clock.register_symbol(symbol_id, entry.whistle_handle.clone())?;
        }
        
        Ok(())
    }
    
    // Remove manual tick processing - SimulationClock handles this
    // pub fn process_symbol_tick() -> REMOVED
}
```

**Lifecycle Coordination:**
- **Symbol activation:** Register with SimulationClock before first tick
- **Symbol eviction:** Unregister from SimulationClock at tick boundaries
- **Thread management:** Coordinate thread placement with SimulationClock
- **Queue management:** Ensure SPSC queues are properly wired

### 6.2 ExecutionManager Integration

**Event Processing:**
```rust
// ExecutionManager processes events from Whistle engines
impl ExecutionManager {
    pub fn process_events(&self, symbol_id: SymbolId, events: &[EngineEvent]) -> Result<(), ExecutionError> {
        // Process events (existing implementation)
        for event in events {
            self.process_single_event(symbol_id, event)?;
        }
        
        // WAL writing happens automatically
        // Analytics emission happens automatically
        // WebUI updates happen automatically
        
        Ok(())
    }
}
```

**WAL and Snapshot Coordination:**
- **WAL writing:** Automatic for all events processed
- **Snapshot triggering:** Based on order count (every 1000 orders)
- **Recovery support:** Enable system recovery on startup
- **Data consistency:** Maintain consistency between memory and persistent state

### 6.3 OrderGateway Integration

**Order Flow:**
```rust
// OrderGateway routes orders to OrderRouter
impl OrderGateway {
    pub fn handle_order_submission(&self, order: OrderRequest) -> Result<OrderResponse, GatewayError> {
        // Validate order
        self.validate_order(&order)?;
        
        // Route to OrderRouter
        self.order_router.route_order(order)?;
        
        // Order will be processed by SimulationClock on next tick
        Ok(OrderResponse::accepted())
    }
}
```

**Market Data Broadcasting:**
- **Real-time updates:** Broadcast order book changes to all clients
- **WebSocket management:** Handle client connections and subscriptions
- **API health:** Monitor and report health status
- **Rate limiting:** Enforce per-client rate limits

### 6.4 Admin CLI Refactor

**WebSocket Client Architecture:**
```rust
// Admin CLI becomes a pure WebSocket client
pub struct AdminCli {
    order_client: WebSocketClient,      // For order submission
    market_data_client: WebSocketClient, // For order book updates
    api_key: String,                    // Admin API key
}

impl AdminCli {
    pub fn new() -> Result<Self, CliError> {
        let api_key = load_admin_api_key()?;
        
        let order_client = WebSocketClient::connect("ws://localhost:8080/orders")?;
        let market_data_client = WebSocketClient::connect("ws://localhost:8080/market-data")?;
        
        Ok(Self {
            order_client,
            market_data_client,
            api_key,
        })
    }
    
    pub fn submit_order(&self, order: OrderRequest) -> Result<(), String> {
        // Authenticate
        self.order_client.authenticate(&self.api_key)?;
        
        // Submit order
        self.order_client.send_order(order)?;
        
        Ok(())
    }
    
    pub fn display_live_dashboard(&self, symbol_id: SymbolId) {
        // Subscribe to market data
        self.market_data_client.subscribe_symbol(symbol_id)?;
        
        // Display real-time updates
        loop {
            if let Some(update) = self.market_data_client.receive_update()? {
                self.display_order_book_update(update);
            }
        }
    }
}
```

**Removed Components:**
- ❌ SystemState management
- ❌ Direct SymbolCoordinator interaction
- ❌ Manual tick advancement
- ❌ Whistle engine creation
- ❌ System initialization

**New Components:**
- ✅ WebSocket client connections
- ✅ API authentication
- ✅ Real-time market data subscription
- ✅ Order submission via API
- ✅ Pure consumer interface

## 7. Performance Requirements

### 7.1 Latency Targets

| Metric | Target | Notes |
|--------|--------|-------|
| Tick processing latency | < 1ms | End-to-end tick processing |
| Symbol processing latency | < 500μs | Per-symbol tick processing |
| Event processing latency | < 100μs | Event flow to ExecutionManager |
| System startup time | < 30s | Full system recovery and startup |

### 7.2 Throughput Targets

| Metric | Target | Notes |
|--------|--------|-------|
| Tick rate | 1kHz | 1000 ticks per second |
| Symbol processing | 100+ symbols/tick | Concurrent symbol processing |
| Event throughput | 1M+ events/sec | Through ExecutionManager |
| Order processing | 100K+ orders/sec | End-to-end order processing |

### 7.3 Resource Requirements

| Resource | Requirement | Notes |
|----------|-------------|-------|
| CPU | 4-8 cores | Main loop + thread pool |
| Memory | 8-16GB | Symbol state + buffers |
| Storage | 100GB+ | WAL + snapshots + logs |
| Network | 1Gbps | WebSocket connections |

## 8. Error Handling and Recovery

### 8.1 Error Categories

**Symbol-Level Errors:**
- **Whistle engine crashes:** Mark for eviction, continue processing
- **Queue overflow:** Reject orders, continue processing
- **Memory allocation failures:** Evict symbol, continue processing
- **Processing timeouts:** Mark for eviction, continue processing

**System-Level Errors:**
- **ExecutionManager failures:** Halt system (critical component)
- **Persistence failures:** Halt system (data integrity critical)
- **OrderGateway failures:** Continue trading, log errors
- **AnalyticsEngine failures:** Continue trading, log errors

**Infrastructure Errors:**
- **Thread pool exhaustion:** Scale up or reject new symbols
- **Memory exhaustion:** Evict least-used symbols
- **Disk space exhaustion:** Halt system (persistence critical)
- **Network failures:** Continue trading, log errors

### 8.2 Recovery Strategies

**Symbol Recovery:**
```rust
impl SimulationClock {
    fn handle_symbol_failure(&self, symbol_id: SymbolId, error: SymbolError) {
        match error {
            SymbolError::EngineCrash => {
                // Mark for eviction, continue processing others
                self.mark_symbol_for_eviction(symbol_id);
                self.metrics_collector.record_symbol_crash(symbol_id);
            }
            SymbolError::ProcessingTimeout => {
                // Mark for eviction, continue processing others
                self.mark_symbol_for_eviction(symbol_id);
                self.metrics_collector.record_symbol_timeout(symbol_id);
            }
            SymbolError::QueueOverflow => {
                // Reject new orders, continue processing
                self.symbol_coordinator.reject_new_orders(symbol_id);
                self.metrics_collector.record_queue_overflow(symbol_id);
            }
        }
    }
}
```

**System Recovery:**
```rust
impl SimulationClock {
    fn handle_system_failure(&self, error: SystemError) -> Result<(), ClockError> {
        match error {
            SystemError::ExecutionManagerFailure => {
                error!("ExecutionManager failed, halting system");
                self.stop();
                Err(ClockError::SystemHalted)
            }
            SystemError::PersistenceFailure => {
                error!("Persistence failed, halting system");
                self.stop();
                Err(ClockError::SystemHalted)
            }
            SystemError::OrderGatewayFailure => {
                warn!("OrderGateway failed, continuing without API");
                self.metrics_collector.record_gateway_failure();
                Ok(())
            }
        }
    }
}
```

### 8.3 Graceful Shutdown

```rust
impl SimulationClock {
    pub fn shutdown(&self) -> Result<(), ClockError> {
        info!("Initiating graceful shutdown");
        
        // Stop accepting new ticks
        self.is_running.store(false, Ordering::Relaxed);
        
        // Wait for current tick to complete
        self.wait_for_tick_completion().await?;
        
        // Stop background services
        self.execution_manager.stop()?;
        self.order_gateway.stop()?;
        
        // Take final snapshot
        self.take_final_snapshot()?;
        
        info!("Graceful shutdown completed");
        Ok(())
    }
}
```

## 9. Monitoring and Observability

### 9.1 Metrics Collection

**System Metrics:**
```rust
pub struct ClockMetrics {
    // Tick metrics
    pub current_tick: u64,
    pub tick_duration_ns: u64,
    pub tick_rate_hz: f64,
    
    // Symbol metrics
    pub active_symbols: u32,
    pub symbols_processed: u32,
    pub symbol_failures: u32,
    
    // Performance metrics
    pub avg_tick_duration_ns: u64,
    pub max_tick_duration_ns: u64,
    pub p95_tick_duration_ns: u64,
    pub p99_tick_duration_ns: u64,
    
    // System health
    pub system_uptime_seconds: u64,
    pub total_ticks_processed: u64,
    pub total_events_processed: u64,
}
```

**Emission to AnalyticsEngine:**
```rust
impl SimulationClock {
    fn emit_metrics(&self, tick: TickId, tick_duration: Duration) -> Result<(), ClockError> {
        let metrics = ClockMetrics {
            current_tick: tick,
            tick_duration_ns: tick_duration.as_nanos() as u64,
            tick_rate_hz: self.calculate_tick_rate(),
            active_symbols: self.get_active_symbol_count(),
            symbols_processed: self.get_symbols_processed_count(),
            symbol_failures: self.get_symbol_failure_count(),
            avg_tick_duration_ns: self.calculate_avg_tick_duration(),
            max_tick_duration_ns: self.get_max_tick_duration(),
            p95_tick_duration_ns: self.calculate_p95_tick_duration(),
            p99_tick_duration_ns: self.calculate_p99_tick_duration(),
            system_uptime_seconds: self.get_uptime_seconds(),
            total_ticks_processed: tick,
            total_events_processed: self.get_total_events_processed(),
        };
        
        // Send to AnalyticsEngine
        self.analytics_engine.emit_clock_metrics(metrics)?;
        
        Ok(())
    }
}
```

### 9.2 Health Checks

**System Health Monitoring:**
```rust
impl SimulationClock {
    pub fn health_check(&self) -> HealthStatus {
        let mut status = HealthStatus::healthy();
        
        // Check tick processing
        if self.get_avg_tick_duration() > Duration::from_millis(10) {
            status.add_warning("High tick processing latency");
        }
        
        // Check symbol failures
        if self.get_symbol_failure_rate() > 0.01 {
            status.add_warning("High symbol failure rate");
        }
        
        // Check system components
        if !self.execution_manager.is_healthy() {
            status.add_error("ExecutionManager unhealthy");
        }
        
        if !self.order_gateway.is_healthy() {
            status.add_warning("OrderGateway unhealthy");
        }
        
        status
    }
}
```

### 9.3 Logging and Debugging

**Structured Logging:**
```rust
// Tick processing logs
info!(
    tick = tick,
    duration_ns = duration.as_nanos(),
    symbols_processed = symbols_processed,
    events_generated = events_generated,
    "Tick completed"
);

// Symbol failure logs
warn!(
    symbol_id = symbol_id,
    error = %error,
    retry_count = retry_count,
    "Symbol processing failed"
);

// System health logs
error!(
    component = "ExecutionManager",
    error = %error,
    "Critical component failure"
);
```

## 10. Testing Strategy

### 10.1 Unit Tests

**Core Functionality:**
```rust
#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_tick_progression() {
        let clock = SimulationClock::new_test();
        assert_eq!(clock.get_current_tick(), 0);
        
        clock.advance_tick();
        assert_eq!(clock.get_current_tick(), 1);
    }
    
    #[test]
    fn test_symbol_registration() {
        let clock = SimulationClock::new_test();
        let handle = create_test_whistle_handle();
        
        clock.register_symbol(1, handle).unwrap();
        assert!(clock.is_symbol_active(1));
        
        clock.unregister_symbol(1).unwrap();
        assert!(!clock.is_symbol_active(1));
    }
    
    #[test]
    fn test_concurrent_symbol_processing() {
        let clock = SimulationClock::new_test();
        let handles = create_multiple_test_handles(10);
        
        for (id, handle) in handles {
            clock.register_symbol(id, handle).unwrap();
        }
        
        let start = Instant::now();
        clock.process_tick_concurrent(1).await.unwrap();
        let duration = start.elapsed();
        
        // Should process all symbols concurrently
        assert!(duration < Duration::from_millis(100));
    }
    
    #[test]
    fn test_error_recovery() {
        let clock = SimulationClock::new_test_with_recovery(ErrorRecovery::Continue);
        let failing_handle = create_failing_whistle_handle();
        
        clock.register_symbol(1, failing_handle).unwrap();
        clock.process_tick_concurrent(1).await.unwrap();
        
        // Symbol should be marked for eviction
        assert!(clock.is_symbol_marked_for_eviction(1));
    }
}
```

### 10.2 Integration Tests

**End-to-End Testing:**
```rust
#[tokio::test]
async fn test_end_to_end_with_simulation_clock() {
    // Initialize all components
    let persistence = LocalPersistence::new_test();
    let symbol_coordinator = SymbolCoordinator::new_test();
    let execution_manager = ExecutionManager::new_test(persistence);
    let order_gateway = OrderGateway::new_test();
    let simulation_clock = SimulationClock::new_test(
        symbol_coordinator,
        execution_manager,
        order_gateway,
    );
    
    // Start services
    execution_manager.start().unwrap();
    order_gateway.start().unwrap();
    
    // Submit orders via OrderGateway
    let order = OrderRequest::new(1, Side::Buy, 150, 10, 1);
    order_gateway.submit_order(order).await.unwrap();
    
    // Process tick
    simulation_clock.process_tick_concurrent(1).await.unwrap();
    
    // Verify order was processed
    let order_book = execution_manager.get_order_book(1).unwrap();
    assert_eq!(order_book.bids.len(), 1);
    assert_eq!(order_book.bids[0], (150, 10));
}
```

### 10.3 Performance Tests

**Load Testing:**
```rust
#[tokio::test]
async fn test_high_load_performance() {
    let clock = SimulationClock::new_test();
    let handles = create_multiple_test_handles(100);
    
    // Register 100 symbols
    for (id, handle) in handles {
        clock.register_symbol(id, handle).unwrap();
    }
    
    // Process 1000 ticks
    let start = Instant::now();
    for tick in 1..=1000 {
        clock.process_tick_concurrent(tick).await.unwrap();
    }
    let duration = start.elapsed();
    
    // Should process 1000 ticks in under 2 seconds (1kHz target)
    assert!(duration < Duration::from_secs(2));
    
    // Average tick duration should be under 1ms
    let avg_tick_duration = duration / 1000;
    assert!(avg_tick_duration < Duration::from_millis(1));
}
```

### 10.4 Chaos Tests

**Failure Recovery Testing:**
```rust
#[tokio::test]
async fn test_symbol_failure_recovery() {
    let clock = SimulationClock::new_test_with_recovery(ErrorRecovery::Continue);
    
    // Register mix of working and failing symbols
    for i in 1..=10 {
        let handle = if i % 3 == 0 {
            create_failing_whistle_handle()
        } else {
            create_test_whistle_handle()
        };
        clock.register_symbol(i, handle).unwrap();
    }
    
    // Process multiple ticks
    for tick in 1..=100 {
        clock.process_tick_concurrent(tick).await.unwrap();
    }
    
    // Verify working symbols are still active
    for i in 1..=10 {
        if i % 3 != 0 {
            assert!(clock.is_symbol_active(i));
        } else {
            assert!(clock.is_symbol_marked_for_eviction(i));
        }
    }
}
```

## 11. Implementation Plan

### 11.1 Phase 1: Core SimulationClock (MVP)

**Components:**
- **Tick management** and progression
- **Symbol registration** and lifecycle
- **Concurrent symbol processing**
- **Basic error handling**

**Deliverables:**
- SimulationClock core implementation
- Symbol registration API
- Basic configuration support
- Unit tests for core functionality

### 11.2 Phase 2: System Integration

**Components:**
- **SymbolCoordinator integration** (remove manual tick processing)
- **ExecutionManager coordination** (event processing)
- **Persistence integration** (WAL and snapshots)
- **Admin CLI refactor** (WebSocket client)

**Deliverables:**
- Updated SymbolCoordinator (remove manual methods)
- ExecutionManager integration
- Refactored Admin CLI
- Integration tests

### 11.3 Phase 3: Production Service

**Components:**
- **Single binary service** (waiver-exchange-service)
- **Startup and recovery** sequence
- **Configuration management**
- **Monitoring and metrics**

**Deliverables:**
- Production service binary
- Configuration files
- Startup scripts
- Monitoring integration

### 11.4 Phase 4: Advanced Features

**Components:**
- **Advanced error recovery** (retry policies)
- **Performance optimization** (profiling, tuning)
- **Advanced monitoring** (detailed metrics)
- **Load testing** and validation

**Deliverables:**
- Advanced error handling
- Performance optimizations
- Comprehensive monitoring
- Load testing results

## 12. Migration Strategy

### 12.1 From Manual to Autonomous

**Current State:**
- Admin CLI manually drives system
- Manual tick advancement
- Direct component interaction
- No autonomous operation

**Target State:**
- SimulationClock drives system autonomously
- Automatic tick progression
- Coordinated component interaction
- Self-running production service

**Migration Steps:**
1. **Implement SimulationClock** alongside existing system
2. **Add registration API** to SymbolCoordinator
3. **Refactor Admin CLI** to use WebSocket client
4. **Create service binary** with SimulationClock
5. **Test end-to-end** with new architecture
6. **Deploy and validate** production system

### 12.2 Backward Compatibility

**Transition Period:**
- **Dual mode operation:** Support both manual and autonomous modes
- **Configuration flag:** Enable/disable SimulationClock
- **Gradual migration:** Move components one by one
- **Rollback capability:** Revert to manual mode if needed

**Configuration:**
```toml
[simulation_clock]
enabled = true                    # Enable SimulationClock
fallback_to_manual = false        # Allow fallback to manual mode
migration_mode = "autonomous"     # "manual", "autonomous", "dual"
```

## 13. Future Enhancements

### 13.1 Advanced Scheduling

**Dynamic Tick Cadence:**
- **Load-based adjustment:** Increase/decrease tick rate based on load
- **Symbol-specific timing:** Different tick rates for different symbols
- **Adaptive scheduling:** Optimize scheduling based on symbol activity

**Advanced Ordering:**
- **Priority-based ordering:** Process high-priority symbols first
- **Load balancing:** Distribute processing across available threads
- **Custom scheduling policies:** Pluggable scheduling algorithms

### 13.2 Performance Optimizations

**Memory Optimization:**
- **Arena allocation:** Pre-allocate memory pools for symbols
- **Memory mapping:** Use memory-mapped files for large datasets
- **Garbage collection:** Implement custom GC for symbol state

**CPU Optimization:**
- **SIMD instructions:** Use vectorized operations for bulk processing
- **CPU affinity:** Pin threads to specific CPU cores
- **NUMA awareness:** Optimize memory access patterns

### 13.3 Advanced Monitoring

**Real-time Dashboards:**
- **Web-based monitoring:** Real-time system status dashboard
- **Performance visualization:** Charts and graphs for system metrics
- **Alert management:** Configurable alerts and notifications

**Machine Learning:**
- **Anomaly detection:** Detect unusual system behavior
- **Predictive scaling:** Predict when to scale resources
- **Performance optimization:** ML-based performance tuning

---

## 14. Conclusion

The SimulationClock design provides a comprehensive solution for transforming the Waiver Exchange from a collection of manual components into a self-running, production-ready trading system. By implementing a centralized tick authority with autonomous operation, the system achieves the performance, determinism, and reliability required for production trading.

The design maintains compatibility with existing components while providing a clear migration path to autonomous operation. The single binary deployment model simplifies operations while the comprehensive monitoring and error handling ensure system reliability.

This design establishes SimulationClock as the **system heartbeat** that enables the Waiver Exchange to operate as a true production trading platform, capable of handling high-frequency trading with deterministic behavior and comprehensive observability.

The phased implementation approach ensures we deliver core functionality quickly while building a foundation for advanced features. The independent service architecture provides flexibility and scalability while maintaining system reliability and performance.

This design document provides the complete blueprint for implementing SimulationClock as the central coordinator that makes the Waiver Exchange a self-sufficient, production-ready trading system.
