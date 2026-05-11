use std::time::Instant;
use std::sync::{Arc, atomic::{AtomicBool, Ordering}};

use rand::{Rng, SeedableRng};
use rand_xoshiro::Xoshiro256PlusPlus;
use rayon::prelude::*;

use crate::domain::feather::{FeatherId, Tier, eligible_orange, eligible_purple};
use crate::domain::statue::{Slot, Statue, StatueKind};
use crate::domain::inventory::Inventory;
use crate::domain::solution::Solution;
use crate::eval::evaluator::Evaluator;
use crate::solver::{Solver, SolveContext, SolverConfig, SolverEvent, ProgressTx};
use crate::solver::common::candidate::Candidate;
use crate::solver::common::repair::greedy_consume;

pub struct SimulatedAnnealing;

impl Solver for SimulatedAnnealing {
    fn name(&self) -> &str { "Simulated Annealing" }

    fn solve(&self, ctx: &SolveContext, tx: ProgressTx) -> Solution {
        let n_restarts = ctx.config.restarts;
        let n_threads  = ctx.config.threads;
        let time_limit = ctx.config.time_budget_secs;
        let base_seed  = ctx.config.seed;
        let cancel     = ctx.cancel.clone();

        let start = Instant::now();

        // Run chains in parallel
        let results: Vec<Option<Solution>> = (0..n_restarts).into_par_iter().map(|i| {
            if cancel.load(Ordering::Relaxed) { return None; }
            let seed = base_seed.wrapping_add(i as u64 * 0x9e3779b97f4a7c15);
            let mut rng = Xoshiro256PlusPlus::seed_from_u64(seed);
            let inv = ctx.inventory.clone();
            run_chain(ctx, inv, &mut rng, start, time_limit, &cancel, &tx)
        }).collect();

        // Pick best
        let best = results.into_iter().flatten()
            .max_by(|a, b| a.objective.partial_cmp(&b.objective).unwrap());

        let sol = best.unwrap_or_else(|| {
            // Fallback: greedy solution
            make_greedy_solution(ctx)
        });

        let _ = tx.send(SolverEvent::Done(Box::new(sol.clone())));
        sol
    }
}

fn make_greedy_solution(ctx: &SolveContext) -> Solution {
    let mut inv = ctx.inventory.clone();
    let mut statues = build_initial_statues(ctx, &mut inv, &mut Xoshiro256PlusPlus::seed_from_u64(ctx.config.seed));
    greedy_consume(&mut statues, &mut inv, ctx.eval);
    let objective = ctx.eval.solution_score(&statues);
    let statue_scores = std::array::from_fn(|i| ctx.eval.statue_score(&statues[i]));
    Solution { statues, objective, statue_scores }
}

fn run_chain(
    ctx:        &SolveContext,
    mut inv:    Inventory,
    rng:        &mut Xoshiro256PlusPlus,
    start:      Instant,
    time_limit: u64,
    cancel:     &Arc<AtomicBool>,
    tx:         &ProgressTx,
) -> Option<Solution> {
    let mut statues = build_initial_statues(ctx, &mut inv, rng);
    greedy_consume(&mut statues, &mut inv, ctx.eval);
    let mut cand = Candidate::new(statues, inv, ctx.eval);

    let mut best_statues = cand.statues.clone();
    let mut best_score   = cand.total_score;

    // Calibrate temperature: sample 100 random moves, take mean |delta|
    let mut deltas = Vec::with_capacity(100);
    for _ in 0..100 {
        if let Some(delta) = random_move_delta(ctx, &mut cand, rng) {
            deltas.push(delta.abs());
        }
    }
    let mean_delta: f64 = if deltas.is_empty() { 1.0 } else {
        deltas.iter().sum::<f64>() / deltas.len() as f64
    };
    let t0 = mean_delta.max(1e-9);
    let cooling = 0.9995_f64;
    let mut temp = t0;
    let mut iter = 0u64;

    while !cancel.load(Ordering::Relaxed) {
        if start.elapsed().as_secs() >= time_limit { break; }

        // Save snapshot for potential rollback
        let snap_inv    = cand.inv.clone();
        let snap_statues: [Statue; 10] = cand.statues.clone();
        let snap_scores:  [f64; 10]    = cand.statue_scores;
        let snap_total                 = cand.total_score;

        let delta = random_move_delta(ctx, &mut cand, rng).unwrap_or(0.0);
        let accept = if delta >= 0.0 { true } else {
            let p = (delta / temp).exp();
            rng.gen::<f64>() < p
        };

        if !accept {
            // Rollback
            cand.statues      = snap_statues;
            cand.inv          = snap_inv;
            cand.statue_scores = snap_scores;
            cand.total_score  = snap_total;
        }

        temp *= cooling;
        iter += 1;

        if cand.total_score > best_score {
            best_score  = cand.total_score;
            best_statues = cand.statues.clone();
            let s = Solution {
                statues: best_statues.clone(),
                objective: best_score,
                statue_scores: std::array::from_fn(|i| cand.statue_scores[i]),
            };
            let _ = tx.try_send(SolverEvent::NewBest(Box::new(s)));
        }

        if iter % 1000 == 0 {
            let _ = tx.try_send(SolverEvent::Progress {
                iter,
                best_obj: best_score,
                budget_used: cand.inv.budget,
            });
        }
    }

    let statue_scores = std::array::from_fn(|i| ctx.eval.statue_score(&best_statues[i]));
    Some(Solution { statues: best_statues, objective: best_score, statue_scores })
}

/// Apply a random neighbor move in-place (Metropolis acceptance handled outside).
/// Returns net score delta (post-move score - pre-move score).
fn random_move_delta(ctx: &SolveContext, cand: &mut Candidate, rng: &mut Xoshiro256PlusPlus) -> Option<f64> {
    let move_type = rng.gen_range(0..5usize);
    let si = rng.gen_range(0..10usize);
    let statue = &cand.statues[si];

    match move_type {
        // Swap one orange slot feather for another eligible orange
        0 => {
            let orange_eligibles = eligible_orange(statue.kind);
            if orange_eligibles.len() < 2 { return None; }
            let slot_idx = rng.gen_range(1..5usize); // slots 1-4 are orange
            let cur_feather = statue.slots[slot_idx].feather;
            // pick a different one
            let candidates: Vec<_> = orange_eligibles.iter().copied()
                .filter(|&f| f != cur_feather)
                .collect();
            if candidates.is_empty() { return None; }
            let new_feather = candidates[rng.gen_range(0..candidates.len())];
            let tier = statue.slots[slot_idx].tier;

            // Check inventory: return old feather cost, consume new feather cost
            let old_def = ctx.eval.feather_table.get(cur_feather);
            let new_def = ctx.eval.feather_table.get(new_feather);
            let old_cost = old_def.t1_cost_at(tier);
            let new_cost = new_def.t1_cost_at(tier);

            // They must be in the same set for a clean swap; if different sets, adjust budgets
            let old_set = old_def.set;
            let new_set = new_def.set;

            // Check feasibility
            if old_set != new_set {
                if cand.inv.get(new_set) < new_cost { return None; }
            } else if new_cost > old_cost && cand.inv.get(new_set) < (new_cost - old_cost) { return None; }

            let before = cand.statue_scores[si];
            let mut new_statue = cand.statues[si].clone();
            new_statue.slots[slot_idx] = Slot::new(new_feather, tier);

            // Adjust inventory
            cand.inv.restore(old_set, old_cost);
            if cand.inv.consume(new_set, new_cost).is_err() {
                cand.inv.consume(old_set, old_cost).ok();
                return None;
            }

            let (old_s, old_sc) = cand.apply_statue(si, new_statue, ctx.eval);
            let delta = cand.total_score - (cand.total_score - cand.statue_scores[si] + old_sc);
            // Metropolis: always accept here (caller decides)
            Some(cand.statue_scores[si] - old_sc)
        }

        // ±1 tier on a single slot
        2 => {
            let slot_idx = rng.gen_range(0..5usize);
            let slot = cand.statues[si].slots[slot_idx];
            let up = rng.gen_bool(0.5);

            let (new_tier, cost_delta_i64) = if up {
                if let Some(nt) = slot.tier.next() {
                    let cost = ctx.eval.feather_table.get(slot.feather).upgrade_cost_from(slot.tier);
                    if cost == 0 { return None; }
                    let set = ctx.eval.feather_table.get(slot.feather).set;
                    if cand.inv.get(set) < cost { return None; }
                    (nt, cost as i64)
                } else { return None; }
            } else {
                if let Some(nt) = slot.tier.prev() {
                    let cost = ctx.eval.feather_table.get(slot.feather).upgrade_cost_from(nt);
                    (nt, -(cost as i64))
                } else { return None; }
            };

            let set = ctx.eval.feather_table.get(slot.feather).set;
            let mut new_statue = cand.statues[si].clone();
            new_statue.slots[slot_idx].tier = new_tier;

            if cost_delta_i64 > 0 {
                cand.inv.consume(set, cost_delta_i64 as u64).ok();
            } else {
                cand.inv.restore(set, (-cost_delta_i64) as u64);
            }

            let old_sc = cand.statue_scores[si];
            cand.apply_statue(si, new_statue, ctx.eval);
            Some(cand.statue_scores[si] - old_sc)
        }

        // Tier-min lift: +1 on the statue's min-tier slot
        4 => {
            let min_tier = cand.statues[si].min_tier();
            let slot_idx = cand.statues[si].slots.iter().position(|s| s.tier == min_tier)?;
            let slot = cand.statues[si].slots[slot_idx];
            let next_tier = slot.tier.next()?;
            let cost = ctx.eval.feather_table.get(slot.feather).upgrade_cost_from(slot.tier);
            if cost == 0 { return None; }
            let set = ctx.eval.feather_table.get(slot.feather).set;
            if cand.inv.get(set) < cost { return None; }
            cand.inv.consume(set, cost).ok();
            let mut new_statue = cand.statues[si].clone();
            new_statue.slots[slot_idx].tier = next_tier;
            let old_sc = cand.statue_scores[si];
            cand.apply_statue(si, new_statue, ctx.eval);
            Some(cand.statue_scores[si] - old_sc)
        }

        _ => None,
    }
}

fn build_initial_statues(ctx: &SolveContext, inv: &mut Inventory, rng: &mut Xoshiro256PlusPlus) -> [Statue; 10] {
    let mut statues: Vec<Statue> = Vec::with_capacity(10);

    // 5 attack, 5 defense
    for kind_idx in 0..2 {
        let kind = if kind_idx == 0 { StatueKind::Attack } else { StatueKind::Defense };
        let purples = eligible_purple(kind);
        let oranges = eligible_orange(kind);

        for _ in 0..5 {
            // Pick a purple (slot 0)
            let purple_f = purples[rng.gen_range(0..purples.len())];
            // Pick 4 distinct oranges
            let mut chosen_oranges: Vec<FeatherId> = Vec::new();
            let mut available: Vec<FeatherId> = oranges.to_vec();
            while chosen_oranges.len() < 4 && !available.is_empty() {
                let idx = rng.gen_range(0..available.len());
                chosen_oranges.push(available.remove(idx));
            }
            while chosen_oranges.len() < 4 {
                chosen_oranges.push(oranges[0]);
            }

            let slots = [
                Slot::new(purple_f, Tier::MIN),
                Slot::new(chosen_oranges[0], Tier::MIN),
                Slot::new(chosen_oranges[1], Tier::MIN),
                Slot::new(chosen_oranges[2], Tier::MIN),
                Slot::new(chosen_oranges[3], Tier::MIN),
            ];
            statues.push(Statue { kind, slots });
        }
    }

    std::array::from_fn(|i| statues[i].clone())
}
