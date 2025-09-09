// Shutdown management for ExecutionManager

use crate::config::ShutdownConfig;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

/// Shutdown manager for graceful ExecutionManager shutdown
pub struct ShutdownManager {
    config: ShutdownConfig,
    shutdown_requested: AtomicBool,
    shutdown_started: AtomicBool,
    shutdown_completed: AtomicBool,
}

impl ShutdownManager {
    pub fn new(config: ShutdownConfig) -> Self {
        Self {
            config,
            shutdown_requested: AtomicBool::new(false),
            shutdown_started: AtomicBool::new(false),
            shutdown_completed: AtomicBool::new(false),
        }
    }

    pub fn initiate_shutdown(&self) {
        self.shutdown_requested.store(true, Ordering::Relaxed);
    }

    pub fn is_shutdown_requested(&self) -> bool {
        self.shutdown_requested.load(Ordering::Relaxed)
    }

    pub fn start_shutdown(&self) {
        self.shutdown_started.store(true, Ordering::Relaxed);
    }

    pub fn is_shutdown_started(&self) -> bool {
        self.shutdown_started.load(Ordering::Relaxed)
    }

    pub fn complete_shutdown(&self) {
        self.shutdown_completed.store(true, Ordering::Relaxed);
    }

    pub fn is_shutdown_completed(&self) -> bool {
        self.shutdown_completed.load(Ordering::Relaxed)
    }

    pub fn get_shutdown_timeout(&self) -> Duration {
        self.config.shutdown_timeout
    }

    pub fn should_flush_on_shutdown(&self) -> bool {
        self.config.flush_on_shutdown
    }

    pub fn should_wait_for_downstream(&self) -> bool {
        self.config.wait_for_downstream
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_shutdown_lifecycle() {
        let manager = ShutdownManager::new(ShutdownConfig::default());

        // Initially not shutdown
        assert!(!manager.is_shutdown_requested());
        assert!(!manager.is_shutdown_started());
        assert!(!manager.is_shutdown_completed());

        // Initiate shutdown
        manager.initiate_shutdown();
        assert!(manager.is_shutdown_requested());
        assert!(!manager.is_shutdown_started());
        assert!(!manager.is_shutdown_completed());

        // Start shutdown
        manager.start_shutdown();
        assert!(manager.is_shutdown_requested());
        assert!(manager.is_shutdown_started());
        assert!(!manager.is_shutdown_completed());

        // Complete shutdown
        manager.complete_shutdown();
        assert!(manager.is_shutdown_requested());
        assert!(manager.is_shutdown_started());
        assert!(manager.is_shutdown_completed());
    }

    #[test]
    fn test_shutdown_config() {
        let config = ShutdownConfig {
            shutdown_timeout: Duration::from_secs(10),
            flush_on_shutdown: true,
            wait_for_downstream: false,
        };

        let manager = ShutdownManager::new(config);

        assert_eq!(manager.get_shutdown_timeout(), Duration::from_secs(10));
        assert!(manager.should_flush_on_shutdown());
        assert!(!manager.should_wait_for_downstream());
    }
}
