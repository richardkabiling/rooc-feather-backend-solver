# Multithreaded Feather Solver — Plan & Spec

## Context
The repo contains only game data (`data/*.csv`), an inventory file (`input.csv`), and rules (`CLAUDE.md`). No source code exists. We are building a Rust crate that, given a feather inventory and a chosen objective preset, computes the best assignment of feathers across 10 statues (5 Attack + 5 Defense) — selecting which feathers go where and what tier each is at — subject to per-set inventory budgets that must be **fully consumed**. The crate exposes a `Solver` trait so multiple solvers can be plugged in and compared; we ship two to start: a multi-start parallel local-search solver (SA) and an exact branch-and-bound solver. Interaction is via a ratatui TUI with live progress.

## Decisions captured from the user
- **Language**: Rust + cargo + rayon.
- **Objective**: a single preset (chosen at runtime) applied to all 10 statues; maximize the weighted-stat sum.
- **Inventory**: arbitrary tiers in input rows; each row converted to T1-equivalent budget in its set's pool at load time.
- **Budget**: hard constraint, must be **fully consumed** — every non-T20 slot whose set still has ≥ its cost-to-next-tier of budget must be upgraded.
- **Output / interaction**: Full ratatui TUI with panels for inventory, presets, solver config, live progress, and final result.
- **Solvers**: extensible via a `Solver` trait. Ship two to start so they can be compared:
  1. multi-start parallel local search (simulated annealing) within a configurable time budget,
  2. exact branch-and-bound, however long it takes.

## Domain spec (compressed from CLAUDE.md)
- Attack statue = 4 Orange + 1 Purple feather, all unique within the statue, types ∈ {Attack, Hybrid}. Defense statue = 4 Orange + 1 Purple, types ∈ {Defense, Hybrid}.
- Each feather has tier 1..20 with stats per tier (`data/feather-stats.csv`). A slot at tier N consumes `1 + TotalCost(N)` T1-equivalent feathers from its **set's** inventory pool.
- Sets (define the conversion pool; conversions are 1:1 within a set):
  - `STDN`: Space, Time (Attack); Divine, Nature (Defense)
  - `DN`: Day (Attack); Night (Defense)
  - `ST`: Sky (Attack); Terra (Defense)
  - `LD`: Light, Dark (both Hybrid Orange)
  - `Purple`: Justice, Grace (Hybrid Purple); Stats (Attack Purple); Soul, Virtue, Mercy (Defense Purple)
- Per-statue effective stats: sum raw feather stats → multiply by category percentage from set bonus → add flat set bonus. **Set-bonus tier = min feather tier in the statue.**
- Stat categories: Attack-stats {Ignore PDEF, Ignore MDEF, PATK, MATK, PDMG, MDMG}, Defense-stats {PDEF, MDEF, HP, PDMG Reduction, MDMG Reduction}, PvE {PvE DMG Reduction, PvE DMG Bonus}, PvP {PvP DMG Reduction, PvP DMG Bonus}. INT/DEX/STR and VIT are pure flat.
- Objective: weighted sum of stats (weights from chosen preset in `data/presets.csv`) summed across all 10 statues.
- Hard constraint: per set, residual budget < min cost-to-next-tier across non-T20 slots in that set (or all slots are T20).

### Preset token normalization (`data/presets.csv` → `StatId`)
`PvEDmgBonus`→`PvEDmgBonus`, `PvEDmgReduction`→`PvEDmgReduction`, `PvPDmgBonus`→`PvPDmgBonus`, `PvPDmgReduction`→`PvPDmgReduction`, `PDMGReduction`→`PDMGReduction`, `MDMGReduction`→`MDMGReduction`, `IgnorePDEF`→`IgnorePDEF`, `IgnoreMDEF`→`IgnoreMDEF`, `INTDEXSTR`→`IntDexStr`, bare `PATK/MATK/PDMG/MDMG/PDEF/MDEF/HP/VIT`.

---

## Correction 1: `StatVec` is `[f64; 17]`, not 16

Counting unique stat columns across all feather/bonus CSVs:

| # | StatId | Source |
|---|--------|--------|
| 0 | `PvPDmgBonus` | feather + attack flat bonus |
| 1 | `PvPDmgReduction` | feather + defense flat bonus |
| 2 | `IgnorePDEF` | feather |
| 3 | `IgnoreMDEF` | feather |
| 4 | `PDMG` | feather (fractional for LD) |
| 5 | `MDMG` | feather (fractional for LD) |
| 6 | `PATK` | feather + attack flat bonus |
| 7 | `MATK` | feather + attack flat bonus |
| 8 | `PvEDmgBonus` | feather |
| 9 | `PvEDmgReduction` | feather |
| 10 | `PDEF` | feather |
| 11 | `MDEF` | feather |
| 12 | `HP` | feather + defense flat bonus |
| 13 | `PDMGReduction` | feather |
| 14 | `MDMGReduction` | feather |
| 15 | `IntDexStr` | feather (Stats feather only) |
| 16 | `VIT` | feather (Virtue feather only) |

**Total: 17.** Use `StatVec = [f64; 17]`, indexed by `StatId as usize`.

> Note: LD feathers (Light/Dark) have fractional PDMG/MDMG values (e.g. 0.28 at T1). `f64` handles this; do not parse these columns as integers.

---

## Correction 2: Set bonus column mapping (per CSV)

`data/attack-set-bonuses.csv` columns:
```
Tier | PATK (flat) | MATK (flat) | PvP DMG Bonus (flat) | Attack Stats % | PvE Stats % | PvP Stats %
```

`data/defense-set-bonuses.csv` columns:
```
Tier | HP (flat) | PvP DMG Reduction (flat) | Defense Stats % | PvE Stats % | PvP Stats %
```

The two tables have **different flat columns** and **different percentage categories**. Model as two separate structs:

```rust
pub struct AttackSetBonus  { pub flat: StatVec, pub pct: StatVec }
pub struct DefenseSetBonus { pub flat: StatVec, pub pct: StatVec }
```

Precompute as `[AttackSetBonus; 20]` and `[DefenseSetBonus; 20]` (indexed 0 = T1 … 19 = T20).

### Percentage multiplier category mapping

**Attack statues**:
- `Attack Stats %` → scales: `IgnorePDEF, IgnoreMDEF, PATK, MATK, PDMG, MDMG`
- `PvE Stats %`    → scales: `PvEDmgReduction, PvEDmgBonus`
- `PvP Stats %`    → scales: `PvPDmgReduction, PvPDmgBonus`

**Defense statues**:
- `Defense Stats %` → scales: `PDEF, MDEF, HP, PDMGReduction, MDMGReduction`
- `PvE Stats %`     → scales: `PvEDmgReduction, PvEDmgBonus`
- `PvP Stats %`     → scales: `PvPDmgReduction, PvPDmgBonus`

Stats not in any category (`IntDexStr`, `VIT`) are passed through **unscaled** — they receive only flat additions from set bonuses.

> Pitfall: `PvP DMG Bonus` appears both as a feather stat (in the raw sum, scaled by `PvP Stats %`) **and** as an attack-set flat bonus added **after** scaling. They share the same `StatVec` slot — the flat addition must happen after scaling, not instead of it.

### Evaluator inner loop (corrected)
```
for each stat i in 0..17:
  scaled[i] = raw[i] * (1.0 + pct[i] / 100.0)
  final[i]  = scaled[i] + flat[i]
score = dot(final, preset.weights)
```
Where `pct` and `flat` are taken from the bonus table at index `min_tier_of_statue - 1`.

---

## Correction 3: T0 rows in `feather-stats.csv`

Every feather has a Tier=0 row (all-zero stats, `Total Cost`=0, `Cost to Next Tier`=1). These represent the "unacquired" baseline. **Skip T0 rows during loading** — the tier table is indexed 1..=20 → `table[tier as usize - 1]`.

---

## Correction 4: T1-equivalent budget — worked example from `input.csv`

`input.csv` (all feathers at Tier 1; `TotalCost(1) = 1`):
```
Feather  Tier  Count   Set     Pool contribution
Space    1     1035    STDN    1035 × (1+1) = 2070
Light    1     698     LD       698 × (1+1) = 1396
Sky      1     570     ST       570 × (1+1) = 1140
Day      1     570     DN       570 × (1+1) = 1140
Justice  1     420     Purple   420 × (1+1) =  840
Stats    1     676     Purple   676 × (1+1) = 1352
```

Purple pool total = 840 + 1352 = 2192.

Only 6 of 18 feather types appear. **Missing feather types contribute 0 to the pool** and may still be placed (they draw from the shared set pool). The solver's eligible-feather list must include all feathers of the correct type/rarity/statue-kind whose set pool is non-zero.

> Budget tightness example: STDN pool = 2070. Placing 20 slots (2 per attack statue × 5 statues + 2 per defense × 5) all at T10 costs 20 × (1+133) = 2680 > 2070 — so budget genuinely constrains tier choices for STDN.

---

## CSV → Rust struct mapping

| File | Parsed into | Notes |
|------|-------------|-------|
| `data/feather-stats.csv` | `FeatherRow { feather, type_, set, tier, rarity, cost_to_next, stats: StatVec, total_cost }` | Skip Tier=0; parse stat columns as `Option<f64>` → `unwrap_or(0.0)` |
| `data/attack-set-bonuses.csv` | `AttackBonusRow { tier, patk, matk, pvp_dmg_bonus, attack_pct, pve_pct, pvp_pct }` | Precomputed into `[AttackSetBonus; 20]` |
| `data/defense-set-bonuses.csv` | `DefenseBonusRow { tier, hp, pvp_dmg_reduction, defense_pct, pve_pct, pvp_pct }` | Precomputed into `[DefenseSetBonus; 20]` |
| `data/attack-stats.csv` | `Vec<StatId>` (6 stats) | Used to build the attack percentage mask |
| `data/defense-stats.csv` | `Vec<StatId>` (5 stats) | Used to build the defense percentage mask |
| `data/pvp-stats.csv` | `Vec<StatId>` (2 stats) | PvP percentage mask |
| `data/pve-stats.csv` | `Vec<StatId>` (2 stats) | PvE percentage mask |
| `data/presets.csv` | `HashMap<String, Preset>` grouped by `preset` column | Token-normalize stat names per the normalization table above |
| `input.csv` | `InventoryRow { feather: String, tier: u8, count: u64 }` → `Inventory` | `count × (1 + total_cost[tier])` per set pool |

---

## Crate layout (`src/`)

```
main.rs                          CLI parse, load data, launch TUI
lib.rs                           re-exports for integration tests
data/{mod, schema, loader}.rs    raw serde rows + load_all()
domain/
  feather.rs                     FeatherId, FeatherDef, Type, Set, Rarity, Tier(1..=20)
  stats.rs                       StatVec=[f64;17], StatId enum (17 variants), Category
  statue.rs                      StatueKind, Slot, Statue
  inventory.rs                   per-Set T1-equivalent budgets ([u64; 5] keyed by Set enum)
  preset.rs                      Preset {name, weights:StatVec}, token normalization
  solution.rs                    Solution {statues:[Statue;10], objective, breakdown}
eval/
  feather_table.rs               [feather][tier] -> StatVec, cumulative cost
  set_bonus_table.rs             [tier] -> (AttackSetBonus | DefenseSetBonus); category masks
  evaluator.rs                   statue_score, solution_score (hot path)
solver/
  mod.rs                         pub trait Solver, SolveContext, SolverConfig, SolverEvent, ProgressTx
  common/
    repair.rs                    greedy "fully consume budget" finisher
    neighborhoods.rs             swap-feather, retier, tier-min lift, cross-statue swap
    candidate.rs                 mutable Candidate w/ incremental delta scoring
  sa.rs                          multi-start simulated annealing (rayon)
  bnb.rs                         exact branch-and-bound (rayon)
  bnb_lp.rs                      LP relaxation upper bound (good_lp+CBC, feature-gated)
tui/
  mod.rs / app.rs                App state machine: Setup -> Running -> Done
  widgets/{inventory, preset_picker, solver_picker, progress, result}.rs
util/{rng, channels}.rs          seeded Xoshiro per worker; crossbeam mpsc helpers
tests/
  golden_eval.rs                 hand-computed statue scores
  solver_agreement.rs            SA vs BnB on a tiny inventory
```

---

## `Cargo.toml` skeleton

```toml
[package]
name    = "rooc-feather-solver"
version = "0.1.0"
edition = "2021"

[dependencies]
ratatui           = "0.28"
crossterm         = "0.28"
rayon             = "1.10"
crossbeam-channel = "0.5"
csv               = "1.3"
serde             = { version = "1", features = ["derive"] }
anyhow            = "1"
thiserror         = "1"
clap              = { version = "4", features = ["derive"] }
rand              = "0.8"
rand_xoshiro      = "0.6"

[features]
default = []
bnb-lp  = ["good_lp"]

[dependencies.good_lp]
version  = "1.8"
features = ["coin_cbc"]
optional = true
```

---

## Domain & evaluator design (the math has to be right)
- `Tier(u8)` newtype enforced 1..=20 at construction.
- `StatVec = [f64; 17]` indexed by compile-time `StatId`. Stat math is fused-multiply-add over the 17 lanes — far faster than `HashMap` and `Copy`.
- `Inventory` = `[u64; 5]` keyed by `Set`. `consume(set, t1_units) -> Result<()>` mutates; cheap to snapshot/restore for backtracking.
- `Statue { kind, slots: [Slot; 5] }` — slot 0 by convention is the purple slot.
- Per-tier feather stats and set-bonus rows are **precomputed once** into dense tables. Inner loop:
  1. sum 5 feather `StatVec`s → `raw`,
  2. find `min_tier`,
  3. lookup `(pct, flat)` from the appropriate bonus table at `min_tier - 1`,
  4. `final[i] = raw[i] * (1.0 + pct[i]/100.0) + flat[i]`,
  5. dot-product with `preset.weights`.
- Optional micro-opt: pre-fold preset weights into the per-tier bonus tables so step 5 collapses into step 4. Apply only after correctness is verified.

---

## `Solver` trait

```rust
pub trait Solver: Send + Sync {
    fn name(&self) -> &str;
    fn solve(&self, ctx: &SolveContext, tx: ProgressTx) -> Solution;
}
pub struct SolveContext<'a> {
    pub game:      &'a GameData,
    pub eval:      &'a Evaluator,
    pub preset:    &'a Preset,
    pub inventory: Inventory,
    pub config:    SolverConfig,        // time_budget, seed, restarts, threads, solver-specific knobs
    pub cancel:    Arc<AtomicBool>,
}
pub enum SolverEvent {
    Progress { iter: u64, best_obj: f64, budget_used: [u64; 5] },
    NewBest(Solution),
    Done(Solution),
}
```

`ProgressTx` is a `crossbeam_channel::Sender<SolverEvent>` with a bounded buffer; the TUI drops on full to avoid blocking the solver.

---

## Solver A — Multi-start parallel SA
- **Seed**: greedy fill — assign 4 oranges per statue maximizing per-feather marginal weight at T1, pick purple, run repair.
- **Neighborhoods** (sampled with weights):
  1. Swap one orange in a statue for a different legal orange from the same set.
  2. Swap the purple slot's feather for another eligible purple feather.
  3. ±1 tier on a single slot.
  4. Cross-statue feather swap (same set and type, or both Hybrid).
  5. **Tier-min lift**: +1 tier on the statue's current min-tier slot (drives set bonus).
- Acceptance: Metropolis. Cooling: geometric, T₀ scaled to mean |Δ| of 100 random moves.
- **Critical**: every accepted move runs `greedy_consume` to satisfy the "fully consumed" constraint, and SA evaluates the **post-repair** objective (otherwise it oscillates between feasible/infeasible states).
- **Parallelism**: `rayon::scope` runs N independent chains with distinct seeds; pick global best. Each chain holds a private `Candidate` with O(1) delta scoring (only the affected statue is rescored).

---

## Solver B — Branch and bound (rayon work-stealing)
Two-stage decomposition:
1. **Configuration enumeration**: choose the 5 feathers per statue (ignore tiers). Coupling across statues is via shared budgets, so we enumerate jointly with bounding, not as a Cartesian product.
2. **Tier assignment**: given fixed configurations, decide tiers for all slots subject to ≤5 per-set budget constraints + "fully consumed" rule.

Bounds:
- **Stage 2 LP relaxation**: relax tiers to continuous in [1,20]; assume `min_tier=20` for set bonus (admissible — bonus is concave in min_tier, so this overestimates). Solve via `good_lp`+CBC (50 vars, ~6 constraints — milliseconds), feature-gated behind `--features bnb-lp` so default builds don't need CBC.
- **Stage 1 admissible UB** for partial configurations: fixed statues use their LP bound; undecided statues use their per-statue tier-20 best score (assumes infinite budget).

Warm-start the BnB incumbent with Solver A's result.

`rayon::iter::ParallelBridge` over Stage-1 nodes; shared `AtomicU64` incumbent (bit-cast f64) for cross-thread pruning.

---

## TUI (ratatui)
Single screen, three rows:
- **Top (30%)**: 3 side-by-side panels — inventory (per-set budget bars), preset picker, solver picker + config form (time budget, restarts, seed).
- **Middle (40%)**: live progress — sparkline of best-objective-over-time, gauge for elapsed/time-budget, counters (iters/sec, best obj, current obj, % budget consumed per set).
- **Bottom (30%)**: result table — 10 rows (statue kind, feathers, tiers, min tier, statue obj), sorted by objective desc.

Event loop: `crossterm` polls keys; solver thread sends `SolverEvent`s via channel; redraw at 30 Hz max. Keys: `Tab` cycle panels, `Enter` start, `q` quit, `s` save best to JSON.

---

## Subtle pitfalls (do not re-discover)
- **StatVec is [f64;17]**: 17 stats, not 16. The `IntDexStr` and `VIT` columns are sparse (only Stats/Virtue feathers) but must be in the dense vector.
- **Fractional stats**: LD feather PDMG/MDMG values are fractional (e.g. 0.28 at T1). Parse all stat columns as `Option<f64>`, then `unwrap_or(0.0)`.
- **T0 rows**: `feather-stats.csv` has a Tier=0 row per feather (all zeros). Skip at load time; tier table is 1-indexed.
- **Set bonus tables differ**: attack and defense have different flat-column sets. Use two separate struct types, not a single generic table.
- **PvP DMG Bonus double-add**: feather raw value gets scaled by `PvP Stats %`; then the attack flat bonus is added after. Both share slot 0 of `StatVec` — ensure flat addition happens *after* scaling, not instead of it.
- **Min-tier set bonus**: tier-up of a non-min slot doesn't help the bonus. SA needs the explicit "tier-min lift" move or it gets trapped.
- **Shared budgets**: LD and Purple sets are consumed by both attack and defense statues. Repair must allocate globally per set, not per statue.
- **Fully-consumed is hard, not soft**: every emitted `Solution` must pass a post-repair check. SA evaluates post-repair objectives.
- **Purple slot eligibility differs by statue kind**: Attack→{Justice, Grace, Stats}; Defense→{Justice, Grace, Soul, Virtue, Mercy}. Static lookup, not runtime filter.
- **`Cost to Next Tier` at T20 is empty** — guard tier-up moves.
- **T1-equivalent cost** = `1 + TotalCost(N)`, NOT `Cost to Next Tier(N)`. For N=1: `1 + TotalCost(1) = 1 + 1 = 2`.
- **`PvP DMG Bonus`** exists both as a feather stat and as a flat set-bonus — they aggregate, must not overwrite.
- **Sparse stat columns**: parse as `Option<f64>`, then `unwrap_or(0.0)` into the dense `StatVec`.
- **Higher-tier inventory rows**: a row `Space, 5, 10` contributes `10 * (1 + TotalCost(5))` = `10 × 38 = 380` T1-equivalent units to the STDN pool — applied at load time.
- **Missing feather types**: `input.csv` has only 6 of 18 feather types. Missing types contribute 0 to pool but can be placed if the set pool has budget. The eligible-feather list must include all feathers of the correct type/rarity/statue-kind whose set pool is non-zero.

---

## Implementation order
1. **`Cargo.toml`**: add all dependencies, feature flags.
2. **`data/`** + CSV loaders + parsing tests. Unit tests: parse all CSVs without panic; verify row counts.
3. **`domain/`**: `StatId` enum (17 variants), `StatVec=[f64;17]`, `Tier(u8)`, `Set` enum, `FeatherDef`, `Statue`, `Inventory`. Unit tests: tier construction panics on 0; inventory consume/restore.
4. **`eval/`** + golden unit tests (lock the math first).
5. **`solver/common/repair.rs`** + neighborhood primitives + their tests.
6. **`solver/sa.rs`** single-threaded, then add rayon multi-start.
7. TUI shell with mocked `SolverEvent`s; wire SA.
8. **`solver/bnb.rs`** Stage-1 enumeration with admissible UB; integrate.
9. **`solver/bnb_lp.rs`** LP relaxation; feature-gate.
10. Cross-solver agreement test on a shrunk inventory; tune SA cooling schedule.

---

## Verification plan
- **Unit tests** (`eval/evaluator.rs`):
  - `t1_uniform_attack_statue_score` — 5 known feathers all T1, hand-computed vs evaluator.
  - `min_tier_drives_set_bonus` — 4 slots at T20, one at T1; bonus must equal T1 row.
  - `flat_added_after_pct` — fixed raw + 50% pct + flat 100; assert `raw*1.5 + 100`.
  - `cost_consumption_t1_equiv` — one T5 Space slot consumes `1 + TotalCost(5) = 1 + 37 = 38` from STDN budget.
  - `fractional_stat_parse` — Light T1 PDMG = 0.28 (not truncated to 0).
- **Property tests** (proptest): random legal solutions never exceed inventory; `greedy_consume` always satisfies the fully-consumed constraint.
- **Golden file**: `tests/fixtures/tiny_inventory.csv` + expected solver output JSON, pinned by seed.
- **Cross-solver agreement**: shrink inventory so Stage-1 enumeration is feasible (~10⁴ nodes); BnB returns optimum; SA with 32 restarts × 30s reaches within 1% on 5/5 runs.
- **Manual smoke**: load real `input.csv` against all 4 presets; sanity-check that `offensive_pve` produces high PvE DMG Bonus on attack statues with high min-tier.

---

## Critical files (where the interesting logic lives)
- `src/domain/stats.rs` — `StatId` enum (17 variants) and `StatVec=[f64;17]`; any error here corrupts everything downstream
- `src/domain/feather.rs` — type/rarity/set enums and per-statue legality predicates
- `src/eval/set_bonus_table.rs` — two separate table types for attack vs defense; category-to-percentage mapping
- `src/eval/evaluator.rs` — the hot path; scale-then-add order is paramount
- `src/solver/common/repair.rs` — guarantees the "budget fully consumed" hard constraint
- `src/solver/sa.rs` — Solver A
- `src/solver/bnb.rs` (+ `bnb_lp.rs`) — Solver B
- `src/tui/app.rs` — wiring solver events into the UI
