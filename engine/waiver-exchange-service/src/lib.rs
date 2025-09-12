//! Waiver Exchange Production Service Library
//!
//! This library provides the core functionality for the Waiver Exchange production service,
//! including configuration management, component initialization, and graceful shutdown handling.

use anyhow::{Context, Result};

pub mod config;
pub mod logging;
pub mod service;
pub mod signals;

pub use config::ServiceConfig;
pub use logging::initialize_logging;
pub use service::ServiceState;
pub use signals::{graceful_shutdown, setup_signal_handlers};

/// Load configuration from files and environment variables
pub fn load_configuration() -> Result<ServiceConfig> {
    config::load_config().context("Failed to load service configuration")
}
