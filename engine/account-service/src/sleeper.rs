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
    pub roster_id: u32, // Changed from String to u32
}

/// Sleeper user response
#[derive(Debug, Deserialize)]
pub struct SleeperUser {
    pub user_id: String,
    pub username: String,
    pub display_name: String,
}

/// Sleeper league response (matches actual API response)
#[derive(Debug, Deserialize)]
pub struct SleeperLeague {
    pub league_id: String,
    pub name: String,
    pub season: String,
    pub total_rosters: u32,
    pub status: String,
    pub sport: String,
    pub settings: serde_json::Value,
    pub season_type: String,
    pub scoring_settings: serde_json::Value,
    pub roster_positions: Vec<String>,
    pub previous_league_id: Option<String>,
    pub draft_id: String,
    pub avatar: Option<String>,
}

/// Sleeper roster response (matches actual API response)
#[derive(Debug, Deserialize)]
pub struct SleeperRoster {
    pub roster_id: u32, // Changed from String to u32
    pub owner_id: String,
    pub league_id: String,
    pub starters: Vec<String>,
    pub reserve: Vec<String>,
    pub players: Vec<String>,
    pub settings: serde_json::Value,
}

/// Sleeper matchup response
#[derive(Debug, Deserialize)]
pub struct SleeperMatchup {
    pub matchup_id: String,
    pub roster_id: String,
    pub points: f64,
}

/// NFL state response
#[derive(Debug, Deserialize)]
pub struct NflState {
    pub week: u32,
    pub season_type: String,
    pub season_start_date: String,
    pub season: String,
    pub previous_season: String,
    pub leg: u32,
    pub league_season: String,
    pub league_create_season: String,
    pub display_week: u32,
}

impl SleeperClient {
    /// Create a new Sleeper API client
    pub fn new(config: SleeperConfig) -> Self {
        Self { config, client: reqwest::Client::new() }
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

    /// Get user info by user ID (returns username, display_name, etc.)
    pub async fn get_user_by_id(&self, user_id: &str) -> Result<SleeperUser, AccountServiceError> {
        let url = format!("{}/user/{}", self.config.api_base_url, user_id);
        tracing::info!("SleeperClient::get_user_by_id calling URL: {}", url);
        let response = self.client.get(&url).send().await?;

        if !response.status().is_success() {
            let error_msg = format!("Failed to get user by ID: {}", response.status());
            tracing::error!("Sleeper API error: {}", error_msg);
            return Err(AccountServiceError::SleeperApiError {
                message: error_msg,
            });
        }

        let user: SleeperUser = response.json().await?;
        tracing::info!("Sleeper API response: user_id={}, username={}, display_name={}", 
                      user.user_id, user.username, user.display_name);
        Ok(user)
    }

    /// Get user's leagues for a season (simplified approach)
    pub async fn get_user_leagues(
        &self,
        user_id: &str,
        season: &str,
    ) -> Result<Vec<LeagueOption>, AccountServiceError> {
        let url = format!("{}/user/{}/leagues/nfl/{}", self.config.api_base_url, user_id, season);
        println!("DEBUG: Fetching leagues from URL: {}", url);

        let response = self.client.get(&url).send().await?;
        let status = response.status();
        println!("DEBUG: Response status: {}", status);

        if !status.is_success() {
            let error_text = response.text().await.unwrap_or_default();
            println!("DEBUG: Error response: {}", error_text);
            return Err(AccountServiceError::SleeperApiError {
                message: format!("Failed to get leagues: {} - {}", status, error_text),
            });
        }

        let response_text = response.text().await?;
        println!("DEBUG: Raw API response: {}", response_text);

        // Simple approach: parse as JSON value and handle null/empty cases
        let json_value: serde_json::Value = serde_json::from_str(&response_text).map_err(|e| {
            AccountServiceError::SleeperApiError {
                message: format!("Failed to parse JSON: {} - Response: {}", e, response_text),
            }
        })?;

        let leagues_array = match json_value {
            serde_json::Value::Null => {
                println!("DEBUG: API returned null - user has no leagues for season {}", season);
                return Ok(Vec::new());
            }
            serde_json::Value::Array(arr) => arr,
            _ => {
                return Err(AccountServiceError::SleeperApiError {
                    message: format!("Expected array, got: {}", json_value),
                });
            }
        };

        println!(
            "DEBUG: Found {} leagues for user {} in season {}",
            leagues_array.len(),
            user_id,
            season
        );

        // Process each league
        let mut league_options = Vec::new();
        for (i, league_json) in leagues_array.iter().enumerate() {
            println!("DEBUG: Processing league {}: {:?}", i, league_json);

            // Extract basic league info
            let league_id =
                league_json.get("league_id").and_then(|v| v.as_str()).ok_or_else(|| {
                    AccountServiceError::SleeperApiError {
                        message: format!("Missing league_id in league {}", i),
                    }
                })?;

            let league_name =
                league_json.get("name").and_then(|v| v.as_str()).unwrap_or("Unnamed League");

            let league_season =
                league_json.get("season").and_then(|v| v.as_str()).unwrap_or(season);

            // Get rosters for this league to find user's roster
            let rosters_url = format!("{}/league/{}/rosters", self.config.api_base_url, league_id);
            let rosters_response = self.client.get(&rosters_url).send().await?;

            if rosters_response.status().is_success() {
                let rosters_text = rosters_response.text().await?;
                println!("DEBUG: Rosters response for league {}: {}", league_id, rosters_text);

                let rosters_array: Vec<serde_json::Value> = serde_json::from_str(&rosters_text)
                    .map_err(|e| AccountServiceError::SleeperApiError {
                        message: format!(
                            "Failed to parse rosters: {} - Response: {}",
                            e, rosters_text
                        ),
                    })?;

                // Find user's roster
                for roster_json in rosters_array {
                    let owner_id = roster_json.get("owner_id").and_then(|v| v.as_str());

                    if owner_id == Some(user_id) {
                        let roster_id = roster_json
                            .get("roster_id")
                            .and_then(|v| v.as_u64())
                            .ok_or_else(|| AccountServiceError::SleeperApiError {
                                message: format!(
                                    "Missing roster_id for user in league {}",
                                    league_id
                                ),
                            })?;

                        println!("DEBUG: Found user roster {} in league {}", roster_id, league_id);
                        league_options.push(LeagueOption {
                            id: league_id.to_string(),
                            name: league_name.to_string(),
                            season: league_season.to_string(),
                            roster_id: roster_id as u32,
                        });
                        break;
                    }
                }
            } else {
                println!(
                    "DEBUG: Failed to get rosters for league {}: {}",
                    league_id,
                    rosters_response.status()
                );
            }
        }

        Ok(league_options)
    }

    /// Get season total fantasy points for a roster (Step B from design doc)
    pub async fn get_season_points(
        &self,
        league_id: &str,
        roster_id: &str,
    ) -> Result<u32, AccountServiceError> {
        let url = format!("{}/league/{}/rosters", self.config.api_base_url, league_id);
        println!("DEBUG: Fetching rosters from URL: {}", url);

        let response = self.client.get(&url).send().await?;
        let status = response.status();
        println!("DEBUG: Rosters response status: {}", status);

        if !status.is_success() {
            let error_text = response.text().await.unwrap_or_default();
            return Err(AccountServiceError::SleeperApiError {
                message: format!("Failed to get rosters: {} - {}", status, error_text),
            });
        }

        let response_text = response.text().await?;
        println!("DEBUG: Rosters raw response: {}", response_text);

        // Parse as JSON array
        let rosters_array: Vec<serde_json::Value> =
            serde_json::from_str(&response_text).map_err(|e| {
                AccountServiceError::SleeperApiError {
                    message: format!(
                        "Failed to parse rosters JSON: {} - Response: {}",
                        e, response_text
                    ),
                }
            })?;

        // Parse roster_id from string to u32 for comparison
        let roster_id_num =
            roster_id.parse::<u32>().map_err(|_| AccountServiceError::SleeperApiError {
                message: format!("Invalid roster_id: {}", roster_id),
            })?;

        // Find the specific roster and get its fantasy points
        for roster_json in rosters_array {
            let roster_id_from_json =
                roster_json.get("roster_id").and_then(|v| v.as_u64()).unwrap_or(0) as u32;

            if roster_id_from_json == roster_id_num {
                println!("DEBUG: Found roster {} in league {}", roster_id_num, league_id);

                // Extract fantasy points from settings
                if let Some(settings) = roster_json.get("settings") {
                    let fpts = settings.get("fpts").and_then(|v| v.as_u64()).unwrap_or(0) as u32;

                    let fpts_decimal =
                        settings.get("fpts_decimal").and_then(|v| v.as_u64()).unwrap_or(0) as u32;

                    // Combine as fpts + fpts_decimal/100 (as per design doc)
                    let total_points = fpts + (fpts_decimal / 100);

                    println!(
                        "DEBUG: Fantasy points - fpts: {}, fpts_decimal: {}, total: {}",
                        fpts, fpts_decimal, total_points
                    );
                    return Ok(total_points);
                } else {
                    println!("DEBUG: No settings found for roster {}", roster_id_num);
                    return Ok(0);
                }
            }
        }

        Err(AccountServiceError::SleeperApiError {
            message: format!("Roster {} not found in league {}", roster_id, league_id),
        })
    }

    /// Get weekly matchup for a specific week
    pub async fn get_weekly_matchup(
        &self,
        league_id: &str,
        week: u32,
    ) -> Result<Vec<SleeperMatchup>, AccountServiceError> {
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

    /// Get current NFL state
    pub async fn get_nfl_state(&self) -> Result<NflState, AccountServiceError> {
        let url = format!("{}/state/nfl", self.config.api_base_url);
        println!("DEBUG: Fetching NFL state from URL: {}", url);

        let response = self.client.get(&url).send().await?;
        let status = response.status();
        println!("DEBUG: NFL state response status: {}", status);

        if !status.is_success() {
            let error_text = response.text().await.unwrap_or_default();
            return Err(AccountServiceError::SleeperApiError {
                message: format!("Failed to get NFL state: {} - {}", status, error_text),
            });
        }

        let response_text = response.text().await?;
        println!("DEBUG: NFL state raw response: {}", response_text);

        let state: NflState = serde_json::from_str(&response_text).map_err(|e| {
            AccountServiceError::SleeperApiError {
                message: format!(
                    "Failed to parse NFL state JSON: {} - Response: {}",
                    e, response_text
                ),
            }
        })?;

        println!(
            "DEBUG: Parsed NFL state - season: {}, league_season: {}",
            state.season, state.league_season
        );
        Ok(state)
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
