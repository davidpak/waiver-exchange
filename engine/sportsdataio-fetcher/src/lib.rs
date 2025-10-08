//! SportsDataIO Fetcher Service
//!
//! This service fetches NFL player data from SportsDataIO API and stores it in our database.
//! It handles both season projections (nightly) and live game stats (during game windows).

pub mod config;
pub mod fetcher;
pub mod models;
pub mod scheduler;

pub use config::FetcherConfig;
pub use fetcher::SportsDataIOFetcher;
pub use models::*;
pub use scheduler::FetcherScheduler;
