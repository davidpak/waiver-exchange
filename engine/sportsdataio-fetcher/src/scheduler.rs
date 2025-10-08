use crate::config::FetcherConfig;
use crate::fetcher::SportsDataIOFetcher;
use anyhow::Result;
use chrono::{DateTime, Utc, NaiveTime};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Mutex;
use tokio::time::{sleep, interval};
use tracing::{info, warn, error};

/// Scheduler for the SportsDataIO Fetcher service
pub struct FetcherScheduler {
    config: FetcherConfig,
    fetcher: Arc<Mutex<SportsDataIOFetcher>>,
}

impl FetcherScheduler {
    /// Create a new scheduler
    pub async fn new(config: FetcherConfig) -> Result<Self> {
        let fetcher = SportsDataIOFetcher::new(config.clone()).await?;
        
        Ok(Self {
            config,
            fetcher: Arc::new(Mutex::new(fetcher)),
        })
    }
    
    /// Start the scheduler (runs indefinitely)
    pub async fn start(&mut self) -> Result<()> {
        info!("Starting SportsDataIO Fetcher Scheduler");
        
        // Start nightly projections task
        if self.config.scheduler.enable_projections {
            let fetcher = Arc::clone(&self.fetcher);
            let config = self.config.clone();
            tokio::spawn(async move {
                Self::run_nightly_projections_task(fetcher, config).await;
            });
        }
        
        // Start live stats polling task
        if self.config.scheduler.enable_live_stats {
            let fetcher = Arc::clone(&self.fetcher);
            let config = self.config.clone();
            tokio::spawn(async move {
                Self::run_live_stats_polling_task(fetcher, config).await;
            });
        }
        
        // Keep the main task alive
        loop {
            sleep(Duration::from_secs(60)).await;
        }
    }
    
    /// Run nightly projections fetch task
    async fn run_nightly_projections_task(fetcher: Arc<Mutex<SportsDataIOFetcher>>, config: FetcherConfig) {
        info!("Starting nightly projections task");
        
        loop {
            // Calculate next fetch time
            let next_fetch = Self::calculate_next_fetch_time(&config.scheduler.nightly_fetch_time);
            let now = Utc::now();
            let sleep_duration = (next_fetch - now).to_std().unwrap_or(Duration::from_secs(3600));
            
            info!("Next projections fetch scheduled for: {}", next_fetch);
            sleep(sleep_duration).await;
            
            // Run the fetch with retry logic
            match Self::run_with_retry(
                || {
                    let fetcher = Arc::clone(&fetcher);
                    async move {
                        let fetcher = fetcher.lock().await;
                        fetcher.run_season_projections_fetch().await
                    }
                },
                &config.scheduler.retry,
            ).await {
                Ok(event) => {
                    info!("Projections fetch completed: {:?}", event);
                }
                Err(e) => {
                    error!("Projections fetch failed after retries: {}", e);
                }
            }
        }
    }
    
    /// Run live stats polling task
    async fn run_live_stats_polling_task(fetcher: Arc<Mutex<SportsDataIOFetcher>>, config: FetcherConfig) {
        info!("Starting live stats polling task");
        
        let mut interval = interval(Duration::from_secs(60)); // Check every minute
        
        loop {
            interval.tick().await;
            
            // Check if we're in a live window
            if config.is_live_window() {
                let current_week = {
                    let fetcher = fetcher.lock().await;
                    fetcher.get_current_week()
                };
                
                // Run the fetch with retry logic
                match Self::run_with_retry(
                    || {
                        let fetcher = Arc::clone(&fetcher);
                        let week = current_week;
                        async move {
                            let fetcher = fetcher.lock().await;
                            fetcher.run_player_game_stats_fetch(week).await
                        }
                    },
                    &config.scheduler.retry,
                ).await {
                    Ok(event) => {
                        info!("Live stats fetch completed: {:?}", event);
                    }
                    Err(e) => {
                        error!("Live stats fetch failed after retries: {}", e);
                    }
                }
                
                // Sleep for the configured poll interval
                sleep(Duration::from_secs(config.sportsdataio.poll_minutes as u64 * 60)).await;
            } else {
                // Not in live window, sleep for a shorter interval
                sleep(Duration::from_secs(300)).await; // 5 minutes
            }
        }
    }
    
    /// Calculate the next fetch time for nightly projections
    fn calculate_next_fetch_time(fetch_time: &str) -> DateTime<Utc> {
        let now = Utc::now();
        let fetch_time = NaiveTime::parse_from_str(fetch_time, "%H:%M")
            .unwrap_or_else(|_| NaiveTime::from_hms_opt(6, 0, 0).unwrap());
        
        let today_fetch = now.date_naive().and_time(fetch_time).and_utc();
        
        if today_fetch > now {
            today_fetch
        } else {
            // Tomorrow's fetch time
            (now.date_naive() + chrono::Duration::days(1))
                .and_time(fetch_time)
                .and_utc()
        }
    }
    
    /// Run a function with retry logic
    async fn run_with_retry<F, Fut, T>(
        mut f: F,
        retry_config: &crate::config::RetryConfig,
    ) -> Result<T>
    where
        F: FnMut() -> Fut,
        Fut: std::future::Future<Output = Result<T>>,
    {
        let mut delay = Duration::from_secs(retry_config.initial_delay_secs);
        
        for attempt in 1..=retry_config.max_retries {
            match f().await {
                Ok(result) => return Ok(result),
                Err(e) => {
                    if attempt == retry_config.max_retries {
                        return Err(e);
                    }
                    
                    warn!("Attempt {} failed: {}, retrying in {:?}", attempt, e, delay);
                    sleep(delay).await;
                    
                    // Exponential backoff
                    delay = Duration::from_secs(
                        (delay.as_secs() as f64 * retry_config.backoff_multiplier)
                            .min(retry_config.max_delay_secs as f64) as u64
                    );
                }
            }
        }
        
        unreachable!()
    }
}

