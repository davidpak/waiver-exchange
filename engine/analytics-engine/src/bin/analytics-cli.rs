//! # Analytics CLI Binary
//!
//! Command-line interface for querying analytics data.

use analytics_engine::cli::{Cli, CliHandler};
use anyhow::Result;
use clap::Parser;

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize logging
    tracing_subscriber::fmt::init();

    // Parse CLI arguments
    let cli = Cli::parse();

    // Create CLI handler
    let handler = CliHandler::new(&cli.data_path).await?;

    // Handle command
    handler.handle_command(cli.command).await?;

    Ok(())
}
