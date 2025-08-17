#![allow(dead_code)]

use crate::{OrderHandle, OrderId, H_NONE};

#[derive(Clone, Copy, Debug)]
struct Entry { key: u64, val: u32 } // key=0 => EMPTY, key=1 => TOMBSTONE

const EMPTY: u64 = 0;
const TOMBSTONE: u64 = 1;

// SplitMix64: fast, reproducible 64-bit hash
#[inline]
fn splitmix64(mut x: u64) -> u64 {
    x = x.wrapping_add(0x9E3779B97F4A7C15);
    let mut z = x;
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58476D1CE4E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D049BB133111EB);
    z ^ (z >> 31)
}

pub struct OrderIndex {
    mask: usize,
    tabs: Box<[Entry]>,
    len: usize,
    tombs: usize,
}

#[derive(Debug, PartialEq, Eq)]
pub enum InsertErr { Full, Duplicate }

impl OrderIndex {
    // cap_pow2 must be a power of two >= 8. Recommend >= 2x peak open orders
    pub fn with_capacity_pow2(cap_pow2: usize) -> Self {
        assert!(cap_pow2.is_power_of_two() && cap_pow2 >= 8, "capacity must be pow2 >= 8");
        Self {
            mask: cap_pow2 - 1,
            tabs: vec![Entry { key: EMPTY, val: 0}; cap_pow2].into_boxed_slice(),
            len: 0, tombs: 0,
        }
    }

    #[inline] pub fn capacity(&self) -> usize { self.tabs.len() }
    #[inline] pub fn len(&self) -> usize { self.len }

    pub fn insert(&mut self, key: OrderId, h: OrderHandle) -> Result<(), InsertErr> {
        assert!(key != EMPTY && key != TOMBSTONE, "reserved keys");
        assert!(h != H_NONE, "invalid handle");
        let mut idx = (splitmix64(key) as usize) & self.mask;
        let mut first_tomb: Option<usize> = None;

        loop {
            let e = &self.tabs[idx];
            if e.key == EMPTY {
                let slot = first_tomb.unwrap_or(idx);
                self.tabs[slot] = Entry { key, val: h.0 };
                if first_tomb.is_some() { self.tombs -= 1; }
                self.len += 1;
                return Ok(());
            }
            if e.key == TOMBSTONE {
                if first_tomb.is_none() { first_tomb = Some(idx); }
            } else if e.key == key {
                return Err(InsertErr::Duplicate);
            }
            idx = (idx + 1) & self.mask;

            if first_tomb.is_none() && self.len + self.tombs >= self.capacity() - 1 {
                return Err(InsertErr::Full);
            }
        }
    }

    pub fn get(&self, key: OrderId) -> Option<OrderHandle> {
        if key == EMPTY || key == TOMBSTONE { return None; }
        let mut idx = (splitmix64(key) as usize) & self.mask;
        loop {
            let e = &self.tabs[idx];
            if e.key == EMPTY { return None; }
            if e.key == key { return Some(OrderHandle(e.val)); }
            idx = (idx + 1) & self.mask;
        }
    }

    pub fn remove(&mut self, key: OrderId) -> Option<OrderHandle> {
        if key == EMPTY || key == TOMBSTONE { return None; }
        let mut idx = (splitmix64(key) as usize) & self.mask;
        loop {
            let e = self.tabs[idx];
            if e.key == EMPTY { return None; }
            if e.key == key {
                self.tabs[idx] = Entry { key: TOMBSTONE, val: 0 };
                self.len -= 1;
                self.tombs += 1;
                return Some(OrderHandle(e.val));
            }
            idx = (idx + 1) & self.mask;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn insert_get_remove_basic() {
        let mut ix = OrderIndex::with_capacity_pow2(16);
        assert_eq!(ix.len(), 0);

        ix.insert(10, OrderHandle(3)).unwrap();
        ix.insert(42, OrderHandle(7)).unwrap();
        ix.insert(99, OrderHandle(1)).unwrap();

        assert_eq!(ix.get(10).unwrap().0, 3);
        assert_eq!(ix.get(42).unwrap().0, 7);
        assert_eq!(ix.get(99).unwrap().0, 1);
        assert!(ix.get(11).is_none());

        assert_eq!(ix.remove(42).unwrap().0, 7);
        assert!(ix.get(42).is_none());
        // tombstone reuse
        ix.insert(142, OrderHandle(9)).unwrap();
        assert_eq!(ix.get(142).unwrap().0, 9);
    }

    #[test]
    fn handles_wraparound_linear_probe() {
        let mut ix = OrderIndex::with_capacity_pow2(8);
        for k in [2u64, 10, 18, 26] {
            ix.insert(k, OrderHandle((k as u32) & 0xFFFF)).unwrap();
        }
        for k in [2u64, 10, 18, 26] {
            assert!(ix.get(k).is_some());
        }
        ix.remove(18);
        ix.insert(34, OrderHandle(34)).unwrap();
        assert!(ix.get(34).is_some());
    }

    #[test]
    fn duplicate_rejected() {
        let mut ix = OrderIndex::with_capacity_pow2(8);
        ix.insert(5, OrderHandle(1)).unwrap();
        assert_eq!(ix.insert(5, OrderHandle(2)), Err(InsertErr::Duplicate));
    }
}