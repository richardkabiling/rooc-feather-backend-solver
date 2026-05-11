use crate::domain::stats::{StatVec, StatVecExt, STAT_COUNT};
use crate::eval::feather_table::FeatherTable;
use crate::domain::feather::Tier;

pub type NormFactors = StatVec;

/// Compute per-stat normalization factors from the feather table (T20 max per feather).
pub fn compute_norm_factors(table: &FeatherTable) -> NormFactors {
    let mut norm = StatVec::zero();
    for def in table.all() {
        let t20 = def.stats_at(Tier::MAX);
        for i in 0..STAT_COUNT {
            if t20[i] > norm[i] { norm[i] = t20[i]; }
        }
    }
    // Guard: stats absent from all feathers → 1.0 to avoid div-by-zero
    for i in 0..STAT_COUNT {
        if norm[i] == 0.0 { norm[i] = 1.0; }
    }
    norm
}

/// Precompute effective weights = preset_weights / norm_factors.
pub fn effective_weights(weights: &StatVec, norm: &NormFactors) -> StatVec {
    let mut ew = *weights;
    for i in 0..STAT_COUNT {
        ew[i] /= norm[i];
    }
    ew
}
