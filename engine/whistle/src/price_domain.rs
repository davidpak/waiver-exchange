pub type Price = u32;
pub type PriceIdx = u32;

#[derive(Clone, Copy, Debug)]
pub struct PriceDomain {
    pub floor: Price,
    pub ceil: Price,
    pub tick: Price,
}

impl PriceDomain {
    #[inline]
    pub fn idx(&self, p: Price) -> Option<PriceIdx> {
        if p < self.floor || p > self.ceil {
            return None;
        }
        let d = p - self.floor;
        if d % self.tick != 0 {
            return None;
        }
        Some(d / self.tick)
    }
    #[inline]
    pub fn price(&self, i: PriceIdx) -> Price {
        self.floor + i * self.tick
    }
    #[inline]
    pub fn ladder_len(&self) -> usize {
        ((self.ceil - self.floor) / self.tick) as usize + 1
    }
}
