use std::collections::HashMap;
use crate::domain::feather::{FeatherId, FeatherDef, Tier};
use crate::domain::stats::{StatVec, StatVecExt};

/// Dense lookup table: feather → tier → StatVec and costs.
#[derive(Clone)]
pub struct FeatherTable {
    /// Indexed by FeatherId (via HashMap for flexibility).
    defs: HashMap<FeatherId, FeatherDef>,
}

impl FeatherTable {
    pub fn new(defs: Vec<FeatherDef>) -> Self {
        let map = defs.into_iter().map(|d| (d.id, d)).collect();
        FeatherTable { defs: map }
    }

    pub fn get(&self, id: FeatherId) -> &FeatherDef {
        &self.defs[&id]
    }

    pub fn all(&self) -> impl Iterator<Item = &FeatherDef> {
        self.defs.values()
    }

    pub fn stats_at(&self, id: FeatherId, tier: Tier) -> &StatVec {
        self.defs[&id].stats_at(tier)
    }

    pub fn t1_cost_at(&self, id: FeatherId, tier: Tier) -> u64 {
        self.defs[&id].t1_cost_at(tier)
    }
}
