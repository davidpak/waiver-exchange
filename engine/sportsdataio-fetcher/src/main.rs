use sportsdataio_fetcher::{FetcherConfig, FetcherScheduler};
use tracing::{info, error};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Initialize logging
    tracing_subscriber::fmt::init();
    
    info!("Starting SportsDataIO Fetcher Service");
    
    // Load configuration
    let config = FetcherConfig::from_env()?;
    info!("Loaded configuration: {:?}", config);
    
    // Create and start scheduler
    let mut scheduler = FetcherScheduler::new(config).await?;
    
    // Start the scheduler (runs indefinitely)
    if let Err(e) = scheduler.start().await {
        error!("Scheduler failed: {}", e);
        return Err(e);
    }
    
    Ok(())
}
