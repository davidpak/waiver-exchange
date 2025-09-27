//! Service configuration management

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

use execution_manager::ExecManagerConfig;
use order_router::RouterConfig;
use persistence::PersistenceConfig;
use simulation_clock::ClockConfig;
use symbol_coordinator::CoordinatorConfig;
use equity_service::EquityServiceConfig;

/// Main service configuration
#[derive(Debug, Clone, Default)]
pub struct ServiceConfig {
    /// SimulationClock configuration
    pub clock: ClockConfig,

    /// SymbolCoordinator configuration
    pub symbol_coordinator: CoordinatorConfig,

    /// ExecutionManager configuration
    pub execution_manager: ExecManagerConfig,

    /// OrderRouter configuration
    pub order_router: RouterConfig,

    /// Persistence configuration
    pub persistence: PersistenceConfig,

    /// Equity Service configuration
    pub equity_service: EquityServiceConfig,

    /// Service-level configuration
    pub service: ServiceSettings,

    /// Logging configuration
    pub logging: LoggingConfig,

    /// Metrics configuration
    pub metrics: MetricsConfig,
}

/// Service-level settings
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServiceSettings {
    /// Data directory for the service
    pub data_dir: PathBuf,

    /// Configuration file path
    pub config_file: Option<PathBuf>,

    /// Enable development mode (more verbose logging, etc.)
    pub development_mode: bool,

    /// Maximum number of symbols to support
    pub max_symbols: u32,

    /// Service startup timeout in seconds
    pub startup_timeout_secs: u64,

    /// Graceful shutdown timeout in seconds
    pub shutdown_timeout_secs: u64,
}

/// Logging configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoggingConfig {
    /// Log level (trace, debug, info, warn, error)
    pub level: String,

    /// Log format (json, pretty)
    pub format: String,

    /// Log file path (if None, logs to stdout)
    pub file: Option<PathBuf>,

    /// Enable structured logging
    pub structured: bool,

    /// Enable log rotation
    pub rotation: bool,

    /// Maximum log file size in MB
    pub max_file_size_mb: u64,
}

/// Metrics configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetricsConfig {
    /// Enable metrics collection
    pub enabled: bool,

    /// Metrics export port
    pub port: u16,

    /// Metrics export path
    pub path: String,

    /// Enable Prometheus metrics
    pub prometheus: bool,

    /// Metrics collection interval in milliseconds
    pub interval_ms: u64,
}

impl Default for ServiceSettings {
    fn default() -> Self {
        Self {
            data_dir: PathBuf::from("./data"),
            config_file: None,
            development_mode: false,
            max_symbols: 1000,
            startup_timeout_secs: 30,
            shutdown_timeout_secs: 10,
        }
    }
}

impl Default for LoggingConfig {
    fn default() -> Self {
        Self {
            level: "info".to_string(),
            format: "pretty".to_string(),
            file: None,
            structured: true,
            rotation: true,
            max_file_size_mb: 100,
        }
    }
}

impl Default for MetricsConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            port: 9090,
            path: "/metrics".to_string(),
            prometheus: true,
            interval_ms: 1000,
        }
    }
}

/// Load configuration from files and environment variables
pub fn load_config() -> Result<ServiceConfig> {
    let mut config = ServiceConfig::default();

    // Load from config file if specified
    if let Some(config_file) = &config.service.config_file {
        if config_file.exists() {
            tracing::debug!("Loading configuration from file: {:?}", config_file);
            config = load_from_file(config_file)?;
        }
    }

    // Override with environment variables
    load_from_env(&mut config)?;

    // Validate configuration
    validate_config(&config)?;

    Ok(config)
}

/// Load configuration from a TOML file
fn load_from_file(_path: &std::path::Path) -> Result<ServiceConfig> {
    // For now, just return default config
    // TODO: Implement proper config file loading
    Ok(ServiceConfig::default())
}

/// Load configuration from environment variables
fn load_from_env(config: &mut ServiceConfig) -> Result<()> {
    // Clock configuration
    if let Ok(level) = std::env::var("WAIVER_LOG_LEVEL") {
        config.logging.level = level;
    }

    if let Ok(format) = std::env::var("WAIVER_LOG_FORMAT") {
        config.logging.format = format;
    }

    if let Ok(dev_mode) = std::env::var("WAIVER_DEV_MODE") {
        config.service.development_mode = dev_mode.parse().unwrap_or(false);
    }

    if let Ok(data_dir) = std::env::var("WAIVER_DATA_DIR") {
        config.service.data_dir = PathBuf::from(data_dir);
    }

    if let Ok(max_symbols) = std::env::var("WAIVER_MAX_SYMBOLS") {
        config.service.max_symbols = max_symbols.parse().unwrap_or(1000);
    }

    Ok(())
}

/// Validate configuration
fn validate_config(config: &ServiceConfig) -> Result<()> {
    // Validate data directory
    if !config.service.data_dir.exists() {
        std::fs::create_dir_all(&config.service.data_dir).with_context(|| {
            format!("Failed to create data directory: {:?}", config.service.data_dir)
        })?;
    }

    // Validate log level
    match config.logging.level.as_str() {
        "trace" | "debug" | "info" | "warn" | "error" => {}
        _ => return Err(anyhow::anyhow!("Invalid log level: {}", config.logging.level)),
    }

    // Validate log format
    match config.logging.format.as_str() {
        "json" | "pretty" => {}
        _ => return Err(anyhow::anyhow!("Invalid log format: {}", config.logging.format)),
    }

    // Validate metrics port
    if config.metrics.port == 0 {
        return Err(anyhow::anyhow!("Invalid metrics port: {}", config.metrics.port));
    }

    Ok(())
}

/// Save configuration to a file
pub fn save_config(_config: &ServiceConfig, _path: &std::path::Path) -> Result<()> {
    // For now, just return Ok
    // TODO: Implement proper config file saving
    Ok(())
}
