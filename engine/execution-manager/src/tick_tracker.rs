// Tick tracking for ExecutionManager

use crate::config::TickTrackingConfig;
use crate::event::DispatchEvent;
use dashmap::{DashMap, DashSet};
use std::sync::atomic::{AtomicU64, Ordering};
use whistle::TickId;

/// Tick tracker for coordinating multi-symbol tick completion
pub struct TickTracker {
    #[allow(dead_code)]
    config: TickTrackingConfig,
    registered_symbols: DashSet<u32>,
    symbol_tick_progress: DashMap<u32, TickId>,
    current_tick: AtomicU64, // Using AtomicU64 with Option encoding
}

impl TickTracker {
    pub fn new(config: TickTrackingConfig) -> Self {
        Self {
            config,
            registered_symbols: DashSet::new(),
            symbol_tick_progress: DashMap::new(),
            current_tick: AtomicU64::new(0), // 0 means None
        }
    }

    pub fn register_symbol(&self, symbol_id: u32) {
        self.registered_symbols.insert(symbol_id);
    }

    pub fn deregister_symbol(&self, symbol_id: u32) {
        self.registered_symbols.remove(&symbol_id);
        self.symbol_tick_progress.remove(&symbol_id);
    }

    pub fn process_event(&self, event: &DispatchEvent) -> Result<(), String> {
        if let Some(tick) = event.logical_timestamp() {
            if let Some(symbol) = event.symbol() {
                self.symbol_tick_progress.insert(symbol, tick);

                // Update current tick if this is newer (lock-free)
                let current = self.current_tick.load(Ordering::Relaxed);
                if current == 0 || tick > current {
                    self.current_tick.store(tick, Ordering::Relaxed);
                }
            }
        }
        Ok(())
    }

    pub fn is_tick_ready(&self, tick_id: TickId) -> bool {
        // Check if all registered symbols have completed this tick
        for symbol_ref in self.registered_symbols.iter() {
            let symbol = *symbol_ref.key();
            if let Some(completed_tick) = self.symbol_tick_progress.get(&symbol) {
                if *completed_tick < tick_id {
                    return false;
                }
            } else {
                return false; // Symbol hasn't completed any ticks
            }
        }
        true
    }

    pub fn get_current_tick(&self) -> Option<TickId> {
        let current = self.current_tick.load(Ordering::Relaxed);
        if current == 0 {
            None
        } else {
            Some(current)
        }
    }

    pub fn get_stats(&self) -> TickBoundaryStats {
        TickBoundaryStats {
            registered_symbols: self.registered_symbols.len(),
            current_tick: self.get_current_tick(),
            symbols_behind: self.count_symbols_behind(),
        }
    }

    fn count_symbols_behind(&self) -> usize {
        if let Some(current_tick) = self.get_current_tick() {
            self.registered_symbols
                .iter()
                .filter(|symbol_ref| {
                    let symbol = *symbol_ref.key();
                    self.symbol_tick_progress.get(&symbol).is_none_or(|tick| *tick < current_tick)
                })
                .count()
        } else {
            0
        }
    }
}

/// Tick boundary statistics
#[derive(Debug, Clone)]
pub struct TickBoundaryStats {
    pub registered_symbols: usize,
    pub current_tick: Option<TickId>,
    pub symbols_behind: usize,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::TickTrackingConfig;
    use crate::event::{DispatchEvent, ExecutionReport};
    use std::time::Instant;
    use whistle::Side;

    fn create_test_tracker() -> TickTracker {
        TickTracker::new(TickTrackingConfig::default())
    }

    #[test]
    fn test_symbol_registration() {
        let tracker = create_test_tracker();

        tracker.register_symbol(1);
        tracker.register_symbol(2);

        let stats = tracker.get_stats();
        assert_eq!(stats.registered_symbols, 2);

        tracker.deregister_symbol(1);
        let stats = tracker.get_stats();
        assert_eq!(stats.registered_symbols, 1);
    }

    #[test]
    fn test_tick_progress_tracking() {
        let tracker = create_test_tracker();

        tracker.register_symbol(1);
        tracker.register_symbol(2);

        // Create test events
        let event1 = DispatchEvent::ExecutionReport(ExecutionReport {
            execution_id: 1,
            order_id: 1,
            price: 150,
            quantity: 10,
            side: Side::Buy,
            aggressor_flag: true,
            logical_timestamp: 100,
            wall_clock_timestamp: Instant::now(),
            symbol: 1,
        });

        let event2 = DispatchEvent::ExecutionReport(ExecutionReport {
            execution_id: 2,
            order_id: 2,
            price: 151,
            quantity: 5,
            side: Side::Sell,
            aggressor_flag: true,
            logical_timestamp: 100,
            wall_clock_timestamp: Instant::now(),
            symbol: 2,
        });

        // Process events
        tracker.process_event(&event1).unwrap();
        tracker.process_event(&event2).unwrap();

        // Check tick readiness
        assert!(tracker.is_tick_ready(100));
        assert!(!tracker.is_tick_ready(101));

        // Check current tick
        assert_eq!(tracker.get_current_tick(), Some(100));
    }

    #[test]
    fn test_tick_readiness() {
        let tracker = create_test_tracker();

        tracker.register_symbol(1);
        tracker.register_symbol(2);

        // Only symbol 1 has completed tick 100
        let event = DispatchEvent::ExecutionReport(ExecutionReport {
            execution_id: 1,
            order_id: 1,
            price: 150,
            quantity: 10,
            side: Side::Buy,
            aggressor_flag: true,
            logical_timestamp: 100,
            wall_clock_timestamp: Instant::now(),
            symbol: 1,
        });

        tracker.process_event(&event).unwrap();

        // Tick 100 should not be ready (symbol 2 hasn't completed it)
        assert!(!tracker.is_tick_ready(100));

        // Register symbol 2 and complete tick 100
        let event2 = DispatchEvent::ExecutionReport(ExecutionReport {
            execution_id: 2,
            order_id: 2,
            price: 151,
            quantity: 5,
            side: Side::Sell,
            aggressor_flag: true,
            logical_timestamp: 100,
            wall_clock_timestamp: Instant::now(),
            symbol: 2,
        });

        tracker.process_event(&event2).unwrap();

        // Now tick 100 should be ready
        assert!(tracker.is_tick_ready(100));
    }
}
