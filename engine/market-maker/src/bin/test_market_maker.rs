use market_maker::{MarketMakerConfig, MarketMakerService};
use tracing::info;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Initialize tracing
    tracing_subscriber::fmt::init();
    
    info!("Testing Market Maker service");
    
    // Load configuration
    let config = MarketMakerConfig::from_env()?;
    info!("Loaded configuration: {:?}", config);
    
    // Create market maker service
    let market_maker = MarketMakerService::new(config).await?;
    
    // Test cache stats
    let cache_stats = market_maker.cache_stats().await;
    info!("Cache stats: {:?}", cache_stats);
    
    // Test metrics
    let metrics = market_maker.metrics();
    info!("Metrics: {:?}", metrics);
    
    // Run one market making cycle
    info!("Running one market making cycle...");
    // TODO: Add method to run single cycle for testing
    
    info!("Test completed successfully");
    Ok(())
}
