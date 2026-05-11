use std::time::Instant;
use std::sync::{Arc, Mutex, atomic::{AtomicBool, Ordering}};

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

        // Shared best solution pool for cross-chain migration.
        let shared_best: Arc<Mutex<Option<([crate::domain::statue::Statue; 10], f64)>>> =
            Arc::new(Mutex::new(None));

        // Run chains in parallel
        let results: Vec<Option<Solution>> = (0..n_restarts).into_par_iter().map(|i| {
            if cancel.load(Ordering::Relaxed) { return None; }
            let seed = base_seed.wrapping_add(i as u64 * 0x9e3779b97f4a7c15);
            let mut rng = Xoshiro256PlusPlus::seed_from_u64(seed);
            let inv = ctx.inventory.clone();
            let orig_inv = ctx.inventory.clone();
            run_chain(i, ctx, inv, orig_inv, &mut rng, start, time_limit, &cancel, &tx, &shared_best)
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
    let mut rng = Xoshiro256PlusPlus::seed_from_u64(ctx.config.seed);
    let mut statues = build_initial_statues(ctx, &mut inv, &mut rng);
    greedy_consume(&mut statues, &mut inv, ctx.eval);
    let objective = ctx.eval.solution_score(&statues);
    let statue_scores = std::array::from_fn(|i| ctx.eval.statue_score(&statues[i]));
    Solution { statues, objective, statue_scores }
}

fn run_chain(
    chain:      usize,
    ctx:        &SolveContext,
    mut inv:    Inventory,
    orig_inv:   Inventory,
    rng:        &mut Xoshiro256PlusPlus,
    start:      Instant,
    time_limit: u64,
    cancel:     &Arc<AtomicBool>,
    tx:         &ProgressTx,
    shared_best: &Arc<Mutex<Option<([crate::domain::statue::Statue; 10], f64)>>>,
) -> Option<Solution> {
    let mut statues = build_initial_statues(ctx, &mut inv, rng);
    // Consume the base T1 placement cost for every slot.
    // t1_cost_at(T1) = total_cost(T1) = 1 for all feathers. All swap moves restore
    // t1_cost_at(tier), so if we fail to deduct here we inject phantom budget per slot.
    for statue in &statues {
        for slot in &statue.slots {
            let def = ctx.eval.feather_table.get(slot.feather);
            let _ = inv.consume(def.set, def.t1_cost_at(Tier::MIN));
        }
    }
    greedy_consume(&mut statues, &mut inv, ctx.eval);
    let mut cand = Candidate::new(statues, inv, ctx.eval);

    let mut best_statues = cand.statues.clone();
    let mut best_score   = cand.total_score;
    let mut iters_since_best = 0u64;

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
        iters_since_best += 1;

        if cand.total_score > best_score {
            best_score  = cand.total_score;
            best_statues = cand.statues.clone();
            iters_since_best = 0;
            let s = Solution {
                statues: best_statues.clone(),
                objective: best_score,
                statue_scores: std::array::from_fn(|i| cand.statue_scores[i]),
            };
            let _ = tx.try_send(SolverEvent::NewBest(chain, Box::new(s)));
        }

        if iter % 1000 == 0 {
            let _ = tx.try_send(SolverEvent::Progress {
                chain,
                iter,
                best_obj: best_score,
                iters_since_best,
                budget_used: cand.inv.budget,
            });
        }

        // Cross-chain sharing: publish our best and optionally adopt a better one.
        let share_interval = ctx.config.share_interval.max(1);
        if iter % share_interval == 0 {
            let mut pool = shared_best.lock().unwrap();
            // Publish if we're the new global best.
            let pool_score = pool.as_ref().map(|(_, s)| *s).unwrap_or(f64::NEG_INFINITY);
            if best_score > pool_score {
                *pool = Some((best_statues.clone(), best_score));
            }
            // Adopt global best if it beats our local best.
            if let Some((ref global_statues, global_score)) = *pool {
                if global_score > best_score {
                    // Reconstruct inventory from orig_inv minus the cost of global_statues.
                    let mut new_inv = orig_inv.clone();
                    for statue in global_statues {
                        for slot in &statue.slots {
                            let def = ctx.eval.feather_table.get(slot.feather);
                            new_inv.budget[def.set as usize] =
                                new_inv.budget[def.set as usize].saturating_sub(def.t1_cost_at(slot.tier));
                        }
                    }
                    let adopted = global_statues.clone();
                    drop(pool);
                    best_statues = adopted.clone();
                    best_score   = global_score;
                    iters_since_best = 0;
                    cand = Candidate::new(adopted, new_inv, ctx.eval);
                    temp = t0; // re-heat so the chain can explore from this new base
                    eprintln!("[chain {:>2}] adopted global best {:.4} at iter {}", chain, global_score, iter);
                }
            }
        }
    }

    let statue_scores = std::array::from_fn(|i| ctx.eval.statue_score(&best_statues[i]));
    Some(Solution { statues: best_statues, objective: best_score, statue_scores })
}

/// Apply a random neighbor move in-place (Metropolis acceptance handled outside).
/// Returns net score delta (post-move score - pre-move score).
fn random_move_delta(ctx: &SolveContext, cand: &mut Candidate, rng: &mut Xoshiro256PlusPlus) -> Option<f64> {
    let move_type = rng.gen_range(0..10usize);
    let si = rng.gen_range(0..10usize);

    match move_type {
        // Swap one orange slot feather for another eligible orange (same tier).
        0 => {
            let statue = &cand.statues[si];
            let orange_eligibles = eligible_orange(statue.kind);
            if orange_eligibles.len() < 2 { return None; }
            let slot_idx = rng.gen_range(1..5usize); // slots 1-4 are orange
            let cur_feather = statue.slots[slot_idx].feather;
            let other_feathers: Vec<_> = statue.slots.iter().enumerate()
                .filter(|&(i, _)| i != slot_idx)
                .map(|(_, s)| s.feather)
                .collect();
            let candidates: Vec<_> = orange_eligibles.iter().copied()
                .filter(|&f| f != cur_feather && !other_feathers.contains(&f))
                .collect();
            if candidates.is_empty() { return None; }
            let new_feather = candidates[rng.gen_range(0..candidates.len())];
            let tier = statue.slots[slot_idx].tier;

            let old_def = ctx.eval.feather_table.get(cur_feather);
            let new_def = ctx.eval.feather_table.get(new_feather);
            let old_cost = old_def.t1_cost_at(tier);
            let new_cost = new_def.t1_cost_at(tier);
            let old_set = old_def.set;
            let new_set = new_def.set;

            if old_set != new_set {
                if cand.inv.get(new_set) < new_cost { return None; }
            } else if new_cost > old_cost && cand.inv.get(new_set) < (new_cost - old_cost) { return None; }

            let mut new_statue = cand.statues[si].clone();
            new_statue.slots[slot_idx] = Slot::new(new_feather, tier);

            cand.inv.restore(old_set, old_cost);
            if cand.inv.consume(new_set, new_cost).is_err() {
                cand.inv.consume(old_set, old_cost).ok();
                return None;
            }

            let (_, old_sc) = cand.apply_statue(si, new_statue, ctx.eval);
            Some(cand.statue_scores[si] - old_sc)
        }

        // Swap the purple slot feather for another eligible purple (same tier).
        1 => {
            let statue = &cand.statues[si];
            let purple_eligibles = eligible_purple(statue.kind);
            if purple_eligibles.len() < 2 { return None; }
            let cur_feather = statue.slots[0].feather;
            let candidates: Vec<_> = purple_eligibles.iter().copied()
                .filter(|&f| f != cur_feather)
                .collect();
            if candidates.is_empty() { return None; }
            let new_feather = candidates[rng.gen_range(0..candidates.len())];
            let tier = statue.slots[0].tier;

            let old_def = ctx.eval.feather_table.get(cur_feather);
            let new_def = ctx.eval.feather_table.get(new_feather);
            let old_cost = old_def.t1_cost_at(tier);
            let new_cost = new_def.t1_cost_at(tier);
            // Purple feathers are always in the Purple set, so sets are always equal.
            let set = old_def.set; // == new_def.set == Purple

            if new_cost > old_cost && cand.inv.get(set) < (new_cost - old_cost) { return None; }

            let mut new_statue = cand.statues[si].clone();
            new_statue.slots[0] = Slot::new(new_feather, tier);

            cand.inv.restore(set, old_cost);
            if cand.inv.consume(set, new_cost).is_err() {
                cand.inv.consume(set, old_cost).ok();
                return None;
            }

            let (_, old_sc) = cand.apply_statue(si, new_statue, ctx.eval);
            Some(cand.statue_scores[si] - old_sc)
        }

        // ±1 tier on a single slot.
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

        // Inter-statue budget transfer: downgrade a slot on si, upgrade a slot on sj.
        3 => {
            let sj = { let mut j = rng.gen_range(0..10usize); if j == si { j = (j + 1) % 10; } j };

            let down_slot_idx = rng.gen_range(0..5usize);
            let down_slot = cand.statues[si].slots[down_slot_idx];
            let down_tier = down_slot.tier.prev()?;
            let down_def  = ctx.eval.feather_table.get(down_slot.feather);
            let refund    = down_def.upgrade_cost_from(down_tier);
            let down_set  = down_def.set;

            let up_slot_idx = rng.gen_range(0..5usize);
            let up_slot = cand.statues[sj].slots[up_slot_idx];
            let up_tier = up_slot.tier.next()?;
            let up_def  = ctx.eval.feather_table.get(up_slot.feather);
            let cost    = up_def.upgrade_cost_from(up_slot.tier);
            if cost == 0 { return None; }
            let up_set  = up_def.set;

            let available = if down_set == up_set {
                cand.inv.get(up_set) + refund
            } else {
                cand.inv.restore(down_set, refund);
                let avail = cand.inv.get(up_set);
                cand.inv.consume(down_set, refund).ok();
                avail
            };
            if available < cost { return None; }

            cand.inv.restore(down_set, refund);
            if cand.inv.consume(up_set, cost).is_err() {
                cand.inv.consume(down_set, refund).ok();
                return None;
            }

            let old_sc_si = cand.statue_scores[si];
            let old_sc_sj = cand.statue_scores[sj];

            let mut new_si = cand.statues[si].clone();
            new_si.slots[down_slot_idx].tier = down_tier;
            let mut new_sj = cand.statues[sj].clone();
            new_sj.slots[up_slot_idx].tier = up_tier;

            cand.apply_statue(si, new_si, ctx.eval);
            cand.apply_statue(sj, new_sj, ctx.eval);

            Some((cand.statue_scores[si] - old_sc_si) + (cand.statue_scores[sj] - old_sc_sj))
        }

        // Orange swap + best affordable tier for the new feather.
        4 => {
            let statue = &cand.statues[si];
            let orange_eligibles = eligible_orange(statue.kind);
            if orange_eligibles.len() < 2 { return None; }
            let slot_idx = rng.gen_range(1..5usize);
            let cur_feather = statue.slots[slot_idx].feather;
            let other_feathers: Vec<_> = statue.slots.iter().enumerate()
                .filter(|&(i, _)| i != slot_idx)
                .map(|(_, s)| s.feather)
                .collect();
            let candidates: Vec<_> = orange_eligibles.iter().copied()
                .filter(|&f| f != cur_feather && !other_feathers.contains(&f))
                .collect();
            if candidates.is_empty() { return None; }
            let new_feather = candidates[rng.gen_range(0..candidates.len())];

            let old_def  = ctx.eval.feather_table.get(cur_feather);
            let new_def  = ctx.eval.feather_table.get(new_feather);
            let old_tier = statue.slots[slot_idx].tier;
            let old_set  = old_def.set;
            let new_set  = new_def.set;

            let old_cost = old_def.t1_cost_at(old_tier);
            cand.inv.restore(old_set, old_cost);

            let mut best_tier = Tier::MIN;
            for t in 1..=20u8 {
                let tier = Tier::new(t);
                if cand.inv.get(new_set) >= new_def.t1_cost_at(tier) { best_tier = tier; } else { break; }
            }

            let new_cost = new_def.t1_cost_at(best_tier);
            if cand.inv.consume(new_set, new_cost).is_err() {
                cand.inv.consume(old_set, old_cost).ok();
                return None;
            }

            let mut new_statue = cand.statues[si].clone();
            new_statue.slots[slot_idx] = Slot::new(new_feather, best_tier);

            let (_, old_sc) = cand.apply_statue(si, new_statue, ctx.eval);
            Some(cand.statue_scores[si] - old_sc)
        }

        // Tier-min lift: +1 on the statue's bottleneck slot (gates set bonus).
        5 => {
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

        // Intra-statue budget transfer: downgrade one slot, upgrade another within
        // the same statue. Helps level up all slots toward the same tier to maximise
        // the set bonus multiplier.
        6 => {
            let down_slot_idx = rng.gen_range(0..5usize);
            let up_slot_idx   = { let mut j = rng.gen_range(0..5usize); if j == down_slot_idx { j = (j + 1) % 5; } j };

            let down_slot = cand.statues[si].slots[down_slot_idx];
            let up_slot   = cand.statues[si].slots[up_slot_idx];

            let down_tier = down_slot.tier.prev()?;
            let up_tier   = up_slot.tier.next()?;

            let down_def = ctx.eval.feather_table.get(down_slot.feather);
            let up_def   = ctx.eval.feather_table.get(up_slot.feather);
            let refund   = down_def.upgrade_cost_from(down_tier);
            let cost     = up_def.upgrade_cost_from(up_slot.tier);
            if cost == 0 { return None; }
            let down_set = down_def.set;
            let up_set   = up_def.set;

            let available = if down_set == up_set {
                cand.inv.get(up_set) + refund
            } else {
                cand.inv.restore(down_set, refund);
                let avail = cand.inv.get(up_set);
                cand.inv.consume(down_set, refund).ok();
                avail
            };
            if available < cost { return None; }

            cand.inv.restore(down_set, refund);
            if cand.inv.consume(up_set, cost).is_err() {
                cand.inv.consume(down_set, refund).ok();
                return None;
            }

            let mut new_statue = cand.statues[si].clone();
            new_statue.slots[down_slot_idx].tier = down_tier;
            new_statue.slots[up_slot_idx].tier   = up_tier;

            let (_, old_sc) = cand.apply_statue(si, new_statue, ctx.eval);
            Some(cand.statue_scores[si] - old_sc)
        }

        // Purple swap + best affordable tier for the new purple feather.
        7 => {
            let statue = &cand.statues[si];
            let purple_eligibles = eligible_purple(statue.kind);
            if purple_eligibles.len() < 2 { return None; }
            let cur_feather = statue.slots[0].feather;
            let candidates: Vec<_> = purple_eligibles.iter().copied()
                .filter(|&f| f != cur_feather)
                .collect();
            if candidates.is_empty() { return None; }
            let new_feather = candidates[rng.gen_range(0..candidates.len())];

            let old_def = ctx.eval.feather_table.get(cur_feather);
            let new_def = ctx.eval.feather_table.get(new_feather);
            let old_cost = old_def.t1_cost_at(statue.slots[0].tier);
            let set = old_def.set; // always Purple

            cand.inv.restore(set, old_cost);

            let mut best_tier = Tier::MIN;
            for t in 1..=20u8 {
                let tier = Tier::new(t);
                if cand.inv.get(set) >= new_def.t1_cost_at(tier) { best_tier = tier; } else { break; }
            }

            let new_cost = new_def.t1_cost_at(best_tier);
            if cand.inv.consume(set, new_cost).is_err() {
                cand.inv.consume(set, old_cost).ok();
                return None;
            }

            let mut new_statue = cand.statues[si].clone();
            new_statue.slots[0] = Slot::new(new_feather, best_tier);

            let (_, old_sc) = cand.apply_statue(si, new_statue, ctx.eval);
            Some(cand.statue_scores[si] - old_sc)
        }

        // Large tier jump: ±N (N drawn from 2..=5) on a single slot.
        // Helps escape flat plateaus that ±1 steps cannot cross at low temperature.
        8 => {
            let slot_idx = rng.gen_range(0..5usize);
            let slot     = cand.statues[si].slots[slot_idx];
            let n        = rng.gen_range(2usize..=5);
            let up       = rng.gen_bool(0.5);
            let def      = ctx.eval.feather_table.get(slot.feather);
            let set      = def.set;

            if up {
                // Accumulate cost for n consecutive upgrades
                let mut cur = slot.tier;
                let mut total_cost = 0u64;
                for _ in 0..n {
                    let next = cur.next()?;
                    let c = def.upgrade_cost_from(cur);
                    if c == 0 { return None; }
                    total_cost += c;
                    cur = next;
                }
                if cand.inv.get(set) < total_cost { return None; }
                cand.inv.consume(set, total_cost).ok();
                let mut new_statue = cand.statues[si].clone();
                new_statue.slots[slot_idx].tier = cur;
                let old_sc = cand.statue_scores[si];
                cand.apply_statue(si, new_statue, ctx.eval);
                Some(cand.statue_scores[si] - old_sc)
            } else {
                let mut cur = slot.tier;
                let mut total_refund = 0u64;
                for _ in 0..n {
                    let prev = cur.prev()?;
                    total_refund += def.upgrade_cost_from(prev);
                    cur = prev;
                }
                cand.inv.restore(set, total_refund);
                let mut new_statue = cand.statues[si].clone();
                new_statue.slots[slot_idx].tier = cur;
                let old_sc = cand.statue_scores[si];
                cand.apply_statue(si, new_statue, ctx.eval);
                Some(cand.statue_scores[si] - old_sc)
            }
        }

        // Cross-statue feather swap: swap the same slot index between two statues of
        // the same kind, keeping their respective tiers. Reassigns which statue holds
        // a given feather without full randomisation.
        9 => {
            // Find a second statue of the same kind
            let kind = cand.statues[si].kind;
            let same_kind: Vec<usize> = (0..10)
                .filter(|&j| j != si && cand.statues[j].kind == kind)
                .collect();
            if same_kind.is_empty() { return None; }
            let sj = same_kind[rng.gen_range(0..same_kind.len())];

            let slot_idx = rng.gen_range(0..5usize);
            let f_i = cand.statues[si].slots[slot_idx].feather;
            let f_j = cand.statues[sj].slots[slot_idx].feather;
            if f_i == f_j { return None; }

            // Ensure the incoming feather isn't already in the destination statue's other slots.
            let si_has_fj = cand.statues[si].slots.iter().enumerate()
                .any(|(i, s)| i != slot_idx && s.feather == f_j);
            let sj_has_fi = cand.statues[sj].slots.iter().enumerate()
                .any(|(i, s)| i != slot_idx && s.feather == f_i);
            if si_has_fj || sj_has_fi { return None; }

            let t_i = cand.statues[si].slots[slot_idx].tier;
            let t_j = cand.statues[sj].slots[slot_idx].tier;

            let def_i = ctx.eval.feather_table.get(f_i);
            let def_j = ctx.eval.feather_table.get(f_j);
            let set_i = def_i.set;
            let set_j = def_j.set;

            // Cost of f_i at t_i and f_j at t_j (current); after swap: f_j at t_i, f_i at t_j
            let cost_fi_ti = def_i.t1_cost_at(t_i);
            let cost_fj_tj = def_j.t1_cost_at(t_j);
            let cost_fj_ti = def_j.t1_cost_at(t_i);
            let cost_fi_tj = def_i.t1_cost_at(t_j);

            // Simulate inventory adjustments and check feasibility
            // Refund current holdings, then charge new holdings
            // Work with i64 deltas per set to check net feasibility
            let mut delta = [0i64; 5];
            delta[set_i as usize] -= cost_fi_ti as i64; // refund f_i@t_i from si
            delta[set_i as usize] += cost_fi_tj as i64; // charge f_i@t_j to sj  (if set_i==set of f_i in sj)
            delta[set_j as usize] -= cost_fj_tj as i64; // refund f_j@t_j from sj
            delta[set_j as usize] += cost_fj_ti as i64; // charge f_j@t_i to si

            // Check no set goes negative.
            // delta[s] > 0 means net consume (inv decreases); new_bal = inv - delta[s].
            for s in 0..5 {
                use crate::domain::feather::Set;
                let set = match s {
                    0 => crate::domain::feather::Set::STDN,
                    1 => crate::domain::feather::Set::DN,
                    2 => crate::domain::feather::Set::ST,
                    3 => crate::domain::feather::Set::LD,
                    _ => crate::domain::feather::Set::Purple,
                };
                let new_bal = cand.inv.get(set) as i64 - delta[s];
                if new_bal < 0 { return None; }
            }

            // Apply inventory deltas
            for s in 0..5usize {
                use crate::domain::feather::Set;
                let set = match s {
                    0 => crate::domain::feather::Set::STDN,
                    1 => crate::domain::feather::Set::DN,
                    2 => crate::domain::feather::Set::ST,
                    3 => crate::domain::feather::Set::LD,
                    _ => crate::domain::feather::Set::Purple,
                };
                if delta[s] > 0 {
                    cand.inv.consume(set, delta[s] as u64).ok();
                } else if delta[s] < 0 {
                    cand.inv.restore(set, (-delta[s]) as u64);
                }
            }

            let old_sc_si = cand.statue_scores[si];
            let old_sc_sj = cand.statue_scores[sj];

            let mut new_si = cand.statues[si].clone();
            let mut new_sj = cand.statues[sj].clone();
            new_si.slots[slot_idx] = Slot::new(f_j, t_i);
            new_sj.slots[slot_idx] = Slot::new(f_i, t_j);

            cand.apply_statue(si, new_si, ctx.eval);
            cand.apply_statue(sj, new_sj, ctx.eval);

            Some((cand.statue_scores[si] - old_sc_si) + (cand.statue_scores[sj] - old_sc_sj))
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
