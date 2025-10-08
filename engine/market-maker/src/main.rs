use market_maker::{MarketMakerConfig, MarketMakerService};
use tracing::{info, error};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Initialize tracing
    tracing_subscriber::fmt::init();
    
    info!("Starting Market Maker service");
    
    // Load configuration
    let config = MarketMakerConfig::from_env()?;
    info!("Loaded configuration: {:?}", config);
    
    // Create market maker service
    let mut market_maker = MarketMakerService::new(config).await?;
    
    // Start the service
    if let Err(e) = market_maker.start().await {
        error!("Market Maker service failed: {}", e);
        return Err(e);
    }
    
    Ok(())
}
