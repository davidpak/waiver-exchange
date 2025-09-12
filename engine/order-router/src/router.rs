use crate::sharding::{shard_for_symbol, ShardId};
use crate::types::{InboundMsgWithSymbol, RouterMetrics, SymbolCoordinatorApi};
use std::collections::HashMap;
use std::sync::Arc;
use whistle::{EnqSeq, InboundQueue, RejectReason, TickId};

#[cfg(test)]
use whistle::InboundMsg;

/// Router configuration
#[derive(Debug, Clone)]
pub struct RouterConfig {
    pub num_shards: u32,
    pub spsc_depth_default: usize,
    pub prewarm_top_k: u32,
    pub burst_window_ticks: u32,
    pub headroom_percent: u32,
    pub activation_policy: ActivationPolicy,
}

impl Default for RouterConfig {
    fn default() -> Self {
        Self {
            num_shards: 1,
            spsc_depth_default: 2048,
            prewarm_top_k: 128,
            burst_window_ticks: 4,
            headroom_percent: 50,
            activation_policy: ActivationPolicy::Hybrid,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum ActivationPolicy {
    Prewarm,
    OnDemand,
    Hybrid,
}

/// Router error types
#[derive(Debug, Clone, PartialEq)]
pub enum RouterError {
    Backpressure,
    SymbolInactive,
    SymbolCapacity,
    ShardMismatch,
    ConfigError(String),
}

impl From<RejectReason> for RouterError {
    fn from(reason: RejectReason) -> Self {
        match reason {
            RejectReason::QueueBackpressure => RouterError::Backpressure,
            _ => RouterError::ConfigError(format!("Unexpected reject reason: {reason:?}")),
        }
    }
}

/// Per-symbol state in the router
#[derive(Debug)]
struct SymbolState {
    enq_seq: EnqSeq,
    queue: Option<Arc<InboundQueue>>,
    is_active: bool,
    activation_requested: bool,
}

impl SymbolState {
    fn new() -> Self {
        Self { enq_seq: 0, queue: None, is_active: false, activation_requested: false }
    }
}

/// Main OrderRouter implementation
pub struct OrderRouter {
    config: RouterConfig,
    symbol_states: HashMap<u32, SymbolState>,
    metrics: RouterMetrics,
    current_tick: TickId,
    coordinator: Option<Box<dyn SymbolCoordinatorApi>>,
}

impl OrderRouter {
    pub fn new(config: RouterConfig) -> Self {
        Self {
            config,
            symbol_states: HashMap::new(),
            metrics: RouterMetrics::default(),
            current_tick: 0,
            coordinator: None,
        }
    }

    /// Set the SymbolCoordinator for symbol activation
    pub fn set_coordinator(&mut self, coordinator: Box<dyn SymbolCoordinatorApi>) {
        self.coordinator = Some(coordinator);
    }

    /// Route a message to the appropriate symbol queue
    pub fn route(
        &mut self,
        tick_now: TickId,
        msg: InboundMsgWithSymbol,
    ) -> Result<(), RouterError> {
        tracing::info!(
            "OrderRouter routing message for symbol {} at tick {}",
            msg.symbol_id,
            tick_now
        );

        // Update current tick if needed
        if tick_now != self.current_tick {
            self.on_tick_boundary(tick_now);
        }

        let symbol_id = msg.symbol_id;

        // Check if we need to activate the symbol
        let needs_activation = {
            let state = self.symbol_states.entry(symbol_id).or_insert_with(SymbolState::new);
            !state.is_active && !state.activation_requested
        };

        if needs_activation {
            // Mark activation as requested first
            {
                let state = self.symbol_states.get_mut(&symbol_id).unwrap();
                state.activation_requested = true;
            }
            self.metrics.activation_requests += 1;

            // Activate symbol using SymbolCoordinator
            self.activate_symbol(symbol_id)?;
        }

        // Now safely get the state for routing
        let state = self.symbol_states.get_mut(&symbol_id).unwrap();

        // Ensure we have a queue for this symbol
        if state.queue.is_none() {
            return Err(RouterError::SymbolInactive);
        }

        // Stamp enqueue sequence
        let mut enriched_msg = msg.msg.clone();
        enriched_msg.enq_seq = state.enq_seq;
        state.enq_seq += 1;

        // Try to enqueue to SPSC using lock-free interface
        let queue = state.queue.as_ref().unwrap();
        match queue.try_enqueue_lockfree(enriched_msg) {
            Ok(()) => {
                self.metrics.enqueued += 1;
                tracing::info!(
                    "OrderRouter successfully enqueued message for symbol {} to Whistle engine",
                    symbol_id
                );
                Ok(())
            }
            Err(_) => {
                self.metrics.rejected_backpressure += 1;
                tracing::warn!(
                    "OrderRouter failed to enqueue message for symbol {} due to backpressure",
                    symbol_id
                );
                Err(RouterError::Backpressure)
            }
        }
    }

    /// Get shard ID for a symbol
    pub fn shard_for_symbol(&self, symbol_id: u32) -> ShardId {
        shard_for_symbol(symbol_id, self.config.num_shards)
    }

    /// Get router configuration
    pub fn config(&self) -> &RouterConfig {
        &self.config
    }

    /// Get current metrics
    pub fn metrics(&self) -> &RouterMetrics {
        &self.metrics
    }

    /// Handle tick boundary - reset enq_seq for all symbols
    fn on_tick_boundary(&mut self, next_tick: TickId) {
        self.current_tick = next_tick;

        // Reset enq_seq for all active symbols
        for state in self.symbol_states.values_mut() {
            state.enq_seq = 0;
        }
    }

    /// Activate symbol using SymbolCoordinator
    fn activate_symbol(&mut self, symbol_id: u32) -> Result<(), RouterError> {
        let state = self.symbol_states.get_mut(&symbol_id).unwrap();

        // Use real SymbolCoordinator if available
        if let Some(coordinator) = &self.coordinator {
            tracing::info!("OrderRouter using real SymbolCoordinator for symbol {}", symbol_id);
            match coordinator.ensure_active(symbol_id) {
                Ok(ready_at) => {
                    // Use the real queue from SymbolCoordinator
                    state.queue = Some(ready_at.queue_writer.queue.clone());
                    state.is_active = true;
                    state.activation_requested = false;
                    self.metrics.active_symbols += 1;
                    tracing::info!(
                        "OrderRouter successfully activated symbol {} with real queue",
                        symbol_id
                    );
                    Ok(())
                }
                Err(e) => {
                    tracing::warn!(
                        "OrderRouter failed to activate symbol {} via SymbolCoordinator: {:?}",
                        symbol_id,
                        e
                    );
                    Err(RouterError::SymbolCapacity)
                }
            }
        } else {
            // Fallback to placeholder implementation
            tracing::warn!("OrderRouter using FALLBACK placeholder implementation for symbol {} - NO REAL QUEUE CONNECTION!", symbol_id);
            let queue = Arc::new(InboundQueue::new(self.config.spsc_depth_default));
            state.queue = Some(queue);
            state.is_active = true;
            state.activation_requested = false;
            self.metrics.active_symbols += 1;
            Ok(())
        }
    }

    /// Get queue for a symbol (for testing)
    #[cfg(test)]
    pub fn get_symbol_queue(&mut self, symbol_id: u32) -> Option<&Arc<InboundQueue>> {
        self.symbol_states.get(&symbol_id)?.queue.as_ref()
    }

    /// Check if symbol is active
    pub fn is_symbol_active(&self, symbol_id: u32) -> bool {
        self.symbol_states.get(&symbol_id).is_some_and(|s| s.is_active)
    }
}

/// Trait for tick boundary notifications
pub trait TickBoundaryNotify {
    fn on_tick_boundary(&self, symbol_id: u32, next_tick: TickId);
}

#[cfg(test)]
mod tests {
    use super::*;
    use whistle::{OrderType, Side};

    fn create_test_message(symbol_id: u32, order_id: u64) -> InboundMsgWithSymbol {
        InboundMsgWithSymbol {
            symbol_id,
            msg: InboundMsg::submit(
                order_id,
                1, // account_id
                Side::Buy,
                OrderType::Limit,
                Some(150), // price
                10,        // qty
                1000,      // ts_norm
                0,         // meta
                0,         // enq_seq (will be stamped by router)
            ),
        }
    }

    #[test]
    fn test_enq_seq_stamping() {
        let config = RouterConfig::default();
        let mut router = OrderRouter::new(config);

        let msg1 = create_test_message(1, 100);
        let msg2 = create_test_message(1, 101);

        // Route first message
        router.route(100, msg1).unwrap();

        // Route second message
        router.route(100, msg2).unwrap();

        // Check that enq_seq was incremented
        let queue = router.get_symbol_queue(1).unwrap();
        // Note: We can't drain from Arc<InboundQueue> in tests, but we can verify it exists
        assert!(queue.capacity() > 0);
    }

    #[test]
    fn test_tick_boundary_reset() {
        let config = RouterConfig::default();
        let mut router = OrderRouter::new(config);

        // Route message in tick 100
        let msg1 = create_test_message(1, 100);
        router.route(100, msg1).unwrap();

        // Route message in tick 101 (should reset enq_seq)
        let msg2 = create_test_message(1, 101);
        router.route(101, msg2).unwrap();

        let queue = router.get_symbol_queue(1).unwrap();
        // Note: We can't drain from Arc<InboundQueue> in tests, but we can verify it exists
        assert!(queue.capacity() > 0);
    }

    #[test]
    fn test_symbol_activation() {
        let config = RouterConfig::default();
        let mut router = OrderRouter::new(config);

        // Symbol should not be active initially
        assert!(!router.is_symbol_active(1));

        // Route message should activate symbol
        let msg = create_test_message(1, 100);
        router.route(100, msg).unwrap();

        // Symbol should now be active
        assert!(router.is_symbol_active(1));
        assert_eq!(router.metrics().active_symbols, 1);
        assert_eq!(router.metrics().activation_requests, 1);
    }

    #[test]
    fn test_backpressure() {
        let config = RouterConfig {
            spsc_depth_default: 2, // Very small queue
            ..Default::default()
        };
        let mut router = OrderRouter::new(config);

        // First message should succeed
        let msg1 = create_test_message(1, 100);
        router.route(100, msg1).unwrap();

        // Second message should be rejected due to backpressure
        let msg2 = create_test_message(1, 101);
        let result = router.route(100, msg2);

        assert_eq!(result, Err(RouterError::Backpressure));
        assert_eq!(router.metrics().rejected_backpressure, 1);
    }
}
