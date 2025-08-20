// engine/whistle/src/bitset.rs
#![allow(dead_code)]

/// Word-packed bitset with fast forward/back scans. Length is in bits.
pub struct Bitset {
    pub(crate) words: Box<[u64]>,
    len_bits: usize,
}

impl Bitset {
    /// Allocate a zeroed bitset with `len_bits` bits.
    pub fn with_len(len_bits: usize) -> Self {
        assert!(len_bits > 0, "bitset must have at least 1 bit");
        let nwords = (len_bits + 63) >> 6;
        Self { words: vec![0u64; nwords].into_boxed_slice(), len_bits }
    }

    #[inline]
    pub fn len_bits(&self) -> usize {
        self.len_bits
    }
    #[inline]
    fn check(&self, i: usize) {
        assert!(i < self.len_bits, "bit index {i} OOB");
    }
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.words.iter().all(|&w| w == 0)
    }

    #[inline]
    pub fn set(&mut self, i: usize) {
        self.check(i);
        let w = i >> 6;
        let b = i & 63;
        self.words[w] |= 1u64 << b;
    }
    #[inline]
    pub fn clear(&mut self, i: usize) {
        self.check(i);
        let w = i >> 6;
        let b = i & 63;
        self.words[w] &= !(1u64 << b);
    }
    #[inline]
    pub fn get(&self, i: usize) -> bool {
        self.check(i);
        let w = i >> 6;
        let b = i & 63;
        ((self.words[w] >> b) & 1) != 0
    }

    /// Next set bit at or after `i`.
    pub fn next_one_at_or_after(&self, i: usize) -> Option<usize> {
        if i >= self.len_bits {
            return None;
        }
        let mut w = i >> 6;
        let off = i & 63;
        let bits = self.words[w] & (!0u64 << off);
        if bits != 0 {
            let idx = (w << 6) + bits.trailing_zeros() as usize;
            return (idx < self.len_bits).then_some(idx);
        }
        w += 1;
        while w < self.words.len() {
            let v = self.words[w];
            if v != 0 {
                let idx = (w << 6) + v.trailing_zeros() as usize;
                return (idx < self.len_bits).then_some(idx);
            }
            w += 1;
        }
        None
    }

    /// Previous set bit at or before `i` (clamps if i >= len).
    pub fn prev_one_at_or_before(&self, i: usize) -> Option<usize> {
        if self.len_bits == 0 {
            return None;
        }
        let t = if i >= self.len_bits { self.len_bits - 1 } else { i };
        let mut w = t >> 6;
        let off = t & 63;
        let mask = if off == 63 { !0u64 } else { (!0u64) >> (63 - off) };
        let bits = self.words[w] & mask;
        if bits != 0 {
            let bitpos = 63 - bits.leading_zeros() as usize;
            return Some((w << 6) + bitpos);
        }
        while w > 0 {
            w -= 1;
            let v = self.words[w];
            if v != 0 {
                let bitpos = 63 - v.leading_zeros() as usize;
                return Some((w << 6) + bitpos);
            }
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn set_get_clear_across_boundaries() {
        let mut bs = Bitset::with_len(130);
        assert!(bs.is_empty());
        for &i in &[0usize, 63, 64, 65, 127, 128, 129] {
            bs.set(i);
            assert!(bs.get(i));
        }
        for &i in &[0usize, 63, 64, 65, 127, 128, 129] {
            bs.clear(i);
            assert!(!bs.get(i));
        }
        assert!(bs.is_empty());
    }

    #[test]
    fn next_one_at_or_after_works() {
        let mut bs = Bitset::with_len(130);
        for &i in &[3usize, 63, 64, 90, 129] {
            bs.set(i);
        }
        assert_eq!(bs.next_one_at_or_after(0), Some(3));
        assert_eq!(bs.next_one_at_or_after(3), Some(3));
        assert_eq!(bs.next_one_at_or_after(4), Some(63));
        assert_eq!(bs.next_one_at_or_after(65), Some(90));
        assert_eq!(bs.next_one_at_or_after(128), Some(129));
        assert_eq!(bs.next_one_at_or_after(130), None);
    }

    #[test]
    fn prev_one_at_or_before_works() {
        let mut bs = Bitset::with_len(130);
        for &i in &[3usize, 63, 64, 90, 129] {
            bs.set(i);
        }
        assert_eq!(bs.prev_one_at_or_before(0), None);
        assert_eq!(bs.prev_one_at_or_before(4), Some(3));
        assert_eq!(bs.prev_one_at_or_before(63), Some(63));
        assert_eq!(bs.prev_one_at_or_before(65), Some(64));
        assert_eq!(bs.prev_one_at_or_before(200), Some(129));
    }
}
