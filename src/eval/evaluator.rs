use crate::domain::feather::Tier;
use crate::domain::stats::{StatVec, StatVecExt, STAT_COUNT};
use crate::domain::statue::{Statue, StatueKind};
use crate::eval::feather_table::FeatherTable;
use crate::eval::set_bonus_table::{AttackSetBonus, DefenseSetBonus};

/// The hot-path evaluator. Holds precomputed tables and effective weights.
pub struct Evaluator {
    pub feather_table:   FeatherTable,
    pub attack_bonuses:  [AttackSetBonus; 20],
    pub defense_bonuses: [DefenseSetBonus; 20],
    /// Normalized preset weights (preset.weights[i] / norm[i]).
    pub eff_weights:     StatVec,
}

impl Evaluator {
    /// Compute score for a single statue.
    pub fn statue_score(&self, statue: &Statue) -> f64 {
        // Step 1: sum feather StatVecs
        let mut raw = StatVec::zero();
        for slot in &statue.slots {
            let sv = self.feather_table.stats_at(slot.feather, slot.tier);
            for i in 0..STAT_COUNT {
                raw[i] += sv[i];
            }
        }

        // Step 2: find min tier
        let min_tier_idx = statue.min_tier().get() as usize - 1;

        // Step 3: lookup bonus
        let (pct, flat) = match statue.kind {
            StatueKind::Attack => {
                let b = &self.attack_bonuses[min_tier_idx];
                (&b.pct, &b.flat)
            }
            StatueKind::Defense => {
                let b = &self.defense_bonuses[min_tier_idx];
                (&b.pct, &b.flat)
            }
        };

        // Step 4: scale then add flat
        let mut final_stats = StatVec::zero();
        for i in 0..STAT_COUNT {
            final_stats[i] = raw[i] * (1.0 + pct[i] / 100.0) + flat[i];
        }

        // Step 5: dot with effective weights
        let mut score = 0.0_f64;
        for i in 0..STAT_COUNT {
            score += final_stats[i] * self.eff_weights[i];
        }
        score
    }

    /// Compute total score over all 10 statues.
    pub fn solution_score(&self, statues: &[Statue; 10]) -> f64 {
        statues.iter().map(|s| self.statue_score(s)).sum()
    }
}
