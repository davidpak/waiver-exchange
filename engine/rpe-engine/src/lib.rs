//! Reference Price Engine (RPE)
//!
//! This service calculates fair prices (F₀ and Fₜ) based on season projections
//! and live fantasy points. It implements the pricing formulas from the design document.

pub mod config;
pub mod engine;
pub mod models;
pub mod calculator;

pub use config::RpeConfig;
pub use engine::RpeEngine;
pub use models::*;
pub use calculator::PriceCalculator;
