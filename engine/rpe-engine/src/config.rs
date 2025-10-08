use serde::{Deserialize, Serialize};

/// Configuration for the Reference Price Engine (RPE)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RpeConfig {
    /// Database configuration
    pub database: DatabaseConfig,
    
    /// RPE calculation parameters
    pub rpe: RpeParameters,
    
    /// Event emission configuration
    pub events: EventConfig,
    
    /// Fair Price 2.0 configuration (new adaptive system)
    pub fair2: Option<Fair2Config>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DatabaseConfig {
    /// Database URL
    pub url: String,
    
    /// Connection pool size
    pub max_connections: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RpeParameters {
    /// Base price in cents (e.g., 5000 = $50.00)
    pub base_cents: i64,
    
    /// Season sensitivity in cents per point (e.g., 200 = $2.00 per point)
    pub beta_season_cents_per_pt: i64,
    
    /// Week sensitivity in cents per point (e.g., 800 = $8.00 per point)
    pub beta_week_cents_per_pt: i64,
    
    /// Position-specific kappa values (cents per point)
    pub kappa_cents_per_pt: std::collections::HashMap<String, i64>,
    
    /// Default kappa value for positions not specified
    pub kappa_default_cents_per_pt: i64,
    
    /// In-game band in basis points (e.g., 3000 = ±30%)
    pub ingame_band_bps: i64,
    
    /// Pacing mode: 'step' or 'poll-step'
    pub pacing_mode: String,
    
    /// Number of pacing steps per game (for poll-step mode)
    pub pacing_steps: u32,
    
    /// Current season (e.g., 2025)
    pub season: i32,
    
    /// Current week
    pub week: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventConfig {
    /// Enable event emission
    pub enabled: bool,
    
    /// Minimum change in cents to emit an event
    pub min_change_cents: i64,
    
    /// Event emission interval in milliseconds
    pub emission_interval_ms: u64,
}

impl Default for RpeConfig {
    fn default() -> Self {
        let mut kappa_map = std::collections::HashMap::new();
        kappa_map.insert("QB".to_string(), 100); // $1.00 per point
        kappa_map.insert("WR".to_string(), 150); // $1.50 per point
        kappa_map.insert("RB".to_string(), 150); // $1.50 per point
        kappa_map.insert("TE".to_string(), 150); // $1.50 per point
        
        Self {
            database: DatabaseConfig {
                url: "postgresql://postgres:password@localhost:5432/waiver_exchange".to_string(),
                max_connections: 10,
            },
            rpe: RpeParameters {
                base_cents: 5000, // $50.00
                beta_season_cents_per_pt: 300, // $3.00 per point (increased emphasis on season projection)
                beta_week_cents_per_pt: 0, // $0.00 per point (removed - was causing double counting)
                kappa_cents_per_pt: kappa_map,
                kappa_default_cents_per_pt: 150, // $1.50 per point
                ingame_band_bps: 3000, // ±30%
                pacing_mode: "step".to_string(),
                pacing_steps: 6,
                season: 2025,
                week: 4,
            },
            events: EventConfig {
                enabled: true,
                min_change_cents: 1, // Emit on any change
                emission_interval_ms: 1000, // 1 second
            },
        }
    }
}

impl RpeConfig {
    /// Load configuration from environment variables
    pub fn from_env() -> anyhow::Result<Self> {
        let mut config = Self::default();
        
        if let Ok(db_url) = std::env::var("DATABASE_URL") {
            config.database.url = db_url;
        }
        
        if let Ok(season) = std::env::var("RPE_SEASON") {
            config.rpe.season = season.parse().unwrap_or(2025);
        }
        
        if let Ok(week) = std::env::var("RPE_WEEK") {
            config.rpe.week = week.parse().unwrap_or(4);
        }
        
        if let Ok(base_cents) = std::env::var("RPE_BASE_CENTS") {
            config.rpe.base_cents = base_cents.parse().unwrap_or(5000);
        }
        
        if let Ok(beta_season) = std::env::var("RPE_BETA_SEASON") {
            config.rpe.beta_season_cents_per_pt = beta_season.parse().unwrap_or(200);
        }
        
        if let Ok(beta_week) = std::env::var("RPE_BETA_WEEK") {
            config.rpe.beta_week_cents_per_pt = beta_week.parse().unwrap_or(800);
        }
        
        if let Ok(band_bps) = std::env::var("RPE_BAND_BPS") {
            config.rpe.ingame_band_bps = band_bps.parse().unwrap_or(3000);
        }
        
        Ok(config)
    }
    
    /// Get kappa value for a position
    pub fn get_kappa_for_position(&self, position: &str) -> i64 {
        self.rpe.kappa_cents_per_pt
            .get(position)
            .copied()
            .unwrap_or(self.rpe.kappa_default_cents_per_pt)
    }
}
