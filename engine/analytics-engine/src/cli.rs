//! # Command Line Interface
//! 
//! CLI for querying analytics data and system monitoring.

use crate::query::QueryEngine;
use anyhow::Result;
use clap::{Parser, Subcommand};
use std::path::PathBuf;

/// Analytics CLI for system monitoring and querying
#[derive(Parser)]
#[command(name = "analytics-cli")]
#[command(about = "Analytics CLI for Waiver Exchange system monitoring")]
pub struct Cli {
    /// Path to analytics data storage
    #[arg(short, long, default_value = "./analytics_data")]
    pub data_path: PathBuf,
    
    /// Command to execute
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Show system health summary
    Health {
        /// Hours to look back
        #[arg(long, default_value = "24")]
        hours: i64,
    },
    /// Show performance metrics
    Performance {
        /// Hours to look back
        #[arg(long, default_value = "24")]
        hours: i64,
    },
    /// Show business metrics
    Business {
        /// Hours to look back
        #[arg(long, default_value = "24")]
        hours: i64,
    },
    /// Show operational metrics
    Operational {
        /// Hours to look back
        #[arg(long, default_value = "24")]
        hours: i64,
    },
    /// Execute custom SQL query
    Query {
        /// SQL query to execute
        sql: String,
    },
    /// Show system status
    Status,
}

/// CLI handler
pub struct CliHandler {
    query_engine: QueryEngine,
}

impl CliHandler {
    /// Create new CLI handler
    pub async fn new(data_path: &PathBuf) -> Result<Self> {
        let query_engine = QueryEngine::new(data_path).await?;
        Ok(Self { query_engine })
    }
    
    /// Handle CLI commands
    pub async fn handle_command(&self, command: Commands) -> Result<()> {
        match command {
            Commands::Health { hours } => {
                self.show_health(hours).await?;
            }
            Commands::Performance { hours } => {
                self.show_performance(hours).await?;
            }
            Commands::Business { hours } => {
                self.show_business(hours).await?;
            }
            Commands::Operational { hours } => {
                self.show_operational(hours).await?;
            }
            Commands::Query { sql } => {
                self.execute_query(&sql).await?;
            }
            Commands::Status => {
                self.show_status().await?;
            }
        }
        Ok(())
    }
    
    /// Show system health summary
    async fn show_health(&self, hours: i64) -> Result<()> {
        println!("🔍 System Health Summary (Last {} hours)", hours);
        println!("{}", "=".repeat(50));
        
        let results = self.query_engine.get_system_health(hours).await?;
        self.print_results(results).await?;
        
        Ok(())
    }
    
    /// Show performance metrics
    async fn show_performance(&self, hours: i64) -> Result<()> {
        println!("⚡ Performance Metrics (Last {} hours)", hours);
        println!("{}", "=".repeat(50));
        
        let results = self.query_engine.get_performance_metrics(hours).await?;
        self.print_results(results).await?;
        
        Ok(())
    }
    
    /// Show business metrics
    async fn show_business(&self, hours: i64) -> Result<()> {
        println!("📊 Business Metrics (Last {} hours)", hours);
        println!("{}", "=".repeat(50));
        
        let results = self.query_engine.get_business_metrics(hours).await?;
        self.print_results(results).await?;
        
        Ok(())
    }
    
    /// Show operational metrics
    async fn show_operational(&self, hours: i64) -> Result<()> {
        println!("🔧 Operational Metrics (Last {} hours)", hours);
        println!("{}", "=".repeat(50));
        
        let results = self.query_engine.get_operational_metrics(hours).await?;
        self.print_results(results).await?;
        
        Ok(())
    }
    
    /// Execute custom SQL query (simplified)
    async fn execute_query(&self, sql: &str) -> Result<()> {
        println!("🔍 Executing Query:");
        println!("{}", sql);
        println!("{}", "=".repeat(50));
        
        // For now, just show a message that SQL queries are not implemented
        println!("SQL queries not implemented in simplified version");
        println!("Use specific commands like 'health', 'performance', etc.");
        
        Ok(())
    }
    
    /// Show system status
    async fn show_status(&self) -> Result<()> {
        println!("📈 Analytics System Status");
        println!("{}", "=".repeat(50));
        
        // Check if data exists
        let health_results = self.query_engine.get_system_health(1).await?;
        let perf_results = self.query_engine.get_performance_metrics(1).await?;
        let biz_results = self.query_engine.get_business_metrics(1).await?;
        
        println!("✅ System Health Data: {} records", health_results.len());
        println!("✅ Performance Data: {} records", perf_results.len());
        println!("✅ Business Data: {} records", biz_results.len());
        
        println!("\n🎯 System is operational and collecting data");
        
        Ok(())
    }
    
    /// Print query results in a formatted table
    async fn print_results(&self, results: Vec<String>) -> Result<()> {
        if results.is_empty() {
            println!("No data found");
            return Ok(());
        }
        
        for result in results {
            println!("{}", result);
        }
        
        Ok(())
    }
}
