//! Sleeper API integration

use crate::config::SleeperConfig;
use crate::AccountServiceError;
use serde::{Deserialize, Serialize};

/// Sleeper API client
#[derive(Debug)]
pub struct SleeperClient {
    config: SleeperConfig,
    client: reqwest::Client,
}

/// League option for user selection
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LeagueOption {
    pub id: String,
    pub name: String,
    pub season: String,
    pub roster_id: String,
}

/// Sleeper user response
#[derive(Debug, Deserialize)]
pub struct SleeperUser {
    pub user_id: String,
    pub username: String,
    pub display_name: String,
}

/// Sleeper league response
#[derive(Debug, Deserialize)]
pub struct SleeperLeague {
    pub league_id: String,
    pub name: String,
    pub season: String,
    pub roster_id: String,
}

/// Sleeper roster response
#[derive(Debug, Deserialize)]
pub struct SleeperRoster {
    pub roster_id: String,
    pub owner_id: String,
    pub league_id: String,
    pub season: String,
}

/// Sleeper matchup response
#[derive(Debug, Deserialize)]
pub struct SleeperMatchup {
    pub matchup_id: String,
    pub roster_id: String,
    pub points: f64,
}

impl SleeperClient {
    /// Create a new Sleeper API client
    pub fn new(config: SleeperConfig) -> Self {
        Self {
            config,
            client: reqwest::Client::new(),
        }
    }
    
    /// Get user ID from username
    pub async fn get_user_id(&self, username: &str) -> Result<String, AccountServiceError> {
        let url = format!("{}/user/{}", self.config.api_base_url, username);
        let response = self.client.get(&url).send().await?;
        
        if !response.status().is_success() {
            return Err(AccountServiceError::SleeperApiError {
                message: format!("Failed to get user: {}", response.status()),
            });
        }
        
        let user: SleeperUser = response.json().await?;
        Ok(user.user_id)
    }
    
    /// Get user's leagues for a season
    pub async fn get_user_leagues(&self, user_id: &str, season: &str) -> Result<Vec<LeagueOption>, AccountServiceError> {
        let url = format!("{}/user/{}/leagues/nfl/{}", self.config.api_base_url, user_id, season);
        let response = self.client.get(&url).send().await?;
        
        if !response.status().is_success() {
            return Err(AccountServiceError::SleeperApiError {
                message: format!("Failed to get leagues: {}", response.status()),
            });
        }
        
        let leagues: Vec<SleeperLeague> = response.json().await?;
        
        // Get rosters for each league to find user's roster
        let mut league_options = Vec::new();
        for league in leagues {
            let rosters_url = format!("{}/league/{}/rosters", self.config.api_base_url, league.league_id);
            let rosters_response = self.client.get(&rosters_url).send().await?;
            
            if rosters_response.status().is_success() {
                let rosters: Vec<SleeperRoster> = rosters_response.json().await?;
                
                // Find user's roster in this league
                if let Some(user_roster) = rosters.iter().find(|r| r.owner_id == user_id) {
                    league_options.push(LeagueOption {
                        id: league.league_id,
                        name: league.name,
                        season: league.season,
                        roster_id: user_roster.roster_id.clone(),
                    });
                }
            }
        }
        
        Ok(league_options)
    }
    
    /// Get season total fantasy points for a roster
    pub async fn get_season_points(&self, league_id: &str, roster_id: &str) -> Result<u32, AccountServiceError> {
        let url = format!("{}/league/{}/rosters", self.config.api_base_url, league_id);
        let response = self.client.get(&url).send().await?;
        
        if !response.status().is_success() {
            return Err(AccountServiceError::SleeperApiError {
                message: format!("Failed to get rosters: {}", response.status()),
            });
        }
        
        let rosters: Vec<SleeperRoster> = response.json().await?;
        
        // Find the specific roster and get its total points
        if let Some(_roster) = rosters.iter().find(|r| r.roster_id == roster_id) {
            // This is a simplified implementation
            // In reality, you'd need to aggregate points from all weeks
            Ok(1000) // Placeholder - would need to implement actual point calculation
        } else {
            Err(AccountServiceError::SleeperApiError {
                message: "Roster not found".to_string(),
            })
        }
    }
    
    /// Get weekly matchup for a specific week
    pub async fn get_weekly_matchup(&self, league_id: &str, week: u32) -> Result<Vec<SleeperMatchup>, AccountServiceError> {
        let url = format!("{}/league/{}/matchups/{}", self.config.api_base_url, league_id, week);
        let response = self.client.get(&url).send().await?;
        
        if !response.status().is_success() {
            return Err(AccountServiceError::SleeperApiError {
                message: format!("Failed to get matchups: {}", response.status()),
            });
        }
        
        let matchups: Vec<SleeperMatchup> = response.json().await?;
        Ok(matchups)
    }
    
    /// Health check for Sleeper API
    pub async fn health_check(&self) -> Result<(), AccountServiceError> {
        let url = format!("{}/state/nfl", self.config.api_base_url);
        let response = self.client.get(&url).send().await?;
        
        if !response.status().is_success() {
            return Err(AccountServiceError::SleeperApiError {
                message: format!("Health check failed: {}", response.status()),
            });
        }
        
        Ok(())
    }
}
