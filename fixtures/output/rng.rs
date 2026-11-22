//! splitmix64: a tiny deterministic generator, so fixtures never depend on a crate version.

pub(crate) struct Rng(u64);

impl Rng {
    pub(crate) fn new(seed: u64) -> Self {
        Self(seed)
    }

    pub(crate) fn next_u64(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut mixed = self.0;
        mixed = (mixed ^ (mixed >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        mixed = (mixed ^ (mixed >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        mixed ^ (mixed >> 31)
    }

    /// Uniform in `0..bound` (the modulo bias is irrelevant for fixture mixing).
    pub(crate) fn below(&mut self, bound: usize) -> usize {
        let bound = u64::try_from(bound.max(1)).unwrap_or(1);
        usize::try_from(self.next_u64() % bound).unwrap_or(0)
    }

    /// Uniform in `low..=high`.
    pub(crate) fn between(&mut self, low: usize, high: usize) -> usize {
        low + self.below(high - low + 1)
    }

    /// True with probability `per_mille / 1000`.
    pub(crate) fn chance(&mut self, per_mille: usize) -> bool {
        self.below(1_000) < per_mille
    }

    pub(crate) fn pick<'a, T>(&mut self, items: &'a [T]) -> Option<&'a T> {
        items.get(self.below(items.len()))
    }
}
