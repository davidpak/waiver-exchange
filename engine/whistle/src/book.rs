// engine/whistle/src/book.rs
#![allow(dead_code)]

use crate::{Arena, Bitset, H_NONE, OrderHandle, PriceDomain, PriceIdx, Qty, Side};

/// FIFO queue + running total for a single price level.
#[derive(Clone, Copy)]
pub struct Level {
    pub head: OrderHandle,
    pub tail: OrderHandle,
    pub total_qty: Qty,
}

impl Default for Level {
    fn default() -> Self {
        Self { head: H_NONE, tail: H_NONE, total_qty: 0 }
    }
}

/// Two-ladder order book (bids & asks) with intrusive FIFOs and bitset navigation.
/// Matches spec §§2.2.4–2.2.6.
pub struct Book {
    pub dom: PriceDomain,

    bids: Box<[Level]>,
    asks: Box<[Level]>,

    non_empty_bids: Bitset,
    non_empty_asks: Bitset,

    best_bid_idx: Option<PriceIdx>,
    best_ask_idx: Option<PriceIdx>,
}

impl Book {
    pub fn new(dom: PriceDomain) -> Self {
        let n = dom.ladder_len();
        Self {
            dom,
            bids: vec![Level::default(); n].into_boxed_slice(),
            asks: vec![Level::default(); n].into_boxed_slice(),
            non_empty_bids: Bitset::with_len(n),
            non_empty_asks: Bitset::with_len(n),
            best_bid_idx: None,
            best_ask_idx: None,
        }
    }

    #[inline]
    fn levels_mut(&mut self, side: Side) -> &mut [Level] {
        match side {
            Side::Buy => &mut self.bids,
            Side::Sell => &mut self.asks,
        }
    }

    #[inline]
    fn levels(&self, side: Side) -> &[Level] {
        match side {
            Side::Buy => &self.bids,
            Side::Sell => &self.asks,
        }
    }

    #[inline]
    fn bitset_mut(&mut self, side: Side) -> &mut Bitset {
        match side {
            Side::Buy => &mut self.non_empty_bids,
            Side::Sell => &mut self.non_empty_asks,
        }
    }

    #[inline]
    fn set_best_on_insert(&mut self, side: Side, pidx: PriceIdx) {
        match side {
            Side::Buy => {
                if self.best_bid_idx.is_none_or(|b| pidx > b) {
                    self.best_bid_idx = Some(pidx);
                }
            }
            Side::Sell => {
                if self.best_ask_idx.is_none_or(|b| pidx < b) {
                    self.best_ask_idx = Some(pidx);
                }
            }
        }
    }

    fn recompute_best_after_empty(&mut self, side: Side, emptied_idx: PriceIdx) {
        match side {
            Side::Buy => {
                if self.best_bid_idx == Some(emptied_idx) {
                    self.best_bid_idx = if emptied_idx == 0 {
                        None
                    } else {
                        self.prev_bid_at_or_below(emptied_idx.saturating_sub(1))
                    };
                }
            }
            Side::Sell => {
                if self.best_ask_idx == Some(emptied_idx) {
                    self.best_ask_idx = self.next_ask_at_or_above(emptied_idx.saturating_add(1));
                }
            }
        }
    }

    /// Insert at tail of level FIFO (O(1)) and update totals/best pointers.
    /// Caller guarantees `h` points to an in-use order with matching `price_idx`/`side`.
    /// Insert at tail of level FIFO (O(1)) and update totals/best pointers.
    /// Caller guarantees `h` points to an in-use order with matching `price_idx`/`side`.
    pub fn insert_tail(
        &mut self,
        arena: &mut Arena,
        side: Side,
        h: OrderHandle,
        pidx: PriceIdx,
        qty: Qty,
    ) {
        let levels = self.levels_mut(side);
        let lvl = &mut levels[pidx as usize];

        if lvl.tail == H_NONE {
            // Empty level → becomes single element
            lvl.head = h;
            lvl.tail = h;
        } else {
            // Append to tail using short reborrow scopes
            let t = lvl.tail;
            {
                let tail_o = arena.get_mut(t);
                tail_o.next = h;
            }
            {
                let new_o = arena.get_mut(h);
                new_o.prev = t;
            }
            lvl.tail = h;
        }

        // Update level totals
        lvl.total_qty = lvl.total_qty.saturating_add(qty);

        // Mark non-empty + maybe update best pointers
        self.bitset_mut(side).set(pidx as usize);
        self.set_best_on_insert(side, pidx);
    }

    /// Unlink a fully-filled order from its level (O(1)) and adjust totals.
    /// Does not free the arena slot; caller decides lifetime.
    pub fn unlink(&mut self, arena: &mut Arena, side: Side, h: OrderHandle) {
        let (pidx_usize, prev, next, qty_open) = {
            let o = arena.get(h);
            (o.price_idx as usize, o.prev, o.next, o.qty_open)
        };

        let levels = self.levels_mut(side);
        let lvl = &mut levels[pidx_usize];

        // Unlink from intrusive list; keep mutable borrows short-lived.
        if prev != H_NONE {
            {
                let prev_o = arena.get_mut(prev);
                prev_o.next = next;
            }
        } else {
            // h was head
            lvl.head = next;
        }

        if next != H_NONE {
            {
                let next_o = arena.get_mut(next);
                next_o.prev = prev;
            }
        } else {
            // h was tail
            lvl.tail = prev;
        }

        // Decrease level total by the order's remaining open qty
        lvl.total_qty = lvl.total_qty.saturating_sub(qty_open);

        // If level becomes empty, clear bit and recompute best pointer
        if lvl.head == H_NONE {
            self.bitset_mut(side).clear(pidx_usize);
            self.recompute_best_after_empty(side, pidx_usize as PriceIdx);
        }

        // Scrub removed order's intrusive pointers (defensive)
        {
            let o_mut = arena.get_mut(h);
            o_mut.prev = H_NONE;
            o_mut.next = H_NONE;
        }
    }

    /// Adjust level total for a partial fill (order stays in place).
    #[inline]
    pub fn partial_fill(&mut self, side: Side, pidx: PriceIdx, traded: Qty) {
        let lvl = &mut self.levels_mut(side)[pidx as usize];
        lvl.total_qty = lvl.total_qty.saturating_sub(traded);
    }

    #[inline]
    pub fn best_bid(&self) -> Option<PriceIdx> {
        self.best_bid_idx
    }
    #[inline]
    pub fn best_ask(&self) -> Option<PriceIdx> {
        self.best_ask_idx
    }

    #[inline]
    pub fn next_ask_at_or_above(&self, i: PriceIdx) -> Option<PriceIdx> {
        self.non_empty_asks.next_one_at_or_after(i as usize).map(|x| x as PriceIdx)
    }

    #[inline]
    pub fn prev_bid_at_or_below(&self, i: PriceIdx) -> Option<PriceIdx> {
        self.non_empty_bids.prev_one_at_or_before(i as usize).map(|x| x as PriceIdx)
    }

    #[inline]
    pub fn level_qty(&self, side: Side, i: PriceIdx) -> Qty {
        self.levels(side)[i as usize].total_qty
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Arena, OrderHandle, PriceDomain, Side};

    fn dom_100_200_5() -> PriceDomain {
        PriceDomain { floor: 100, ceil: 200, tick: 5 }
    }

    #[test]
    fn insert_updates_totals_and_best() {
        let dom = dom_100_200_5();
        let mut book = Book::new(dom);
        let mut arena = Arena::with_capacity(8);

        // helper to alloc a resting order
        let mut alloc_at = |side: Side, price: u32, qty: Qty| -> OrderHandle {
            let h = arena.alloc().unwrap();
            let pidx = dom.idx(price).unwrap();
            let o = arena.get_mut(h);
            o.side = side;
            o.price_idx = pidx;
            o.qty_open = qty;
            h
        };

        let b1 = alloc_at(Side::Buy, 115, 10);
        let b2 = alloc_at(Side::Buy, 120, 4);
        let a1 = alloc_at(Side::Sell, 110, 3);

        book.insert_tail(&mut arena, Side::Buy, b1, dom.idx(115).unwrap(), 10);
        assert_eq!(book.best_bid(), Some(dom.idx(115).unwrap()));
        assert_eq!(book.level_qty(Side::Buy, dom.idx(115).unwrap()), 10);

        book.insert_tail(&mut arena, Side::Buy, b2, dom.idx(120).unwrap(), 4);
        assert_eq!(book.best_bid(), Some(dom.idx(120).unwrap()));
        assert_eq!(book.level_qty(Side::Buy, dom.idx(120).unwrap()), 4);

        book.insert_tail(&mut arena, Side::Sell, a1, dom.idx(110).unwrap(), 3);
        assert_eq!(book.best_ask(), Some(dom.idx(110).unwrap()));
        assert_eq!(book.level_qty(Side::Sell, dom.idx(110).unwrap()), 3);
    }

    #[test]
    fn unlink_and_partial_fill_adjustments() {
        let dom = dom_100_200_5();
        let mut book = Book::new(dom);
        let mut arena = Arena::with_capacity(8);

        // two asks at same level
        let h1 = arena.alloc().unwrap();
        let pidx = dom.idx(110).unwrap();
        {
            let o = arena.get_mut(h1);
            o.side = Side::Sell;
            o.price_idx = pidx;
            o.qty_open = 5;
        }
        book.insert_tail(&mut arena, Side::Sell, h1, pidx, 5);

        let h2 = arena.alloc().unwrap();
        {
            let o = arena.get_mut(h2);
            o.side = Side::Sell;
            o.price_idx = pidx;
            o.qty_open = 7;
            o.prev = H_NONE;
            o.next = H_NONE;
        }
        book.insert_tail(&mut arena, Side::Sell, h2, pidx, 7);

        assert_eq!(book.best_ask(), Some(pidx));
        assert_eq!(book.level_qty(Side::Sell, pidx), 12);

        // partial fill eldest by 3
        {
            let o = arena.get_mut(h1);
            o.qty_open -= 3; // h1 now has 2 qty
        }
        // Adjust level total to reflect the partial fill
        book.partial_fill(Side::Sell, pidx, 3);
        assert_eq!(book.level_qty(Side::Sell, pidx), 9); // 12 - 3

        // fully fill eldest → unlink
        // Note: in real usage, unlink would be called with the order's current qty_open
        // Here we simulate a full fill by unlinking with the remaining qty_open (2)
        book.unlink(&mut arena, Side::Sell, h1);
        assert_eq!(book.level_qty(Side::Sell, pidx), 7); // remaining order's qty
        // still non-empty, best_ask unchanged
        assert_eq!(book.best_ask(), Some(pidx));

        // fully fill second → unlink → level becomes empty
        // Note: in real usage, unlink would be called with the order's current qty_open
        // Here we simulate a full fill by unlinking with the remaining qty_open (7)
        book.unlink(&mut arena, Side::Sell, h2);
        assert_eq!(book.level_qty(Side::Sell, pidx), 0);
        // best_ask recomputes to next non-empty (none here)
        assert_eq!(book.best_ask(), None);
    }

    #[test]
    fn navigation_with_bitsets() {
        let dom = dom_100_200_5();
        let mut book = Book::new(dom);
        let mut arena = Arena::with_capacity(8);

        let p110 = dom.idx(110).unwrap();
        let p130 = dom.idx(130).unwrap();
        let p150 = dom.idx(150).unwrap();

        // mark sparse asks at 110 and 150; bids at 130
        for &(side, price, qty) in
            &[(Side::Sell, 110, 1), (Side::Sell, 150, 2), (Side::Buy, 130, 3)]
        {
            let h = arena.alloc().unwrap();
            let pidx = dom.idx(price).unwrap();
            let o = arena.get_mut(h);
            o.side = side;
            o.price_idx = pidx;
            o.qty_open = qty;
            book.insert_tail(&mut arena, side, h, pidx, qty);
        }

        assert_eq!(book.best_ask(), Some(p110));
        assert_eq!(book.best_bid(), Some(p130));
        assert_eq!(book.next_ask_at_or_above(p110), Some(p110));
        assert_eq!(book.next_ask_at_or_above(p110 + 1), Some(p150));
        assert_eq!(book.prev_bid_at_or_below(p150), Some(p130));
    }
}
