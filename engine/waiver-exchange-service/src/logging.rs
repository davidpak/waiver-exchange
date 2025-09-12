//! Logging and tracing setup

use anyhow::Result;
use tracing_subscriber::{
    fmt, layer::SubscriberExt, prelude::*, util::SubscriberInitExt, EnvFilter,
};

/// Initialize logging and tracing
pub fn initialize_logging() -> Result<()> {
    // Set up environment filter
    let env_filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));

    // Set up formatting layer
    let fmt_layer = fmt::layer()
        .with_target(false)
        .with_thread_ids(true)
        .with_thread_names(true)
        .with_file(true)
        .with_line_number(true)
        .with_ansi(true);

    // Initialize the subscriber
    tracing_subscriber::registry().with(env_filter).with(fmt_layer).init();

    Ok(())
}

/// Initialize logging with custom configuration
pub fn initialize_logging_with_config(
    level: &str,
    format: &str,
    _file: Option<&std::path::Path>,
) -> Result<()> {
    // Set up environment filter
    let env_filter = EnvFilter::new(level);

    // Set up formatting layer based on format
    let fmt_layer = match format {
        "json" => fmt::layer()
            .json()
            .with_target(true)
            .with_thread_ids(true)
            .with_thread_names(true)
            .with_file(true)
            .with_line_number(true)
            .boxed(),
        "pretty" => fmt::layer()
            .pretty()
            .with_target(false)
            .with_thread_ids(true)
            .with_thread_names(true)
            .with_file(true)
            .with_line_number(true)
            .with_ansi(true)
            .boxed(),
        _ => fmt::layer()
            .with_target(false)
            .with_thread_ids(true)
            .with_thread_names(true)
            .with_file(true)
            .with_line_number(true)
            .with_ansi(true)
            .boxed(),
    };

    // Initialize the subscriber
    tracing_subscriber::registry().with(env_filter).with(fmt_layer).init();

    Ok(())
}
