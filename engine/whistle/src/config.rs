use crate::{Price, PriceDomain};

#[derive(Clone, Copy, Debug)]
pub enum BandMode { Abs(Price), Percent(u16) }
#[derive(Clone, Copy, Debug)]
pub struct Bands { pub mode: BandMode }

#[derive(Clone, Copy, Debug)]
pub enum SelfMatchPolicy { Skip, CancelResting, CancelAggressor }

#[derive(Clone, Copy, Debug)]
pub enum ExecIdMode { Sharded, External }

#[derive(Clone, Copy, Debug)]
pub enum ReferencePriceSource { SnapshotLastTrade, PriorClose, MidpointOnWarm, Manual(Price) }

#[derive(Clone, Copy, Debug)]
pub struct EngineCfg {
    pub symbol: u32,

    // Price Model
    pub price_domain: PriceDomain,
    pub bands: Bands,

    // Bounded resources & batch controls
    pub batch_max: u32,
    pub arena_capacity: u32,   // max open orders stored
    pub elastic_arena: bool,  // allow growth only at tick boundaries

    // Execution ID Policy
    pub exec_shift_bits: u8,    // bits in exec_id for local counter
    pub exec_id_mode: ExecIdMode,

    // Matching policies
    pub self_match_policy: SelfMatchPolicy,
    pub allow_market_cold_start: bool, // typically false

    // Bands reference
    pub reference_price_source: ReferencePriceSource,
}

#[derive(Debug)]
pub enum CfgError { InvalidTick, DomainEmpty, BatchZero, ArenaZero, ExecShiftTooSmallOrLarge }

impl EngineCfg {
    pub fn validate(&self) -> Result<(), CfgError> {
        if self.price_domain.tick == 0 { return Err(CfgError::InvalidTick); }
        if self.price_domain.ceil < self.price_domain.floor { return Err(CfgError::DomainEmpty); }
        if self.batch_max == 0 { return Err(CfgError::BatchZero); }
        if self.arena_capacity == 0 { return Err(CfgError::ArenaZero); }
        if self.exec_shift_bits == 0 || self.exec_shift_bits > 32 { return Err(CfgError::ExecShiftTooSmallOrLarge); }
        Ok(())
    } 
}