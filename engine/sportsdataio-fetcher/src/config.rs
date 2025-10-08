use serde::{Deserialize, Serialize};

/// Configuration for the SportsDataIO Fetcher service
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FetcherConfig {
    /// SportsDataIO API configuration
    pub sportsdataio: SportsDataIOConfig,
    
    /// Database configuration
    pub database: DatabaseConfig,
    
    /// Scheduler configuration
    pub scheduler: SchedulerConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SportsDataIOConfig {
    /// Projections API key environment variable name
    pub projections_api_key_env: String,
    
    /// Live stats API key environment variable name
    pub live_stats_api_key_env: String,
    
    /// Current season (e.g., "2025REG")
    pub season: String,
    
    /// Current week
    pub week: u32,
    
    /// Polling interval in minutes for live stats
    pub poll_minutes: u32,
    
    /// Live game windows (UTC)
    pub live_windows_utc: Vec<GameWindow>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GameWindow {
    /// Day of week
    pub day: String,
    
    /// Start time (HH:MM format)
    pub start: String,
    
    /// End time (HH:MM format, can include +1 for next day)
    pub end: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DatabaseConfig {
    /// Database URL
    pub url: String,
    
    /// Connection pool size
    pub max_connections: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SchedulerConfig {
    /// Enable nightly projections fetch
    pub enable_projections: bool,
    
    /// Enable live stats polling
    pub enable_live_stats: bool,
    
    /// Nightly fetch time (HH:MM format, UTC)
    pub nightly_fetch_time: String,
    
    /// Retry configuration
    pub retry: RetryConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RetryConfig {
    /// Maximum number of retries
    pub max_retries: u32,
    
    /// Initial retry delay in seconds
    pub initial_delay_secs: u64,
    
    /// Maximum retry delay in seconds
    pub max_delay_secs: u64,
    
    /// Backoff multiplier
    pub backoff_multiplier: f64,
}

impl Default for FetcherConfig {
    fn default() -> Self {
        Self {
            sportsdataio: SportsDataIOConfig {
                projections_api_key_env: "SPORTS_DATA_IO_PROJECTIONS_KEY".to_string(),
                live_stats_api_key_env: "SPORTS_DATA_IO_LIVE_STATS_KEY".to_string(),
                season: "2025REG".to_string(),
                week: 4,
                poll_minutes: 10,
                live_windows_utc: vec![
                    GameWindow {
                        day: "THU".to_string(),
                        start: "00:00".to_string(),
                        end: "08:00".to_string(),
                    },
                    GameWindow {
                        day: "SUN".to_string(),
                        start: "17:00".to_string(),
                        end: "02:00+1".to_string(),
                    },
                    GameWindow {
                        day: "MON".to_string(),
                        start: "00:00".to_string(),
                        end: "06:00".to_string(),
                    },
                ],
            },
            database: DatabaseConfig {
                url: "postgresql://postgres:password@localhost:5432/waiver_exchange".to_string(),
                max_connections: 10,
            },
            scheduler: SchedulerConfig {
                enable_projections: true,
                enable_live_stats: true,
                nightly_fetch_time: "06:00".to_string(),
                retry: RetryConfig {
                    max_retries: 3,
                    initial_delay_secs: 5,
                    max_delay_secs: 300,
                    backoff_multiplier: 2.0,
                },
            },
        }
    }
}

impl FetcherConfig {
    /// Load configuration from environment variables
    pub fn from_env() -> anyhow::Result<Self> {
        let mut config = Self::default();
        
        // Override with environment variables if present
        if let Ok(api_key) = std::env::var(&config.sportsdataio.projections_api_key_env) {
            // Store API key for later use
            std::env::set_var("SPORTS_DATA_IO_PROJECTIONS_API_KEY", api_key);
        }
        
        if let Ok(api_key) = std::env::var(&config.sportsdataio.live_stats_api_key_env) {
            // Store API key for later use
            std::env::set_var("SPORTS_DATA_IO_LIVE_STATS_API_KEY", api_key);
        }
        
        if let Ok(db_url) = std::env::var("DATABASE_URL") {
            config.database.url = db_url;
        }
        
        if let Ok(season) = std::env::var("SPORTSDATAIO_SEASON") {
            config.sportsdataio.season = season;
        }
        
        if let Ok(week) = std::env::var("SPORTSDATAIO_WEEK") {
            config.sportsdataio.week = week.parse().unwrap_or(4);
        }
        
        Ok(config)
    }
    
    /// Get the projections API key from environment
    pub fn get_projections_api_key(&self) -> anyhow::Result<String> {
        std::env::var(&self.sportsdataio.projections_api_key_env)
            .or_else(|_| std::env::var("SPORTS_DATA_IO_PROJECTIONS_API_KEY"))
            .map_err(|_| anyhow::anyhow!("SportsDataIO projections API key not found in environment"))
    }
    
    /// Get the live stats API key from environment
    pub fn get_live_stats_api_key(&self) -> anyhow::Result<String> {
        std::env::var(&self.sportsdataio.live_stats_api_key_env)
            .or_else(|_| std::env::var("SPORTS_DATA_IO_LIVE_STATS_API_KEY"))
            .map_err(|_| anyhow::anyhow!("SportsDataIO live stats API key not found in environment"))
    }
    
    /// Check if we're currently in a live game window
    pub fn is_live_window(&self) -> bool {
        let now = chrono::Utc::now();
        let current_day = now.format("%a").to_string();
        let current_time = now.format("%H:%M").to_string();
        
        for window in &self.sportsdataio.live_windows_utc {
            if window.day == current_day {
                // Parse start and end times
                if let (Ok(start), Ok(end)) = (
                    chrono::NaiveTime::parse_from_str(&window.start, "%H:%M"),
                    chrono::NaiveTime::parse_from_str(&window.end.replace("+1", ""), "%H:%M")
                ) {
                    let current_naive = chrono::NaiveTime::parse_from_str(&current_time, "%H:%M").unwrap_or_default();
                    
                    // Handle next day end times
                    if window.end.ends_with("+1") {
                        // Window spans midnight
                        return current_naive >= start || current_naive <= end;
                    } else {
                        // Normal window
                        return current_naive >= start && current_naive <= end;
                    }
                }
            }
        }
        
        false
    }
}
