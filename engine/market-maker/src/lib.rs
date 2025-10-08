//! Market Maker Service
//!
//! This service provides liquidity by posting bid/ask quotes around fair prices.
//! It monitors order books, fetches fair prices from the RPE engine, and posts
//! market making orders when conditions are met.

pub mod config;
pub mod service;
pub mod cache;
pub mod models;

pub use config::MarketMakerConfig;
pub use service::MarketMakerService;
pub use cache::FairPriceCache;
pub use models::*;
