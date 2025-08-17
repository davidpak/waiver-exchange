#![allow(dead_code)]

use crate::{AccountId, EnqSeq, H_NONE, OrderHandle, OrderId, PriceIdx, Qty, Side, TsNorm};

#[repr(C)]
#[derive(Clone, Copy)]
pub struct Order {
    // hot (touched in match loop)
    pub id:        OrderId,
    pub acct:      AccountId,
    pub side:      Side,
    pub price_idx: PriceIdx,
    pub qty_open:  Qty,
    pub ts_norm:   TsNorm,
    pub enq_seq:   EnqSeq,

    // intrustive FIFO pointers (for Book)
    pub prev: OrderHandle,
    pub next: OrderHandle,

    // cold/debug (kept compact)
    pub typ:   u8,
    pub _pad:  u8,
    pub _pad2: u16,

}

impl Default for Order {
    fn default() -> Self {
        Self {
            id: 0, acct: 0, side: Side::Buy, price_idx: 0, qty_open: 0, ts_norm: 0, enq_seq: 0,
            prev: H_NONE, next: H_NONE, typ: 0, _pad: 0, _pad2: 0
        }
    }
}

pub struct Arena {
    buf:  Box<[Order]>,
    free: Vec<u32>,
    used: Vec<bool>,
}

impl Arena {
    pub fn with_capacity(capacity: u32) -> Self {
        assert!(capacity > 0, "arena capacity must be > 0");
        let cap = capacity as usize;
        let mut free = Vec::with_capacity(cap);
        for i in (0..cap).rev() { free.push(i as u32); }
        Self {
            buf: vec![Order::default(); cap].into_boxed_slice(),
            free,
            used: vec![false; cap],
        }
    }

    #[inline] pub fn capacity(&self) -> u32 { self.buf.len() as u32 }

    #[inline]
    pub fn alloc(&mut self) -> Option<OrderHandle> {
        let idx = self.free.pop()?;
        debug_assert!(!self.used[idx as usize], "allocating an in-use slot");
        self.used[idx as usize] = true;

        let o = &mut self.buf[idx as usize];
        o.prev = H_NONE; o.next = H_NONE;
        Some(OrderHandle(idx))
    }

    #[inline]
    pub fn free(&mut self, h: OrderHandle) {
        assert!(h != H_NONE, "cannot free H_NONE");
        let i = h.0 as usize;
        assert!(i < self.buf.len(), "handle out of range");
        assert!(self.used[i], "double free detected");
        self.used[i] = false;
        self.buf[i] = Order::default();
        self.free.push(h.0);
    }

    #[inline] pub fn get(&self, h: OrderHandle) -> &Order {
        let i = h.0 as usize;
        assert!(self.used[i], "get: slot not in use");
        &self.buf[i]
    }
    #[inline] pub fn get_mut(&mut self, h: OrderHandle) -> &mut Order {
        let i = h.0 as usize;
        assert!(self.used[i], "get_mut: slot not in use");
        &mut self.buf[i]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn arena_alloc_free_roundtrip() {
        let mut a = Arena::with_capacity(3);
        let h1 = a.alloc().expect("1");
        let h2 = a.alloc().expect("2");
        let h3 = a.alloc().expect("3");
        assert!(a.alloc().is_none(), "full");

        {
            let o = a.get_mut(h2);
            o.id = 42;
            o.qty_open = 7;
        }
        let o2 = a.get(h2);
        assert_eq!(o2.id, 42);
        assert_eq!(o2.qty_open, 7);

        a.free(h3);
        a.free(h2);
        a.free(h1);
        let h4 = a.alloc().unwrap();
        assert_eq!(h4.0, h1.0, "LIFO reuse expected");
    }

    #[test]
    #[should_panic]
    fn arena_double_free_panics() {
        let mut a = Arena::with_capacity(1);
        let h = a.alloc().unwrap();
        a.free(h);
        a.free(h);  // second free should panic (debug guard)
    }
}
