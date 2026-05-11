use crate::domain::feather::{FeatherId, Tier};
use crate::domain::stats::StatVec;

/// One assigned slot in a statue.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Slot {
    pub feather: FeatherId,
    pub tier:    Tier,
}

impl Slot {
    pub fn new(feather: FeatherId, tier: Tier) -> Self { Slot { feather, tier } }
}

/// Kind of statue.
pub use crate::domain::feather::StatueKind;

/// A fully configured statue (5 slots: slot 0 = purple, slots 1-4 = orange).
#[derive(Debug, Clone)]
pub struct Statue {
    pub kind:  StatueKind,
    pub slots: [Slot; 5],
}

impl Statue {
    pub fn min_tier(&self) -> Tier {
        self.slots.iter().map(|s| s.tier).min().unwrap()
    }
}
