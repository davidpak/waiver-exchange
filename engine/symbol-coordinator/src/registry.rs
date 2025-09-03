use crate::types::{EngineMetadata, SymbolId, SymbolState, ThreadId, WhistleHandle};
use std::collections::HashMap;
use whistle::TickId;

/// Registry entry for a symbol
pub struct SymbolEntry {
    pub symbol_id: SymbolId,
    pub state: SymbolState,
    pub whistle_handle: WhistleHandle,
    pub thread_id: ThreadId,
}

impl SymbolEntry {
    pub fn new(
        symbol_id: SymbolId,
        thread_id: ThreadId,
        spsc_queue: whistle::InboundQueue,
        current_tick: TickId,
    ) -> Self {
        let metadata = EngineMetadata {
            symbol_id,
            thread_id,
            state: SymbolState::Registered,
            created_at: current_tick,
        };

        let whistle_handle = WhistleHandle {
            order_tx: crate::types::OrderQueueWriter::new(spsc_queue),
            metadata,
            tick_flag: std::sync::atomic::AtomicBool::new(false),
            engine: whistle::Whistle::new(whistle::EngineCfg {
                symbol: symbol_id,
                price_domain: whistle::PriceDomain { floor: 100, ceil: 10000, tick: 1 },
                bands: whistle::Bands { mode: whistle::BandMode::Percent(10) },
                batch_max: 100,
                arena_capacity: 1024,
                elastic_arena: false,
                exec_shift_bits: 16,
                exec_id_mode: whistle::ExecIdMode::Sharded,
                self_match_policy: whistle::SelfMatchPolicy::Skip,
                allow_market_cold_start: false,
                reference_price_source: whistle::ReferencePriceSource::MidpointOnWarm,
            }),
        };

        Self { symbol_id, state: SymbolState::Registered, whistle_handle, thread_id }
    }

    pub fn activate(&mut self, current_tick: TickId) {
        self.state = SymbolState::Active;
        self.whistle_handle.metadata.state = SymbolState::Active;
        self.whistle_handle.metadata.created_at = current_tick;
    }

    pub fn mark_evicting(&mut self) {
        self.state = SymbolState::Evicting;
        self.whistle_handle.metadata.state = SymbolState::Evicting;
    }

    pub fn mark_evicted(&mut self) {
        self.state = SymbolState::Evicted;
        self.whistle_handle.metadata.state = SymbolState::Evicted;
    }
}

/// Registry for tracking all symbols
pub struct SymbolRegistry {
    entries: HashMap<SymbolId, SymbolEntry>,
    current_tick: TickId,
}

impl SymbolRegistry {
    pub fn new() -> Self {
        Self { entries: HashMap::new(), current_tick: 0 }
    }

    pub fn register_symbol(
        &mut self,
        symbol_id: SymbolId,
        thread_id: ThreadId,
        spsc_queue: whistle::InboundQueue,
    ) -> Result<(), String> {
        if self.entries.contains_key(&symbol_id) {
            return Err(format!("Symbol {symbol_id} already registered"));
        }

        let entry = SymbolEntry::new(symbol_id, thread_id, spsc_queue, self.current_tick);
        self.entries.insert(symbol_id, entry);
        Ok(())
    }

    pub fn get_entry(&self, symbol_id: SymbolId) -> Option<&SymbolEntry> {
        self.entries.get(&symbol_id)
    }

    pub fn get_entry_mut(&mut self, symbol_id: SymbolId) -> Option<&mut SymbolEntry> {
        self.entries.get_mut(&symbol_id)
    }

    pub fn activate_symbol(&mut self, symbol_id: SymbolId) -> Result<(), String> {
        if let Some(entry) = self.entries.get_mut(&symbol_id) {
            entry.activate(self.current_tick);
            Ok(())
        } else {
            Err(format!("Symbol {symbol_id} not found"))
        }
    }

    pub fn evict_symbol(&mut self, symbol_id: SymbolId) -> Result<(), String> {
        if let Some(entry) = self.entries.get_mut(&symbol_id) {
            entry.mark_evicting();
            Ok(())
        } else {
            Err(format!("Symbol {symbol_id} not found"))
        }
    }

    pub fn remove_symbol(&mut self, symbol_id: SymbolId) -> Result<(), String> {
        if let Some(entry) = self.entries.get_mut(&symbol_id) {
            entry.mark_evicted();
            Ok(())
        } else {
            Err(format!("Symbol {symbol_id} not found"))
        }
    }

    pub fn is_symbol_active(&self, symbol_id: SymbolId) -> bool {
        self.entries
            .get(&symbol_id)
            .map(|entry| entry.state == SymbolState::Active)
            .unwrap_or(false)
    }

    pub fn get_active_symbols(&self) -> Vec<SymbolId> {
        self.entries
            .iter()
            .filter(|(_, entry)| entry.state == SymbolState::Active)
            .map(|(symbol_id, _)| *symbol_id)
            .collect()
    }

    pub fn update_tick(&mut self, new_tick: TickId) {
        self.current_tick = new_tick;
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}
