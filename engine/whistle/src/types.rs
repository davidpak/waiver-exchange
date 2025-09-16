#[allow(dead_code)]
use core::fmt;

pub type OrderId = u64;
pub type AccountId = u64;
pub type Qty = u64;
pub type TsNorm = u64;
pub type EnqSeq = u32;

#[repr(u8)]
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub enum Side {
    Buy = 0,
    Sell = 1,
}

impl Side {
    /// Get the opposite side
    pub fn opposite(&self) -> Side {
        match self {
            Side::Buy => Side::Sell,
            Side::Sell => Side::Buy,
        }
    }
}

#[repr(u8)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OrderType {
    Limit = 0,
    Market = 1,
    Ioc = 2,
    PostOnly = 3,
}

#[repr(transparent)]
#[derive(Clone, Copy, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct OrderHandle(pub u32);
pub const H_NONE: OrderHandle = OrderHandle(u32::MAX);

impl fmt::Debug for OrderHandle {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if *self == H_NONE { write!(f, "H_NONE") } else { write!(f, "H({})", self.0) }
    }
}
