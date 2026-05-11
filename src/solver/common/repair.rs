use crate::domain::feather::{FeatherId, FeatherDef, Set, Tier};
use crate::domain::statue::{Slot, Statue, StatueKind};
use crate::domain::inventory::Inventory;
use crate::eval::evaluator::Evaluator;
use crate::eval::feather_table::FeatherTable;
use crate::domain::feather::{eligible_orange, eligible_purple};

/// Greedy repair: given a solution state and per-set budgets, spend all remaining budget
/// by upgrading slots, prioritising the slot that gives the best marginal objective gain.
/// Returns the modified statues and the remaining budget (should all be 0 after repair).
pub fn greedy_consume(statues: &mut [Statue; 10], inv: &mut Inventory, eval: &Evaluator) {
    // Repeat until no set has enough budget to upgrade any slot
    loop {
        let mut best_delta = 0.0_f64;
        let mut best_action: Option<(usize, usize)> = None; // (statue_idx, slot_idx)

        for (si, statue) in statues.iter().enumerate() {
            let before = eval.statue_score(statue);
            for (sli, slot) in statue.slots.iter().enumerate() {
                if let Some(next_tier) = slot.tier.next() {
                    let cost = eval.feather_table.get(slot.feather).upgrade_cost_from(slot.tier);
                    if cost == 0 { continue; }
                    let set = eval.feather_table.get(slot.feather).set;
                    if inv.get(set) < cost { continue; }

                    // Compute delta score
                    let mut tmp = statue.clone();
                    tmp.slots[sli].tier = next_tier;
                    let after = eval.statue_score(&tmp);
                    let delta = after - before;

                    if delta > best_delta {
                        best_delta = delta;
                        best_action = Some((si, sli));
                    }
                }
            }
        }

        if best_action.is_none() {
            // No upgrade improves score with remaining budget — still try to spend if any slot can be upgraded
            // (to satisfy the "fully consumed" constraint)
            let mut found = false;
            'outer: for (si, statue) in statues.iter().enumerate() {
                for (sli, slot) in statue.slots.iter().enumerate() {
                    if let Some(_next_tier) = slot.tier.next() {
                        let cost = eval.feather_table.get(slot.feather).upgrade_cost_from(slot.tier);
                        if cost == 0 { continue; }
                        let set = eval.feather_table.get(slot.feather).set;
                        if inv.get(set) >= cost {
                            // Spend it anyway to consume budget
                            inv.consume(set, cost).ok();
                            statues[si].slots[sli].tier = _next_tier;
                            found = true;
                            break 'outer;
                        }
                    }
                }
            }
            if !found { break; }
        } else {
            let (si, sli) = best_action.unwrap();
            let slot = statues[si].slots[sli];
            let cost = eval.feather_table.get(slot.feather).upgrade_cost_from(slot.tier);
            let set  = eval.feather_table.get(slot.feather).set;
            inv.consume(set, cost).ok();
            statues[si].slots[sli].tier = slot.tier.next().unwrap();
        }
    }
}

/// Check whether the "fully consumed" constraint is satisfied:
/// for each set, remaining budget < min upgrade cost across all non-T20 slots in that set.
pub fn is_fully_consumed(statues: &[Statue; 10], inv: &Inventory, eval: &Evaluator) -> bool {
    use crate::domain::feather::SET_COUNT;
    let mut min_upgrade: [u64; SET_COUNT] = [u64::MAX; SET_COUNT];

    for statue in statues {
        for slot in &statue.slots {
            if let Some(_) = slot.tier.next() {
                let def = eval.feather_table.get(slot.feather);
                let cost = def.upgrade_cost_from(slot.tier);
                if cost > 0 {
                    let s = def.set as usize;
                    if cost < min_upgrade[s] { min_upgrade[s] = cost; }
                }
            }
        }
    }

    for s in 0..SET_COUNT {
        let budget = inv.budget[s];
        let min_c  = min_upgrade[s];
        if min_c == u64::MAX { continue; } // all T20 in this set — OK
        if budget >= min_c { return false; }
    }
    true
}
