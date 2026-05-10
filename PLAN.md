# Multithreaded Feather Solver — Plan & Spec

## Context
The repo is a fresh project containing only game data (`data/*.csv`), an inventory file (`input.csv`), and rules (`CLAUDE.md`). No source code exists yet.

We are building a Rust crate that, given a feather inventory and a chosen objective preset, computes the best assignment of 50 feathers across 10 statues (5 Attack + 5 Defense) — selecting which feathers go where and what tier each is at — subject to per-set inventory budgets that must be **fully consumed**. The crate exposes a `Solver` trait so multiple solvers can be plugged in and compared; we ship two to start: a multi-start parallel local-search solver (SA) and an exact branch-and-bound solver. Interaction is via a ratatui TUI with live progress.

This unblocks comparing tier-up strategies under realistic budget pressure (LD and Purple sets are shared between attack and defense statues, so naive per-statue greedy fails) and gives a foundation for adding more solvers later.

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
`PvEDmgBonus`→"PvE DMG Bonus", `PvEDmgReduction`→"PvE DMG Reduction", `PvPDmgBonus`→"PvP DMG Bonus", `PvPDmgReduction`→"PvP DMG Reduction", `PDMGReduction`, `MDMGReduction`, `IgnorePDEF`→"Ignore PDEF", `IgnoreMDEF`→"Ignore MDEF", `INTDEXSTR`→"INT/DEX/STR", and bare `PATK/MATK/PDMG/MDMG/PDEF/MDEF/HP/VIT`.

## Crate layout (`src/`)

```
main.rs                          CLI parse, load data, launch TUI
lib.rs                           re-exports for integration tests
data/{mod, schema, loader}.rs    raw serde rows + load_all()
domain/
  feather.rs                     FeatherId, FeatherDef, Type, Set, Rarity, Tier(1..=20)
  stats.rs                       StatVec=[f64;16], StatId enum, Category
  statue.rs                      StatueKind, Slot, Statue
  inventory.rs                   per-Set T1-equivalent budgets
  preset.rs                      Preset {name, weights:StatVec}, token normalization
  solution.rs                    Solution {statues:[Statue;10], objective, breakdown}
eval/
  feather_table.rs               [feather][tier] -> StatVec, cumulative cost
  set_bonus_table.rs             [tier] -> (pct_vec, flat_vec) for attack & defense
  evaluator.rs                   statue_score, solution_score (hot path)
solver/
  mod.rs                         pub trait Solver, SolverConfig, SolverEvent, ProgressTx
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

## Domain & evaluator design (the math has to be right)
- `Tier(u8)` newtype enforced 1..=20 at construction.
- `StatVec = [f64; 16]` indexed by compile-time `StatId`. Stat math is fused-multiply-add over the 16 lanes — far faster than `HashMap` and `Copy`.
- `Inventory` = `[u64; 5]` keyed by `Set`. `consume(set, t1_units) -> Result<()>` mutates; cheap to snapshot/restore for backtracking.
- `Statue { kind, slots: [Slot; 5] }` — slot 0 by convention is the purple slot.
- Per-tier feather stats and set-bonus rows are **precomputed once** into dense tables. Inner loop:
  1. sum 5 feather `StatVec`s,
  2. find `min_tier`,
  3. lookup `(pct, flat)`,
  4. `final[i] = raw[i] * (1 + pct[i]/100) + flat[i]`,
  5. dot-product with `preset.weights`.
- Optional micro-opt: pre-fold preset weights into the per-tier bonus tables so step 5 collapses into step 4.

## `Solver` trait

```rust
pub trait Solver: Send + Sync {
    fn name(&self) -> &str;
    fn solve(&self, ctx: &SolveContext, tx: ProgressTx) -> Solution;
}
pub struct SolveContext<'a> {
    pub game: &'a GameData,
    pub eval: &'a Evaluator,
    pub preset: &'a Preset,
    pub inventory: Inventory,
    pub config: SolverConfig,        // time_budget, seed, restarts, threads, solver-specific knobs
    pub cancel: Arc<AtomicBool>,
}
pub enum SolverEvent {
    Progress { iter: u64, best_obj: f64, budget_used: [u64; 5] },
    NewBest(Solution),
    Done(Solution),
}
```

`ProgressTx` is a `crossbeam_channel::Sender<SolverEvent>` with a bounded buffer; the TUI drops on full to avoid blocking the solver.

## Solver A — Multi-start parallel SA
- **Seed**: greedy fill — assign 4 oranges per statue maximizing per-feather marginal weight at T1, pick purple, run repair.
- **Neighborhoods** (sampled with weights):
  1. Swap one orange in a statue for a different legal orange.
  2. Swap the purple slot's feather.
  3. ±1 tier on a single slot.
  4. Cross-statue feather swap (same kind, or both Hybrid).
  5. **Tier-min lift**: +1 tier on the statue's current min-tier slot (drives set bonus).
- Acceptance: Metropolis. Cooling: geometric, T₀ scaled to mean |Δ| of 100 random moves.
- **Critical**: every accepted move runs `greedy_consume` to satisfy the "fully consumed" constraint, and SA evaluates the **post-repair** objective (otherwise it oscillates between feasible/infeasible states).
- **Parallelism**: `rayon::scope` runs N independent chains with distinct seeds; pick global best. Each chain holds a private `Candidate` with O(1) delta scoring (only the affected statue is rescored).

## Solver B — Branch and bound (rayon work-stealing)
Two-stage decomposition:
1. **Configuration enumeration**: choose the 5 feathers per statue (ignore tiers). Per-statue counts: Attack 45 configs, Defense 75. Coupling across statues is via shared budgets, so we enumerate jointly with bounding, not as a Cartesian product.
2. **Tier assignment**: given fixed configurations, decide tiers for all 50 slots subject to ≤5 per-set budget constraints + "fully consumed" rule.

Bounds:
- **Stage 2 LP relaxation**: relax tiers to continuous in [1,20]; assume `min_tier=20` for set bonus (admissible — bonus is concave in min_tier, so this overestimates). Solve via `good_lp`+CBC (50 vars, ~6 constraints — milliseconds), feature-gated behind `--features bnb-lp` so default builds don't need CBC.
- **Stage 1 admissible UB** for partial configurations: fixed statues use their LP bound; undecided statues use their per-statue tier-20 best score (assumes infinite budget).

Warm-start the BnB incumbent with Solver A's result — Stage 1 worst-case is 45⁵ × 75⁵ ≈ 7×10¹⁶, so pruning is mandatory.

`rayon::iter::ParallelBridge` over Stage-1 nodes; shared `AtomicU64` incumbent (bit-cast f64) for cross-thread pruning.

## TUI (ratatui)
Single screen, three rows:
- **Top (30%)**: 3 side-by-side panels — inventory (per-set budget bars), preset picker, solver picker + config form (time budget, restarts, seed).
- **Middle (40%)**: live progress — sparkline of best-objective-over-time, gauge for elapsed/time-budget, counters (iters/sec, best obj, current obj, % budget consumed per set).
- **Bottom (30%)**: result table — 10 rows (statue kind, feathers, tiers, min tier, statue obj), sorted by objective desc.

Event loop: `crossterm` polls keys; solver thread sends `SolverEvent`s via channel; redraw at 30 Hz max. Keys: `Tab` cycle panels, `Enter` start, `q` quit, `s` save best to JSON.

## Crates (`Cargo.toml`)
```
ratatui = "0.28"
crossterm = "0.28"
rayon = "1.10"
crossbeam-channel = "0.5"
csv = "1.3"
serde = { version = "1", features = ["derive"] }
anyhow = "1"
thiserror = "1"
clap = { version = "4", features = ["derive"] }
rand = "0.8"
rand_xoshiro = "0.6"
good_lp = { version = "1.8", features = ["coin_cbc"], optional = true }
```

## Critical files (where the interesting logic lives)
- `src/domain/feather.rs` — type/rarity/set enums and per-statue legality predicates
- `src/eval/evaluator.rs` — the hot path; correctness here is paramount
- `src/solver/common/repair.rs` — guarantees the "budget fully consumed" hard constraint
- `src/solver/sa.rs` — Solver A
- `src/solver/bnb.rs` (+ `bnb_lp.rs`) — Solver B
- `src/tui/app.rs` — wiring solver events into the UI

## Subtle pitfalls (do not re-discover)
- **Min-tier set bonus**: tier-up of a non-min slot doesn't help the bonus. SA needs the explicit "tier-min lift" move or it gets trapped.
- **Shared budgets**: LD and Purple sets are consumed by both attack and defense statues. Repair must allocate globally per set, not per statue.
- **Fully-consumed is hard, not soft**: every emitted `Solution` must pass a post-repair check. SA evaluates post-repair objectives.
- **Purple slot eligibility differs by statue kind**: Attack→{Justice, Grace, Stats}; Defense→{Justice, Grace, Soul, Virtue, Mercy}. Static lookup, not runtime filter.
- **`Cost to Next Tier` at T20 is empty** — guard tier-up moves.
- **T1-equivalent cost** = `1 + TotalCost(N)`, NOT `Cost to Next Tier(N)`. Easy off-by-one.
- **`PvP DMG Bonus`** exists both as a feather stat and as a flat set-bonus — they aggregate, must not overwrite.
- **Sparse stat columns**: parse as `Option<f64>`, then `unwrap_or(0.0)` into the dense `StatVec`.
- **Higher-tier inventory rows**: a row `Space, 5, 10` contributes `10 * (1 + TotalCost(5))` T1-equivalent units to the STDN pool — applied at load time.

## Implementation order
1. `data/` + `domain/` + CSV loaders + parsing tests.
2. `eval/` + golden unit tests (lock the math first).
3. `solver/common/repair.rs` + neighborhood primitives + their tests.
4. `solver/sa.rs` single-threaded, then add rayon multi-start.
5. TUI shell with mocked `SolverEvent`s; wire SA.
6. `solver/bnb.rs` Stage-1 enumeration with admissible UB; integrate.
7. `solver/bnb_lp.rs` LP relaxation; feature-gate.
8. Cross-solver agreement test on a shrunk inventory; tune SA cooling schedule.

## Verification plan
- **Unit tests** (`eval/evaluator.rs`):
  - `t1_uniform_attack_statue_score` — 5 known feathers all T1, hand-computed vs evaluator.
  - `min_tier_drives_set_bonus` — 4 slots at T20, one at T1; bonus must equal T1 row.
  - `flat_added_after_pct` — fixed raw + 50% pct + flat 100; assert `raw*1.5 + 100`.
  - `cost_consumption_t1_equiv` — one T5 Space slot consumes `1 + TotalCost(5) = 38` from STDN budget.
- **Property tests** (proptest): random legal solutions never exceed inventory; `greedy_consume` always satisfies the fully-consumed constraint.
- **Golden file**: `tests/fixtures/tiny_inventory.csv` + expected solver output JSON, pinned by seed.
- **Cross-solver agreement**: shrink inventory so Stage-1 enumeration is feasible (~10⁴ nodes); BnB returns optimum; SA with 32 restarts × 30s reaches within 1% on 5/5 runs.
- **Manual smoke**: load real `input.csv` against all 4 presets; sanity-check that `offensive_pve` produces high PvE DMG Bonus on attack statues with high min-tier.
