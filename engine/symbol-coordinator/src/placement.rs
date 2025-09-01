use crate::types::{SymbolId, ThreadId};

/// Policy for assigning symbols to threads
pub trait PlacementPolicy: std::fmt::Debug {
    fn assign_thread(&self, symbol_id: SymbolId) -> ThreadId;
}

/// Round-robin thread placement policy
#[derive(Debug)]
pub struct RoundRobinPolicy {
    num_threads: u32,
    next_thread: std::sync::atomic::AtomicU32,
}

impl RoundRobinPolicy {
    pub fn new(num_threads: u32) -> Self {
        Self { num_threads, next_thread: std::sync::atomic::AtomicU32::new(0) }
    }
}

impl PlacementPolicy for RoundRobinPolicy {
    fn assign_thread(&self, _symbol_id: SymbolId) -> ThreadId {
        let current = self.next_thread.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        current % self.num_threads
    }
}

/// Hash-based thread placement policy for deterministic assignment
#[derive(Debug)]
pub struct HashBasedPolicy {
    num_threads: u32,
}

impl HashBasedPolicy {
    pub fn new(num_threads: u32) -> Self {
        Self { num_threads }
    }
}

impl PlacementPolicy for HashBasedPolicy {
    fn assign_thread(&self, symbol_id: SymbolId) -> ThreadId {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};

        let mut hasher = DefaultHasher::new();
        symbol_id.hash(&mut hasher);
        let hash = hasher.finish();

        (hash as u32) % self.num_threads
    }
}

/// Thread pool for managing engine threads
#[derive(Debug)]
pub struct EngineThreadPool {
    num_threads: u32,
    symbols_per_thread: Vec<u32>, // Count of symbols assigned to each thread
}

impl EngineThreadPool {
    pub fn new(num_threads: u32) -> Self {
        Self { num_threads, symbols_per_thread: vec![0; num_threads as usize] }
    }

    pub fn assign_symbol(&mut self, thread_id: ThreadId) -> Result<(), String> {
        if thread_id >= self.num_threads {
            return Err(format!("Invalid thread ID: {thread_id}"));
        }

        self.symbols_per_thread[thread_id as usize] += 1;
        Ok(())
    }

    pub fn unassign_symbol(&mut self, thread_id: ThreadId) -> Result<(), String> {
        if thread_id >= self.num_threads {
            return Err(format!("Invalid thread ID: {thread_id}"));
        }

        let count = &mut self.symbols_per_thread[thread_id as usize];
        if *count > 0 {
            *count -= 1;
        }
        Ok(())
    }

    pub fn get_thread_load(&self, thread_id: ThreadId) -> Option<u32> {
        if thread_id < self.num_threads {
            Some(self.symbols_per_thread[thread_id as usize])
        } else {
            None
        }
    }

    pub fn get_least_loaded_thread(&self) -> ThreadId {
        self.symbols_per_thread
            .iter()
            .enumerate()
            .min_by_key(|(_, &count)| count)
            .map(|(thread_id, _)| thread_id as u32)
            .unwrap_or(0)
    }
}
