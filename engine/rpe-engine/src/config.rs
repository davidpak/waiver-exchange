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

/// Fair Price 2.0 Configuration - Adaptive, Performance-Weighted Reference Price
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Fair2Config {
    /// Base price in cents (e.g., 5000 = $50.00)
    pub base_cents: i64,
    
    /// Beta sensitivity in cents per point (e.g., 300 = $3.00 per point)
    pub beta_cents_per_pt: i64,
    
    /// Position-specific kappa values (cents per point)
    pub kappa_cents_per_pt: std::collections::HashMap<String, i64>,
    
    /// Alpha weighting mode: "linear" or "exp"
    pub alpha_mode: String,
    
    /// Exponential decay lambda (only used if alpha_mode="exp")
    pub alpha_exp_lambda: f64,
    
    /// Band in basis points (e.g., 3000 = ±30%)
    pub band_bps: i64,
    
    /// EMA delta configuration
    pub ema_delta: EmaDeltaConfig,
    
    /// Consistency adjustment configuration
    pub consistency: ConsistencyConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmaDeltaConfig {
    /// Conceptual window for EMA (for documentation)
    pub window: u8,
    
    /// EMA smoothing factor (0 < γ ≤ 1)
    pub smoothing: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConsistencyConfig {
    /// Enable consistency-based kappa adjustment
    pub enabled: bool,
    
    /// Scale factor for volatility adjustment
    pub scale: f64,
    
    /// Minimum weeks required for consistency calculation
    pub min_weeks_for_sigma: u8,
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
        
        // Fair Price 2.0 default configuration
        let mut fair2_kappa_map = std::collections::HashMap::new();
        fair2_kappa_map.insert("QB".to_string(), 100); // $1.00 per point
        fair2_kappa_map.insert("WR".to_string(), 150); // $1.50 per point
        fair2_kappa_map.insert("RB".to_string(), 150); // $1.50 per point
        fair2_kappa_map.insert("TE".to_string(), 150); // $1.50 per point
        
        let fair2_config = Fair2Config {
            base_cents: 5000, // $50.00
            beta_cents_per_pt: 300, // $3.00 per point
            kappa_cents_per_pt: fair2_kappa_map,
            alpha_mode: "linear".to_string(), // Start with linear for simplicity
            alpha_exp_lambda: 0.12, // Exponential decay rate
            band_bps: 3000, // ±30%
            ema_delta: EmaDeltaConfig {
                window: 3, // Conceptual window
                smoothing: 0.3, // EMA smoothing factor
            },
            consistency: ConsistencyConfig {
                enabled: true, // Enable consistency adjustment
                scale: 10.0, // Volatility scale factor
                min_weeks_for_sigma: 4, // Minimum weeks for consistency calculation
            },
        };
        
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
            fair2: Some(fair2_config), // Enable Fair Price 2.0 by default
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
    
    /// Check if Fair Price 2.0 is enabled
    pub fn is_fair2_enabled(&self) -> bool {
        self.fair2.is_some()
    }
    
    /// Get Fair Price 2.0 configuration
    pub fn get_fair2_config(&self) -> Option<&Fair2Config> {
        self.fair2.as_ref()
    }
}

impl Fair2Config {
    /// Get kappa value for a position in Fair Price 2.0
    pub fn get_kappa_for_position(&self, position: &str) -> i64 {
        self.kappa_cents_per_pt
            .get(position)
            .copied()
            .unwrap_or(150) // Default to $1.50 per point
    }
    
    /// Calculate alpha weight based on weeks played
    pub fn calculate_alpha(&self, weeks_played: u8) -> f64 {
        match self.alpha_mode.as_str() {
            "exp" => (-self.alpha_exp_lambda * weeks_played as f64).exp(),
            "linear" => (1.0 - (weeks_played as f64 / 17.0)).max(0.0),
            _ => (1.0 - (weeks_played as f64 / 17.0)).max(0.0), // Default to linear
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fair2_config_default() {
        let config = RpeConfig::default();
        assert!(config.is_fair2_enabled());
        
        let fair2 = config.get_fair2_config().unwrap();
        assert_eq!(fair2.base_cents, 5000);
        assert_eq!(fair2.beta_cents_per_pt, 300);
        assert_eq!(fair2.alpha_mode, "linear");
        assert_eq!(fair2.band_bps, 3000);
    }

    #[test]
    fn test_alpha_calculation_linear() {
        let config = RpeConfig::default();
        let fair2 = config.get_fair2_config().unwrap();
        
        // Test linear alpha calculation
        assert_eq!(fair2.calculate_alpha(0), 1.0); // Week 0: 100% projection
        assert_eq!(fair2.calculate_alpha(8), 0.5294117647058824); // Week 8: ~53% projection
        assert_eq!(fair2.calculate_alpha(17), 0.0); // Week 17: 0% projection
        assert_eq!(fair2.calculate_alpha(20), 0.0); // Week 20: 0% projection (clamped)
    }

    #[test]
    fn test_alpha_calculation_exponential() {
        let config = RpeConfig::default();
        let fair2 = config.get_fair2_config().unwrap();
        
        // Create a config with exponential alpha
        let mut exp_config = fair2.clone();
        exp_config.alpha_mode = "exp".to_string();
        exp_config.alpha_exp_lambda = 0.12;
        
        // Test exponential alpha calculation
        assert_eq!(exp_config.calculate_alpha(0), 1.0); // Week 0: 100% projection
        assert!(exp_config.calculate_alpha(8) < 0.4); // Week 8: <40% projection
        assert!(exp_config.calculate_alpha(17) > 0.0); // Week 17: >0% projection (never reaches 0)
    }

    #[test]
    fn test_kappa_for_position() {
        let config = RpeConfig::default();
        let fair2 = config.get_fair2_config().unwrap();
        
        assert_eq!(fair2.get_kappa_for_position("QB"), 100);
        assert_eq!(fair2.get_kappa_for_position("RB"), 150);
        assert_eq!(fair2.get_kappa_for_position("WR"), 150);
        assert_eq!(fair2.get_kappa_for_position("TE"), 150);
        assert_eq!(fair2.get_kappa_for_position("UNKNOWN"), 150); // Default
    }
}