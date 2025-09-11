//! # AnalyticsEngine
//! 
//! Comprehensive analytics and observability system for the Waiver Exchange.
//! 
//! The AnalyticsEngine provides real-time metrics collection, historical data storage,
//! and analytics capabilities for monitoring system performance, business metrics,
//! and operational health.

pub mod config;
pub mod storage;
pub mod aggregation;
pub mod ingestion;
pub mod query;
pub mod cli;

// Re-export main types for easy usage
pub use config::AnalyticsConfig;
pub use storage::ParquetStorage;
pub use aggregation::MetricsAggregator;
pub use ingestion::AnalyticsIngestion;
pub use query::QueryEngine;

// Analytics event definitions
pub mod analytics;

/// Initialize the AnalyticsEngine with the given configuration
pub async fn init_analytics(config: AnalyticsConfig) -> Result<AnalyticsEngine, anyhow::Error> {
    tracing::info!("Initializing AnalyticsEngine with config: {:?}", config);
    
    let storage = ParquetStorage::new(&config.storage.base_path).await?;
    let aggregator = MetricsAggregator::new(config.aggregation.clone());
    let ingestion = AnalyticsIngestion::new(config.ingestion.clone()).await?;
    let query_engine = QueryEngine::new(&config.storage.base_path).await?;
    
    Ok(AnalyticsEngine {
        storage,
        aggregator,
        ingestion,
        query_engine,
        config,
    })
}

/// Main AnalyticsEngine service
#[derive(Debug)]
pub struct AnalyticsEngine {
    pub storage: ParquetStorage,
    pub aggregator: MetricsAggregator,
    pub ingestion: AnalyticsIngestion,
    pub query_engine: QueryEngine,
    pub config: AnalyticsConfig,
}

impl AnalyticsEngine {
    /// Start the analytics engine
    pub async fn start(self) -> Result<(), anyhow::Error> {
        tracing::info!("Starting AnalyticsEngine");
        
        // Start ingestion service
        let ingestion_handle = tokio::spawn(async move {
            self.ingestion.start().await
        });
        
        // Start aggregation service
        let aggregator_handle = tokio::spawn(async move {
            self.aggregator.start().await
        });
        
        // Start query service
        let query_handle = tokio::spawn(async move {
            self.query_engine.start().await
        });
        
        // Wait for all services
        tokio::try_join!(ingestion_handle, aggregator_handle, query_handle)?;
        
        Ok(())
    }
    
    /// Get query engine for CLI usage
    pub fn query_engine(&self) -> &QueryEngine {
        &self.query_engine
    }
}
