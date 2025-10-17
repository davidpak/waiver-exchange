use anyhow::{Context, Result};
use reqwest::Client;
use scraper::{Html, Selector};
use serde::{Deserialize, Serialize};
use std::time::Duration;
use tracing::{info, warn};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NflLeaderboardPlayer {
    pub player_id: u32,
    pub name: String,
    pub position: String,
    pub team: String,
    pub fantasy_points: f64,
    pub rank: u32,
}

use crate::types::{Player, PlayerData, WeeklyPlayer, WeeklyPlayerData};

/// NFL.com fantasy football player scraper
pub struct NflPlayerScraper {
    client: Client,
    #[allow(dead_code)]
    base_url: String,
}

impl NflPlayerScraper {
    /// Create a new NFL player scraper
    pub fn new() -> Result<Self> {
        let client = Client::builder()
            .timeout(Duration::from_secs(30))
            .user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/91.0.4472.124 Safari/537.36")
            .build()
            .context("Failed to create HTTP client")?;

        Ok(Self { client, base_url: "https://fantasy.nfl.com".to_string() })
    }

    /// Scrape season projections from NFL.com
    pub async fn scrape_season_projections(&self, season: &str) -> Result<PlayerData> {
        info!("Starting to scrape season projections for {}", season);

        // Use the clean URL for season projections
        let url = "https://fantasy.nfl.com/research/projections?position=O&sort=projectedPts&statCategory=projectedStats&statSeason=2025&statType=seasonProjectedStats";
        info!("Fetching data from: {}", url);

        // Fetch the page
        let response = self.client.get(url).send().await.context("Failed to fetch NFL.com page")?;

        if !response.status().is_success() {
            anyhow::bail!("HTTP request failed with status: {}", response.status());
        }

        let html = response.text().await.context("Failed to read response body")?;
        info!("Successfully fetched HTML ({} bytes)", html.len());

        // Parse the HTML
        let document = Html::parse_document(&html);
        let players = self.parse_player_table(&document)?;

        info!("Successfully parsed {} players", players.len());

        let mut player_data = PlayerData::new(season.to_string());
        player_data.players = players;
        player_data.sort_and_rank();

        info!(
            "Created player data with {} players, top player: {} ({})",
            player_data.players.len(),
            player_data.players[0].name,
            player_data.players[0].projected_points
        );

        Ok(player_data)
    }

    /// Scrape season projections from NFL.com with pagination
    pub async fn scrape_season_projections_with_offset(
        &self,
        season: &str,
        offset: u32,
    ) -> Result<PlayerData> {
        info!("Starting to scrape season projections for {} with offset {}", season, offset);

        // Use the URL with offset for pagination
        let url = format!("https://fantasy.nfl.com/research/projections?position=O&sort=projectedPts&statCategory=projectedStats&statSeason=2025&statType=seasonProjectedStats&offset={offset}");
        info!("Fetching data from: {}", url);

        // Fetch the page
        let response =
            self.client.get(&url).send().await.context("Failed to fetch NFL.com page")?;

        if !response.status().is_success() {
            anyhow::bail!("HTTP request failed with status: {}", response.status());
        }

        let html_content = response.text().await.context("Failed to read response body")?;

        info!("Successfully fetched HTML ({} bytes)", html_content.len());

        // Parse the HTML
        let document = Html::parse_document(&html_content);
        let players = self.parse_player_table(&document)?;

        info!("Successfully parsed {} players", players.len());

        // Create player data
        let mut player_data = PlayerData::new(season.to_string());
        player_data.players = players;
        player_data.sort_and_rank();

        info!(
            "Created player data with {} players, top player: {} ({})",
            player_data.players.len(),
            player_data.players[0].name,
            player_data.players[0].projected_points
        );

        Ok(player_data)
    }

    /// Scrape all 500 players by combining multiple pages
    pub async fn scrape_all_players(&self, season: &str, max_players: u32) -> Result<PlayerData> {
        info!("Starting to scrape all {} players for {}", max_players, season);

        let mut all_players = Vec::new();
        let mut offset = 0;
        let players_per_page = 25;
        let total_pages = max_players.div_ceil(players_per_page);

        info!("Will scrape {} pages to get {} players", total_pages, max_players);

        for page in 1..=total_pages {
            info!("Scraping page {} of {} (offset: {})", page, total_pages, offset);

            let page_data = self.scrape_season_projections_with_offset(season, offset).await?;
            all_players.extend(page_data.players);

            // Stop if we've reached our target
            if all_players.len() >= max_players as usize {
                break;
            }

            offset += players_per_page;

            // Add a small delay to be respectful to the server
            tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
        }

        // Trim to exact number if we got more than requested
        if all_players.len() > max_players as usize {
            all_players.truncate(max_players as usize);
        }

        info!("Successfully scraped {} players from {} pages", all_players.len(), total_pages);

        // Create combined player data
        let mut player_data = PlayerData::new(season.to_string());
        player_data.players = all_players;
        player_data.sort_and_rank();

        info!(
            "Created combined player data with {} players, top player: {} ({})",
            player_data.players.len(),
            player_data.players[0].name,
            player_data.players[0].projected_points
        );

        Ok(player_data)
    }

    /// Scrape weekly player stats from NFL.com with pagination
    pub async fn scrape_weekly_stats(&self, season: &str, week: u32) -> Result<WeeklyPlayerData> {
        info!("Starting to scrape weekly stats for season {} week {}", season, week);

        let mut all_players = Vec::new();
        let mut offset = 1; // Start at 1, not 0
        let page_size = 25;
        let mut total_pages = 0;

        loop {
            // Use the URL for weekly stats with offset for pagination
            let url = format!(
                "https://fantasy.nfl.com/research/players?offset={}&position=O&sort=pts&statCategory=stats&statSeason={}&statType=weekStats&statWeek={}",
                offset, season, week
            );
            info!("Fetching page {} (offset {}) from: {}", total_pages + 1, offset, url);

            // Fetch the page
            let response = self.client.get(&url).send().await.context("Failed to fetch NFL.com page")?;

            if !response.status().is_success() {
                anyhow::bail!("HTTP request failed with status: {}", response.status());
            }

            let html = response.text().await.context("Failed to read response body")?;
            info!("Successfully fetched HTML ({} bytes)", html.len());

            // Parse the HTML
            let document = Html::parse_document(&html);
            let page_players = self.parse_weekly_player_table(&document, week)?;

            if page_players.is_empty() {
                // No more players, we've reached the end
                info!("No more players found, stopping pagination");
                break;
            }

            info!("Parsed {} players from page {}", page_players.len(), total_pages + 1);
            
            // Show first 5 players from this page
            if !page_players.is_empty() {
                info!("   📊 First 5 players from page {}:", total_pages + 1);
                for (i, player) in page_players.iter().take(5).enumerate() {
                    info!("      {}. {} ({}) - {} - {:.1} pts vs {}", 
                        i + 1, player.name, player.position, player.team, 
                        player.fantasy_points, player.opponent);
                }
            }
            
            all_players.extend(page_players);
            
            total_pages += 1;
            offset += page_size;

            // Stop when we have enough players (around 450-500 for our symbols)
            if all_players.len() >= 500 {
                info!("Reached target player count ({}), stopping pagination", all_players.len());
                break;
            }

            // Safety check to prevent infinite loops
            if total_pages > 50 {
                warn!("Reached maximum page limit (50), stopping pagination");
                break;
            }

            // Small delay between requests to be respectful
            tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
        }

        info!("Successfully parsed {} total players across {} pages for week {}", all_players.len(), total_pages, week);

        let mut weekly_data = WeeklyPlayerData::new(season.to_string(), week);
        weekly_data.players = all_players;
        weekly_data.sort_by_points();

        if !weekly_data.players.is_empty() {
            info!(
                "Created weekly data with {} players, top player: {} ({:.1} pts)",
                weekly_data.players.len(),
                weekly_data.players[0].name,
                weekly_data.players[0].fantasy_points
            );
        }

        Ok(weekly_data)
    }

    /// Parse the weekly player table from HTML
    fn parse_weekly_player_table(&self, document: &Html, week: u32) -> Result<Vec<WeeklyPlayer>> {
        let mut players = Vec::new();

        // Selector for player rows - looking for rows with class starting with "player-"
        let player_row_selector = Selector::parse("tr[class*=\"player-\"]")
            .map_err(|e| anyhow::anyhow!("Failed to create player row selector: {}", e))?;

        // Selector for player name cell (the first cell with class "playerNameAndInfo")
        let name_selector = Selector::parse(".playerNameAndInfo")
            .map_err(|e| anyhow::anyhow!("Failed to create name selector: {}", e))?;

        // Selector for opponent (second column)
        let opponent_selector = Selector::parse(".playerOpponent")
            .map_err(|e| anyhow::anyhow!("Failed to create opponent selector: {}", e))?;

        // Selector for weekly fantasy points (last column with class "statTotal")
        let points_selector = Selector::parse(".statTotal")
            .map_err(|e| anyhow::anyhow!("Failed to create points selector: {}", e))?;

        for (row_index, row) in document.select(&player_row_selector).enumerate() {
            if row_index >= 1000 {
                // Limit to top 1000 players
                break;
            }

            match self.parse_weekly_player_row(&row, &name_selector, &opponent_selector, &points_selector, week) {
                Ok(Some(player)) => {
                    players.push(player);
                }
                Ok(None) => {
                    // Skip this row (no valid data)
                    continue;
                }
                Err(e) => {
                    warn!("Failed to parse weekly player row {}: {}", row_index, e);
                    continue;
                }
            }
        }

        Ok(players)
    }

    /// Parse a single weekly player row
    fn parse_weekly_player_row(
        &self,
        row: &scraper::ElementRef,
        name_selector: &Selector,
        opponent_selector: &Selector,
        points_selector: &Selector,
        week: u32,
    ) -> Result<Option<WeeklyPlayer>> {
        // Extract player ID from the row's class attribute
        let player_id = self.extract_player_id(row)?;

        // Extract player name and basic info
        let (name, position, team) = self.extract_player_info(row, name_selector)?;

        // Extract opponent
        let opponent = self.extract_opponent(row, opponent_selector)?;

        // Extract weekly fantasy points
        let fantasy_points = self.extract_weekly_points(row, points_selector)?;

        // Skip if we don't have valid data
        if name.is_empty() || fantasy_points < 0.0 {
            return Ok(None);
        }

        Ok(Some(WeeklyPlayer {
            player_id,
            name,
            position,
            team,
            week,
            fantasy_points,
            opponent,
            rank: None, // Will be assigned after sorting
        }))
    }

    /// Extract opponent team from the opponent cell
    fn extract_opponent(&self, row: &scraper::ElementRef, opponent_selector: &Selector) -> Result<String> {
        let opponent_cell = row.select(opponent_selector).next()
            .context("Could not find opponent cell")?;
        
        let opponent_text = opponent_cell.text().collect::<String>().trim().to_string();
        Ok(opponent_text)
    }

    /// Extract weekly fantasy points from the points cell
    fn extract_weekly_points(&self, row: &scraper::ElementRef, points_selector: &Selector) -> Result<f64> {
        let points_cell = row.select(points_selector).next()
            .context("Could not find points cell")?;
        
        let points_text = points_cell.text().collect::<String>().trim().to_string();
        
        if points_text.is_empty() || points_text == "-" {
            return Ok(0.0);
        }
        
        points_text.parse::<f64>()
            .map_err(|e| anyhow::anyhow!("Failed to parse fantasy points '{}': {}", points_text, e))
    }

    /// Parse the player table from HTML
    fn parse_player_table(&self, document: &Html) -> Result<Vec<Player>> {
        let mut players = Vec::new();

        // Selector for player rows - looking for rows with class starting with "player-"
        let player_row_selector = Selector::parse("tr[class*=\"player-\"]")
            .map_err(|e| anyhow::anyhow!("Failed to create player row selector: {}", e))?;

        // Selector for player name cell (the first cell with class "playerNameAndInfo")
        let name_selector = Selector::parse(".playerNameAndInfo")
            .map_err(|e| anyhow::anyhow!("Failed to create name selector: {}", e))?;

        // Selector for season total fantasy points (last column with class "stat projected numeric sorted last")
        let points_selector = Selector::parse(".stat.projected.numeric.sorted.last")
            .map_err(|e| anyhow::anyhow!("Failed to create points selector: {}", e))?;

        for (row_index, row) in document.select(&player_row_selector).enumerate() {
            if row_index >= 500 {
                // Limit to top 500 players
                break;
            }

            match self.parse_player_row(&row, &name_selector, &points_selector) {
                Ok(Some(player)) => {
                    players.push(player);
                }
                Ok(None) => {
                    // Skip this row (no valid data)
                    continue;
                }
                Err(e) => {
                    warn!("Failed to parse player row {}: {}", row_index, e);
                    continue;
                }
            }
        }

        Ok(players)
    }

    /// Parse a single player row
    fn parse_player_row(
        &self,
        row: &scraper::ElementRef,
        name_selector: &Selector,
        points_selector: &Selector,
    ) -> Result<Option<Player>> {
        // Extract player ID from the row's class attribute
        let player_id = self.extract_player_id(row)?;

        // Extract player name and basic info
        let (name, position, team) = self.extract_player_info(row, name_selector)?;

        // Extract projected fantasy points
        let projected_points = self.extract_projected_points(row, points_selector)?;

        // Skip if we don't have valid data
        if name.is_empty() || projected_points <= 0.0 {
            return Ok(None);
        }

        Ok(Some(Player {
            player_id,
            name,
            position,
            team,
            projected_points,
            symbol_id: None, // Will be assigned later
            rank: None,      // Will be assigned after sorting
        }))
    }

    /// Extract player ID from row class attribute
    fn extract_player_id(&self, row: &scraper::ElementRef) -> Result<String> {
        let class_attr = row.value().attr("class").context("Row missing class attribute")?;

        // Look for pattern like "player-2560955"
        for class in class_attr.split_whitespace() {
            if class.starts_with("player-") {
                let id = class.strip_prefix("player-").context("Invalid player ID format")?;
                return Ok(id.to_string());
            }
        }

        anyhow::bail!("Could not find player ID in class attribute: {}", class_attr);
    }

    /// Extract player name, position, and team from the name cell
    fn extract_player_info(
        &self,
        row: &scraper::ElementRef,
        name_selector: &Selector,
    ) -> Result<(String, String, String)> {
        // Find the player name cell (the first cell with class "playerNameAndInfo")
        let name_cell =
            row.select(name_selector).next().context("Could not find player name cell")?;

        // Extract player name from the <a> tag with class "playerName"
        let player_name_selector = Selector::parse("a.playerName")
            .map_err(|e| anyhow::anyhow!("Failed to create player name selector: {}", e))?;
        let name_element = name_cell
            .select(&player_name_selector)
            .next()
            .context("Could not find player name link")?;
        let name = name_element.text().collect::<String>().trim().to_string();

        // Find the <em> tag that contains "Position - Team"
        let em_selector = Selector::parse("em")
            .map_err(|e| anyhow::anyhow!("Failed to create em selector: {}", e))?;
        let em_element =
            name_cell.select(&em_selector).next().context("Could not find position/team info")?;

        let pos_team_text = em_element.text().collect::<String>().trim().to_string();

        // Parse format like "QB - BAL"
        let parts: Vec<&str> = pos_team_text.split(" - ").collect();
        if parts.len() != 2 {
            anyhow::bail!("Invalid position/team format: {}", pos_team_text);
        }

        let position = parts[0].to_string();
        let team = parts[1].to_string();

        Ok((name, position, team))
    }

    /// Extract projected fantasy points from the projected points cell
    fn extract_projected_points(
        &self,
        row: &scraper::ElementRef,
        points_selector: &Selector,
    ) -> Result<f64> {
        let points_cell =
            row.select(points_selector).next().context("Could not find projected points cell")?;

        let points_text = points_cell.text().collect::<String>();
        let points_text = points_text.trim();

        if points_text.is_empty() {
            return Ok(0.0);
        }

        points_text
            .parse::<f64>()
            .with_context(|| format!("Failed to parse projected points: '{points_text}'"))
    }

    /// Scrape NFL Fantasy leaderboard page
    pub async fn scrape_nfl_leaderboard_page(&self, url: &str) -> Result<Vec<NflLeaderboardPlayer>, Box<dyn std::error::Error>> {
        let response = self.client.get(url)
            .header("User-Agent", "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36")
            .send()
            .await?;

        if !response.status().is_success() {
            return Err(format!("HTTP error: {}", response.status()).into());
        }

        let html = response.text().await?;
        let document = scraper::Html::parse_document(&html);
        
        // Selector for player rows
        let player_selector = scraper::Selector::parse("tr[class*=\"player-\"]").unwrap();
        let name_selector = scraper::Selector::parse("a.playerName").unwrap();
        let position_selector = scraper::Selector::parse("em").unwrap();
        let points_selector = scraper::Selector::parse("span.playerSeasonTotal").unwrap();
        
        let mut players = Vec::new();
        
        for row in document.select(&player_selector) {
            // Extract player ID from class name
            let class_attr = row.value().attr("class").unwrap_or("");
            let player_id = if let Some(start) = class_attr.find("player-") {
                let id_part = &class_attr[start + 7..];
                if let Some(end) = id_part.find(' ') {
                    id_part[..end].parse::<u32>().unwrap_or(0)
                } else {
                    id_part.parse::<u32>().unwrap_or(0)
                }
            } else {
                continue;
            };
            
            // Extract player name
            let name = row.select(&name_selector)
                .next()
                .map(|e| e.text().collect::<String>().trim().to_string())
                .unwrap_or_default();
            
            if name.is_empty() {
                continue;
            }
            
            // Extract position and team
            let position_text = row.select(&position_selector)
                .next()
                .map(|e| e.text().collect::<String>())
                .unwrap_or_default();
            
            let (position, team) = if position_text.contains(" - ") {
                let parts: Vec<&str> = position_text.split(" - ").collect();
                if parts.len() >= 2 {
                    (parts[0].trim().to_string(), parts[1].trim().to_string())
                } else {
                    (position_text.clone(), "".to_string())
                }
            } else {
                (position_text, "".to_string())
            };
            
            // Extract fantasy points
            let points_text = row.select(&points_selector)
                .next()
                .map(|e| e.text().collect::<String>())
                .unwrap_or_default();
            
            let fantasy_points = points_text.parse::<f64>().unwrap_or(0.0);
            
            players.push(NflLeaderboardPlayer {
                player_id,
                name,
                position,
                team,
                fantasy_points,
                rank: 0, // Will be set by caller
            });
        }
        
        Ok(players)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_points_to_currency_conversion() {
        let player_data = PlayerData::new("2025".to_string());

        assert_eq!(player_data.points_to_currency(38.76), 387);
        assert_eq!(player_data.points_to_currency(25.5), 255);
        assert_eq!(player_data.points_to_currency(0.0), 0);
    }

    #[test]
    fn test_sort_and_rank() {
        let mut player_data = PlayerData::new("2025".to_string());

        player_data.players.push(Player {
            player_id: "1".to_string(),
            name: "Player A".to_string(),
            position: "QB".to_string(),
            team: "TEAM".to_string(),
            projected_points: 20.0,
            symbol_id: None,
            rank: None,
        });

        player_data.players.push(Player {
            player_id: "2".to_string(),
            name: "Player B".to_string(),
            position: "RB".to_string(),
            team: "TEAM".to_string(),
            projected_points: 30.0,
            symbol_id: None,
            rank: None,
        });

        player_data.sort_and_rank();

        assert_eq!(player_data.players[0].name, "Player B");
        assert_eq!(player_data.players[0].rank, Some(1));
        assert_eq!(player_data.players[1].name, "Player A");
        assert_eq!(player_data.players[1].rank, Some(2));
    }
}
