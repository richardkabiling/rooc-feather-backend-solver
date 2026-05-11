use std::sync::{Arc, atomic::{AtomicBool, Ordering}};
use crossbeam_channel::Sender;

use crate::data::GameData;
use crate::domain::inventory::Inventory;
use crate::domain::preset::Preset;
use crate::domain::solution::Solution;
use crate::eval::evaluator::Evaluator;

pub mod common;
pub mod sa;
pub mod bnb;

pub type ProgressTx = Sender<SolverEvent>;

#[derive(Debug, Clone)]
pub enum SolverEvent {
    Progress { chain: usize, iter: u64, best_obj: f64, iters_since_best: u64, budget_used: [u64; 5] },
    NewBest(usize, Box<Solution>),
    Done(Box<Solution>),
}

#[derive(Debug, Clone)]
pub struct SolverConfig {
    pub time_budget_secs: u64,
    pub seed:             u64,
    pub restarts:         usize,
    pub threads:          usize,
    pub log_every:        u64,
    /// How often (in SA iterations per chain) to share the best solution across chains.
    pub share_interval:   u64,
}

impl Default for SolverConfig {
    fn default() -> Self {
        SolverConfig {
            time_budget_secs: 30,
            seed:             42,
            restarts:         8,
            threads:          4,
            log_every:        10_000,
            share_interval:   50_000,
        }
    }
}

pub struct SolveContext<'a> {
    pub game:      &'a GameData,
    pub eval:      &'a Evaluator,
    pub preset:    &'a Preset,
    pub inventory: Inventory,
    pub config:    SolverConfig,
    pub cancel:    Arc<AtomicBool>,
}

pub trait Solver: Send + Sync {
    fn name(&self) -> &str;
    fn solve(&self, ctx: &SolveContext, tx: ProgressTx) -> Solution;
}
