use std::collections::HashMap;
use crate::domain::stats::{StatVec, StatVecExt, parse_stat_id, StatId};

/// A scoring preset: name + weight vector.
#[derive(Debug, Clone)]
pub struct Preset {
    pub name:    String,
    pub weights: StatVec,
}

impl Preset {
    pub fn from_rows(name: String, rows: &[(String, f64)]) -> Self {
        let mut weights = StatVec::zero();
        for (stat_name, w) in rows {
            if let Some(id) = parse_stat_id(stat_name) {
                weights[id as usize] = *w;
            }
        }
        Preset { name, weights }
    }
}
