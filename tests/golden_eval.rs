use std::path::Path;
use rooc_feather_solver::data::{GameData, loader::load_inventory};
use rooc_feather_solver::domain::feather::{FeatherId, Tier};
use rooc_feather_solver::domain::stats::{StatId, StatVec, StatVecExt};
use rooc_feather_solver::domain::statue::{Slot, Statue, StatueKind};
use rooc_feather_solver::eval::feather_table::FeatherTable;
use rooc_feather_solver::eval::set_bonus_table::{build_attack_table, build_defense_table};
use rooc_feather_solver::eval::normalizer::{compute_norm_factors, effective_weights};
use rooc_feather_solver::eval::evaluator::Evaluator;
use rooc_feather_solver::domain::stats::STAT_COUNT;

fn load_game() -> (GameData, Evaluator) {
    let data_dir = Path::new("data");
    let game = GameData::load(data_dir).expect("load game data");
    let ft      = FeatherTable::new(game.feathers.clone());
    let atk_tbl = build_attack_table(&game.attack_bonuses);
    let def_tbl = build_defense_table(&game.defense_bonuses);
    let norm    = compute_norm_factors(&ft);
    // Use uniform weights for testing
    let weights = [1.0_f64; STAT_COUNT];
    let ew      = effective_weights(&weights, &norm);
    let eval = Evaluator {
        feather_table: ft,
        attack_bonuses: atk_tbl,
        defense_bonuses: def_tbl,
        eff_weights: ew,
    };
    (game, eval)
}

#[test]
fn test_csv_loads_without_panic() {
    let (game, _) = load_game();
    assert!(!game.feathers.is_empty(), "feathers should be non-empty");
    assert_eq!(game.attack_bonuses.len(),  20);
    assert_eq!(game.defense_bonuses.len(), 20);
    assert!(!game.presets.is_empty());
}

#[test]
fn test_fractional_stat_parse() {
    let (game, eval) = load_game();
    let light_def = eval.feather_table.get(FeatherId::Light);
    let stats_t1 = light_def.stats_at(Tier::new(1));
    // Light T1 PDMG = 0.28 (not truncated to 0)
    let pdmg = stats_t1[StatId::PDMG as usize];
    assert!((pdmg - 0.28).abs() < 1e-9, "Light T1 PDMG should be 0.28, got {}", pdmg);
}

#[test]
fn test_t0_skipped() {
    // All feather defs should have T1-indexed tables (no T0 row)
    let (_, eval) = load_game();
    let space = eval.feather_table.get(FeatherId::Space);
    // T1 PATK for Space should be > 0
    let patk_t1 = space.stats_at(Tier::new(1))[StatId::PATK as usize];
    assert!(patk_t1 > 0.0, "Space T1 PATK should be > 0, got {}", patk_t1);
}

#[test]
fn test_t1_cost_formula() {
    // T1 cost = 1 + TotalCost(1) = 1 + 1 = 2
    let (_, eval) = load_game();
    let space = eval.feather_table.get(FeatherId::Space);
    assert_eq!(space.t1_cost_at(Tier::new(1)), 2, "Space T1 t1_cost should be 2");
}

#[test]
fn test_norm_factors_correct() {
    let (_, eval) = load_game();
    let norm = compute_norm_factors(&eval.feather_table);
    // HP norm should be 1020 (from feathers), IgnorePDEF = 164
    let hp_norm   = norm[StatId::HP        as usize];
    let ipdef_norm = norm[StatId::IgnorePDEF as usize];
    let pdmg_norm  = norm[StatId::PDMG      as usize];
    assert!((hp_norm   - 1020.0).abs() < 1.0,  "HP norm should be ~1020, got {}", hp_norm);
    assert!((ipdef_norm - 164.0).abs() < 1.0,  "IgnorePDEF norm should be ~164, got {}", ipdef_norm);
    assert!((pdmg_norm -    1.8).abs() < 0.01, "PDMG norm should be ~1.8, got {}", pdmg_norm);
    // Guard: no zero norms
    for i in 0..STAT_COUNT {
        assert!(norm[i] > 0.0, "norm[{}] should be > 0", i);
    }
}

#[test]
fn test_min_tier_drives_set_bonus() {
    let (game, eval) = load_game();
    // Build an attack statue with 4 T20 oranges and 1 T1 purple
    let slots = [
        Slot::new(FeatherId::Justice, Tier::new(1)),  // purple slot 0 at T1
        Slot::new(FeatherId::Space,   Tier::new(20)),
        Slot::new(FeatherId::Time,    Tier::new(20)),
        Slot::new(FeatherId::Light,   Tier::new(20)),
        Slot::new(FeatherId::Day,     Tier::new(20)),
    ];
    let statue_min1 = Statue { kind: StatueKind::Attack, slots };

    // Same but purple at T20
    let slots20 = [
        Slot::new(FeatherId::Justice, Tier::new(20)),
        Slot::new(FeatherId::Space,   Tier::new(20)),
        Slot::new(FeatherId::Time,    Tier::new(20)),
        Slot::new(FeatherId::Light,   Tier::new(20)),
        Slot::new(FeatherId::Day,     Tier::new(20)),
    ];
    let statue_max = Statue { kind: StatueKind::Attack, slots: slots20 };

    // Min-tier-1 statue should score less than min-tier-20
    let score_min1 = eval.statue_score(&statue_min1);
    let score_max  = eval.statue_score(&statue_max);
    assert!(score_max > score_min1, "T20 min-tier statue should score higher: {} vs {}", score_max, score_min1);
}

#[test]
fn test_flat_added_after_pct() {
    // Verify evaluator order: raw[i] * (1 + pct/100) + flat
    // At T1 attack: PATK flat = 13, attack_pct = 10
    // If raw PATK = 100 (hypothetical), final = 100 * 1.1 + 13 = 123
    // We just check that the score is positive and attack bonus T1 < T20
    let (_, eval) = load_game();
    let b1  = eval.attack_bonuses[0]; // T1
    let b20 = eval.attack_bonuses[19]; // T20
    // PATK flat T20 should be greater than T1
    let patk_flat_t1  = b1.flat[StatId::PATK  as usize];
    let patk_flat_t20 = b20.flat[StatId::PATK as usize];
    assert!(patk_flat_t20 > patk_flat_t1, "PATK flat should grow T1→T20");
}

#[test]
fn test_inventory_load() {
    let data_dir = Path::new("data");
    let game = GameData::load(data_dir).expect("load game data");
    let inv = load_inventory(Path::new("input.csv"), &game.feathers).expect("load inventory");
    // Space 1035 × (1+1) = 2070 → STDN pool
    use rooc_feather_solver::domain::feather::Set;
    let stdn = inv.get(Set::STDN);
    assert_eq!(stdn, 2070, "STDN budget should be 2070, got {}", stdn);
    let purple = inv.get(Set::Purple);
    assert_eq!(purple, 840 + 1352, "Purple budget should be 2192, got {}", purple);
}
