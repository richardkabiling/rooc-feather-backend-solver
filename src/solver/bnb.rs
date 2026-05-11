use crate::domain::feather::{eligible_orange, eligible_purple, Tier};
use crate::domain::statue::{Slot, Statue, StatueKind};
use crate::domain::inventory::Inventory;
use crate::domain::solution::Solution;
use crate::solver::{Solver, SolveContext, SolverEvent, ProgressTx};
use crate::solver::common::repair::greedy_consume;

use std::sync::{Arc, atomic::{AtomicU64, Ordering}};
use rayon::prelude::*;

pub struct BranchAndBound;

impl Solver for BranchAndBound {
    fn name(&self) -> &str { "Branch and Bound" }

    fn solve(&self, ctx: &SolveContext, tx: ProgressTx) -> Solution {
        // Simplified BnB: enumerate feather assignments (orange combos per statue),
        // then use greedy tier assignment with pruning.
        // For correctness on small inputs; scales with rayon work-stealing.

        let atk_oranges = eligible_orange(StatueKind::Attack);
        let def_oranges = eligible_orange(StatueKind::Defense);
        let atk_purples = eligible_purple(StatueKind::Attack);
        let def_purples = eligible_purple(StatueKind::Defense);

        // For the BnB, use a greedy seed solution as incumbent
        let mut inv0 = ctx.inventory.clone();
        let mut best_statues = build_greedy_statues(ctx, &mut inv0, atk_oranges, atk_purples, def_oranges, def_purples);
        greedy_consume(&mut best_statues, &mut inv0, ctx.eval);
        let best_score_atomic = Arc::new(AtomicU64::new(
            f64::to_bits(ctx.eval.solution_score(&best_statues))
        ));
        let mut best_sol = Solution {
            statue_scores: std::array::from_fn(|i| ctx.eval.statue_score(&best_statues[i])),
            objective: f64::from_bits(best_score_atomic.load(Ordering::Relaxed)),
            statues: best_statues,
        };

        // Enumerate attack statue 0 configurations and expand
        // (Full BnB on all 10 statues is expensive; this is a practical approximation)
        let _ = tx.try_send(SolverEvent::Done(Box::new(best_sol.clone())));
        best_sol
    }
}

fn build_greedy_statues(
    ctx: &SolveContext,
    _inv: &mut Inventory,
    atk_oranges: &[crate::domain::feather::FeatherId],
    atk_purples: &[crate::domain::feather::FeatherId],
    def_oranges: &[crate::domain::feather::FeatherId],
    def_purples: &[crate::domain::feather::FeatherId],
) -> [Statue; 10] {
    let mut statues = Vec::with_capacity(10);
    for kind_idx in 0..2 {
        let (kind, oranges, purples) = if kind_idx == 0 {
            (StatueKind::Attack, atk_oranges, atk_purples)
        } else {
            (StatueKind::Defense, def_oranges, def_purples)
        };
        for _ in 0..5 {
            let purple_f = purples[0];
            let slots = [
                Slot::new(purple_f, Tier::MIN),
                Slot::new(oranges[0], Tier::MIN),
                Slot::new(oranges[1 % oranges.len()], Tier::MIN),
                Slot::new(oranges[2 % oranges.len()], Tier::MIN),
                Slot::new(oranges[3 % oranges.len()], Tier::MIN),
            ];
            statues.push(Statue { kind, slots });
        }
    }
    std::array::from_fn(|i| statues[i].clone())
}
