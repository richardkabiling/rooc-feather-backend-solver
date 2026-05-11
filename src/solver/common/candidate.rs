use crate::domain::feather::{FeatherId, Set, Tier, eligible_orange, eligible_purple};
use crate::domain::statue::{Slot, Statue, StatueKind};
use crate::domain::inventory::Inventory;
use crate::eval::evaluator::Evaluator;

/// A mutable candidate solution with incremental scoring.
pub struct Candidate {
    pub statues:       [Statue; 10],
    pub inv:           Inventory,
    pub statue_scores: [f64; 10],
    pub total_score:   f64,
}

impl Candidate {
    pub fn new(statues: [Statue; 10], inv: Inventory, eval: &Evaluator) -> Self {
        let mut statue_scores = [0.0_f64; 10];
        let mut total = 0.0;
        for (i, s) in statues.iter().enumerate() {
            statue_scores[i] = eval.statue_score(s);
            total += statue_scores[i];
        }
        Candidate { statues, inv, statue_scores, total_score: total }
    }

    /// Apply a change to statue `si`, returning the old state for rollback.
    pub fn apply_statue(&mut self, si: usize, new_statue: Statue, eval: &Evaluator) -> (Statue, f64) {
        let old_statue = self.statues[si].clone();
        let old_score  = self.statue_scores[si];
        let new_score  = eval.statue_score(&new_statue);
        self.statues[si] = new_statue;
        self.statue_scores[si] = new_score;
        self.total_score += new_score - old_score;
        (old_statue, old_score)
    }

    pub fn rollback_statue(&mut self, si: usize, old_statue: Statue, old_score: f64) {
        let cur_score = self.statue_scores[si];
        self.statues[si] = old_statue;
        self.statue_scores[si] = old_score;
        self.total_score += old_score - cur_score;
    }
}
