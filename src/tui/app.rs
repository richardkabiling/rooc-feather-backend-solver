use std::collections::HashMap;
use std::sync::{Arc, atomic::{AtomicBool, Ordering}};
use std::thread;
use std::time::Instant;

use crossbeam_channel::bounded;

use crate::data::GameData;
use crate::domain::feather::SET_COUNT;
use crate::domain::inventory::Inventory;
use crate::domain::solution::Solution;
use crate::domain::statue::StatueKind;
use crate::eval::evaluator::Evaluator;
use crate::solver::{Solver, SolveContext, SolverConfig, SolverEvent};

pub fn run(
    game: GameData,
    inventory: Inventory,
    preset_name: &str,
    config: SolverConfig,
) -> anyhow::Result<Solution> {
    use crate::eval::feather_table::FeatherTable;
    use crate::eval::set_bonus_table::{build_attack_table, build_defense_table};
    use crate::eval::normalizer::{compute_norm_factors, effective_weights};
    use crate::solver::sa::SimulatedAnnealing;

    let preset = game.presets.get(preset_name)
        .ok_or_else(|| anyhow::anyhow!("Preset '{}' not found", preset_name))?
        .clone();

    eprintln!("[info] preset       : {}", preset_name);
    eprintln!("[info] time budget  : {}s", config.time_budget_secs);
    eprintln!("[info] restarts     : {}", config.restarts);
    eprintln!("[info] threads      : {}", config.threads);
    eprintln!("[info] seed         : {}", config.seed);
    eprintln!("[info] log every    : {} iters", config.log_every);
    eprintln!("[info] starting solver...");

    let ft = FeatherTable::new(game.feathers.clone());
    let atk_tbl = build_attack_table(&game.attack_bonuses);
    let def_tbl = build_defense_table(&game.defense_bonuses);
    let norm = compute_norm_factors(&ft);
    let ew   = effective_weights(&preset.weights, &norm);
    let eval = Evaluator {
        feather_table: ft,
        attack_bonuses: atk_tbl,
        defense_bonuses: def_tbl,
        eff_weights: ew,
    };
    let game_data = GameData {
        feathers: Vec::new(),
        attack_bonuses: Vec::new(),
        defense_bonuses: Vec::new(),
        presets: HashMap::new(),
    };

    let cancel = Arc::new(AtomicBool::new(false));
    let (tx, rx) = bounded::<SolverEvent>(128);

    // Set cancel on Ctrl-C so the solver winds down and we still emit output.
    {
        let cancel_sig = cancel.clone();
        ctrlc::set_handler(move || {
            eprintln!("\n[interrupted] stopping solver, saving best solution so far...");
            cancel_sig.store(true, Ordering::Relaxed);
        }).ok();
    }

    let cancel2 = cancel.clone();
    let inv = inventory.clone();
    let orig_budget = inventory.budget;
    let config2 = config.clone();
    let preset2 = preset.clone();
    let eval_print = eval.clone();

    let handle = thread::spawn(move || {
        let ctx = SolveContext {
            game: &game_data,
            eval: &eval,
            preset: &preset2,
            inventory: inv,
            config: config2,
            cancel: cancel2,
        };
        SimulatedAnnealing.solve(&ctx, tx)
    });

    let start = Instant::now();
    let log_every = config.log_every.max(1);
    let mut last_obj = 0.0f64;
    let mut iters = 0u64;
    let mut global_best = 0.0f64;
    let mut best_solution: Option<Solution> = None;

    loop {
        match rx.recv_timeout(std::time::Duration::from_millis(100)) {
            Ok(SolverEvent::Progress { chain, iter, best_obj: obj, iters_since_best, .. }) => {
                iters = iter;
                last_obj = obj;
                if obj > global_best { global_best = obj; }
                if iter % log_every == 0 {
                    eprintln!(
                        "[chain {:>2}] elapsed={:.0}s  iters={}  chain_best={:.4}  global_best={:.4}  stagnation={}/{} ({:.1}%)",
                        chain,
                        start.elapsed().as_secs_f64(),
                        iter,
                        last_obj,
                        global_best,
                        iters_since_best,
                        iter,
                        if iter > 0 { iters_since_best as f64 / iter as f64 * 100.0 } else { 0.0 },
                    );
                }
            }
            Ok(SolverEvent::NewBest(chain, sol)) => {
                if sol.objective > global_best { global_best = sol.objective; }
                eprintln!(
                    "[chain {:>2}] new best  elapsed={:.0}s  obj={:.4}  global_best={:.4}",
                    chain,
                    start.elapsed().as_secs_f64(),
                    sol.objective,
                    global_best,
                );
                best_solution = Some(*sol);
            }
            Ok(SolverEvent::Done(sol)) => {
                if sol.objective > global_best { global_best = sol.objective; }
                eprintln!(
                    "[done] elapsed={:.0}s  iters={}  best={:.4}",
                    start.elapsed().as_secs_f64(), iters, sol.objective
                );
                best_solution = Some(*sol);
                break;
            }
            Err(crossbeam_channel::RecvTimeoutError::Disconnected) => break,
            Err(crossbeam_channel::RecvTimeoutError::Timeout) => {}
        }
    }

    // Drain any events that arrived between the last recv and channel close.
    while let Ok(ev) = rx.try_recv() {
        match ev {
            SolverEvent::NewBest(chain, sol) => {
                if sol.objective > global_best { global_best = sol.objective; }
                eprintln!("[chain {:>2}] new best  elapsed={:.0}s  obj={:.4}  global_best={:.4}",
                    chain, start.elapsed().as_secs_f64(), sol.objective, global_best);
                best_solution = Some(*sol);
            }
            SolverEvent::Done(sol) => { best_solution = Some(*sol); }
            _ => {}
        }
    }

    let _ = handle.join();
    let mut sol = best_solution.ok_or_else(|| anyhow::anyhow!("Solver produced no solution"))?;

    // Sort slots within each statue by individual feather score contribution (descending).
    {
        use crate::domain::stats::STAT_COUNT;
        for statue in sol.statues.iter_mut() {
            let old_slots = statue.slots;
            let mut order: [usize; 5] = [0, 1, 2, 3, 4];
            order.sort_unstable_by(|&a, &b| {
                let score = |slot: &crate::domain::statue::Slot| -> f64 {
                    let sv = eval_print.feather_table.stats_at(slot.feather, slot.tier);
                    (0..STAT_COUNT).map(|k| sv[k] * eval_print.eff_weights[k]).sum::<f64>()
                };
                score(&old_slots[b]).partial_cmp(&score(&old_slots[a])).unwrap()
            });
            for (new_pos, old_pos) in order.iter().enumerate() {
                statue.slots[new_pos] = old_slots[*old_pos];
            }
        }
    }

    // Sort attack statues (indices 0-4) and defense statues (indices 5-9) by score descending.
    {
        let mut atk: Vec<(usize, f64)> = (0..5).map(|i| (i, sol.statue_scores[i])).collect();
        let mut def: Vec<(usize, f64)> = (5..10).map(|i| (i, sol.statue_scores[i])).collect();
        atk.sort_unstable_by(|a, b| b.1.partial_cmp(&a.1).unwrap());
        def.sort_unstable_by(|a, b| b.1.partial_cmp(&a.1).unwrap());

        let old_statues = sol.statues.clone();
        let old_scores  = sol.statue_scores;
        for (new_pos, (old_pos, _)) in atk.iter().enumerate() {
            sol.statues[new_pos]       = old_statues[*old_pos].clone();
            sol.statue_scores[new_pos] = old_scores[*old_pos];
        }
        for (new_pos, (old_pos, _)) in def.iter().enumerate() {
            sol.statues[5 + new_pos]       = old_statues[*old_pos].clone();
            sol.statue_scores[5 + new_pos] = old_scores[*old_pos];
        }
    }

    print_solution(&sol, &eval_print, &orig_budget);
    Ok(sol)
}

pub fn print_solution(sol: &Solution, eval: &Evaluator, orig_budget: &[u64; SET_COUNT]) {
    use crate::domain::stats::STAT_COUNT;

    const STAT_NAMES: [&str; STAT_COUNT] = [
        "PvP Dmg Bonus", "PvP Dmg Reduction", "Ignore PDEF", "Ignore MDEF",
        "PDMG", "MDMG", "PATK", "MATK",
        "PvE Dmg Bonus", "PvE Dmg Reduction", "PDEF", "MDEF", "HP",
        "PDMG Reduction", "MDMG Reduction", "INT/DEX/STR", "VIT",
    ];
    const SET_NAMES: [&str; SET_COUNT] = ["STDN", "DN", "ST", "LD", "Purple"];

    println!("Total objective: {:.4}\n", sol.objective);

    // Budget used
    let mut used = [0u64; SET_COUNT];
    for statue in &sol.statues {
        for slot in &statue.slots {
            let def = eval.feather_table.get(slot.feather);
            used[def.set as usize] += def.t1_cost_at(slot.tier);
        }
    }
    println!("Budget used:");
    for i in 0..SET_COUNT {
        println!("  {:>5} / {:<5} T1 {}", used[i], orig_budget[i], SET_NAMES[i]);
    }
    println!();

    // Total stats across all statues
    let mut total_final = [0.0f64; STAT_COUNT];
    let mut total_score = [0.0f64; STAT_COUNT];
    for statue in &sol.statues {
        let min_idx = statue.min_tier().get() as usize - 1;
        let mut raw = [0.0f64; STAT_COUNT];
        for slot in &statue.slots {
            let sv = eval.feather_table.stats_at(slot.feather, slot.tier);
            for i in 0..STAT_COUNT { raw[i] += sv[i]; }
        }
        let (pct, flat) = match statue.kind {
            StatueKind::Attack  => { let b = &eval.attack_bonuses[min_idx];  (&b.pct, &b.flat) }
            StatueKind::Defense => { let b = &eval.defense_bonuses[min_idx]; (&b.pct, &b.flat) }
        };
        for i in 0..STAT_COUNT {
            let f = raw[i] * (1.0 + pct[i] / 100.0) + flat[i];
            total_final[i] += f;
            total_score[i] += f * eval.eff_weights[i];
        }
    }
    println!("Total stats:");
    println!("  {:<24}  {:>10}  {:>10}", "stat", "total", "score");
    for i in 0..STAT_COUNT {
        if total_final[i].abs() > 1e-4 || total_score[i].abs() > 1e-4 {
            println!("  {:<24}  {:>10.2}  {:>10.4}", STAT_NAMES[i], total_final[i], total_score[i]);
        }
    }
    println!();

    // Per-statue breakdown
    for (idx, statue) in sol.statues.iter().enumerate() {
        let min_idx = statue.min_tier().get() as usize - 1;
        let mut raw = [0.0f64; STAT_COUNT];
        for slot in &statue.slots {
            let sv = eval.feather_table.stats_at(slot.feather, slot.tier);
            for i in 0..STAT_COUNT { raw[i] += sv[i]; }
        }
        let (pct, flat) = match statue.kind {
            StatueKind::Attack  => { let b = &eval.attack_bonuses[min_idx];  (&b.pct, &b.flat) }
            StatueKind::Defense => { let b = &eval.defense_bonuses[min_idx]; (&b.pct, &b.flat) }
        };
        println!("Statue {} ({:?}): score={:.4}  min_tier=T{}",
            idx + 1, statue.kind, sol.statue_scores[idx], statue.min_tier().get());
        for slot in &statue.slots {
            let def = eval.feather_table.get(slot.feather);
            println!("  {:?} T{}  ({} T1 {:?})", slot.feather, slot.tier.get(), def.t1_cost_at(slot.tier), def.set);
        }
        println!("  --- stat breakdown ---");
        println!("  {:<24}  {:>8}  {:>10}  {:>10}  {:>10}  {:>10}  {:>10}",
            "stat", "raw", "pct_bonus", "after_pct", "flat_bonus", "final", "score");
        for i in 0..STAT_COUNT {
            let after_pct = raw[i] * (1.0 + pct[i] / 100.0);
            let final_val = after_pct + flat[i];
            let score     = final_val * eval.eff_weights[i];
            if raw[i].abs() > 1e-4 || flat[i].abs() > 1e-4 {
                println!("  {:<24}  {:>8.2}  {:>9.2}%  {:>10.2}  {:>10.2}  {:>10.2}  {:>10.4}",
                    STAT_NAMES[i], raw[i], pct[i], after_pct, flat[i], final_val, score);
            }
        }
        println!();
    }
}

pub fn save_solution(sol: &Solution) -> anyhow::Result<()> {
    use std::fs::File;
    use std::io::Write;
    use crate::domain::statue::StatueKind;

    let mut out = String::new();
    out.push_str(&format!("Total objective: {:.4}\n\n", sol.objective));
    for (i, statue) in sol.statues.iter().enumerate() {
        out.push_str(&format!("Statue {} ({:?}): score={:.4}\n", i+1, statue.kind, sol.statue_scores[i]));
        for slot in &statue.slots {
            out.push_str(&format!("  {:?} T{}\n", slot.feather, slot.tier.get()));
        }
    }
    let mut f = File::create("best_solution.txt")?;
    f.write_all(out.as_bytes())?;

    // Build JSON output
    let mut attack: Vec<serde_json::Value> = Vec::new();
    let mut defense: Vec<serde_json::Value> = Vec::new();
    for statue in &sol.statues {
        let slots: Vec<serde_json::Value> = statue.slots.iter().map(|s| {
            serde_json::json!([format!("{:?}", s.feather), s.tier.get() as u32])
        }).collect();
        let val = serde_json::Value::Array(slots);
        match statue.kind {
            StatueKind::Attack  => attack.push(val),
            StatueKind::Defense => defense.push(val),
        }
    }
    let json = serde_json::json!({ "attack": attack, "defense": defense });
    let mut jf = File::create("best_solution.json")?;
    jf.write_all(serde_json::to_string_pretty(&json)?.as_bytes())?;

    Ok(())
}
