use crate::domain::feather::{FeatherId, Set, SET_COUNT, Tier};
use anyhow::{bail, Result};

/// Per-set T1-equivalent budget.
#[derive(Debug, Clone)]
pub struct Inventory {
    pub budget: [u64; SET_COUNT],
}

impl Inventory {
    pub fn zero() -> Self { Inventory { budget: [0; SET_COUNT] } }

    pub fn add(&mut self, set: Set, amount: u64) {
        self.budget[set as usize] += amount;
    }

    pub fn get(&self, set: Set) -> u64 {
        self.budget[set as usize]
    }

    pub fn consume(&mut self, set: Set, amount: u64) -> Result<()> {
        let b = &mut self.budget[set as usize];
        if *b < amount {
            bail!("insufficient budget in {:?}: need {}, have {}", set, amount, b);
        }
        *b -= amount;
        Ok(())
    }

    pub fn restore(&mut self, set: Set, amount: u64) {
        self.budget[set as usize] += amount;
    }
}
