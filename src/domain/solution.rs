use crate::domain::statue::Statue;
use crate::domain::stats::StatVec;
use serde::{Deserialize, Serialize};

/// Final solution: 10 statues (5 attack, 5 defense), objective score, and per-statue scores.
#[derive(Debug, Clone)]
pub struct Solution {
    /// Statues: indices 0-4 are Attack, 5-9 are Defense.
    pub statues:        [Statue; 10],
    pub objective:      f64,
    pub statue_scores:  [f64; 10],
}
