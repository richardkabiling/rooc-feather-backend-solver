# rooc-feather-solver

## Download

Pre-built binaries are available from the [latest release](../../releases/latest). Download the asset for your platform:

| Asset | Platform |
|-------|----------|
| `rooc-feather-solver-linux-x86_64` | Linux (x86-64) |
| `rooc-feather-solver-macos-x86_64` | macOS (Intel) |
| `rooc-feather-solver-macos-arm64`  | macOS (Apple Silicon) |
| `rooc-feather-solver-windows-x86_64.exe` | Windows (x86-64) |

On Linux/macOS, make the binary executable after downloading:

```sh
chmod +x rooc-feather-solver
```

## Quick start

1. Place your inventory in `input.csv` (see [Inventory](#inventory--inputcsv) below).
2. Put the `data/` directory from this repo alongside the binary.
3. Run:

```sh
./rooc-feather-solver --preset offensive_pvp --input input.csv --time 60
```

Results are written to `best_solution.txt` and `best_solution.json` in the working directory.

## Input

### Inventory — `input.csv`

One row per feather stack you own. Can be any tier between 1 and 20:

```csv
Feather, Tier, Count
Space, 1, 1171
Light, 1, 763
Justice, 1, 1319
```

| Column    | Description |
|-----------|-------------|
| `Feather` | Feather name (case-sensitive: `Space`, `Time`, `Day`, `Sky`, `Divine`, `Nature`, `Night`, `Terra`, `Light`, `Dark`, `Justice`, `Grace`, `Stats`, `Soul`, `Virtue`, `Mercy`) |
| `Tier`    | Tier of the feathers in this stack (1–20) |
| `Count`   | Number of feathers in this stack |

Non-T1 feathers are converted to T1-equivalents using the cumulative upgrade cost from `data/feather-stats.csv`.

### Data directory — `data/`

| File | Contents |
|------|----------|
| `feather-stats.csv` | Per-feather, per-tier stats and upgrade costs |
| `attack-stats.csv` / `defense-stats.csv` | Base statue stat tables |
| `attack-set-bonuses.csv` / `defense-set-bonuses.csv` | Set bonus tables |
| `pvp-stats.csv` / `pve-stats.csv` | PvP/PvE modifier tables |
| `presets.csv` | Named stat-weight presets |

### Presets

A preset assigns a relative **weight** to each stat. The solver maximises the weighted sum of final statue stats, so higher-weighted stats are prioritised when allocating feathers and tiers.

Weights are normalised internally by each stat's maximum possible T20 value, so a weight of `3` on `PDMG` and a weight of `3` on `IgnorePDEF` carry equal effective importance regardless of the raw magnitude difference between those stats.

Presets are defined in `data/presets.csv` (columns: `preset`, `stat`, `weight`). Stats omitted from a preset have an implicit weight of 0. Available presets:

| Preset | Focus |
|--------|-------|
| `offensive_pvp` | PvP damage output |
| `offensive_pve` | PvE damage output |
| `defensive_pvp` | PvP survivability |
| `defensive_pve` | PvE survivability |
| `shallow_weighting_offensive_pvp` | PvP damage, flatter weight distribution |
| `custom_offensive_pvp` | Custom PvP offensive tuning |

## Usage

```
rooc-feather-solver [OPTIONS] --preset <PRESET>

Options:
  -p, --preset <PRESET>              Preset name to use for stat weights
  -i, --input <INPUT>                Path to inventory CSV [default: input.csv]
  -d, --data <DATA>                  Path to the data directory [default: data]
      --time <TIME>                  Time budget in seconds [default: 30]
      --restarts <RESTARTS>          Number of SA restarts/chains [default: 8]
      --threads <THREADS>            Number of threads [default: 4]
      --seed <SEED>                  Random seed [default: 42]
      --log-every <LOG_EVERY>        Log progress every N iterations per chain [default: 10000]
      --share-interval <N>           Share best solution across chains every N iterations [default: 50000]
  -h, --help                         Print help
```

Longer runs with more threads generally find better solutions:

```sh
./rooc-feather-solver \
  --preset offensive_pvp \
  --time 300 \
  --restarts 16 \
  --threads 8
```

## Output

Results are written to:

- `best_solution.txt` — human-readable breakdown with per-statue and per-stat scores
- `best_solution.json` — machine-readable `{ "attack": [...], "defense": [...] }` where each statue is a list of `[FeatherName, tier]` pairs

## Building from source

Requires [Rust](https://rustup.rs/) (stable).

```sh
cargo build --release
```

The binary is at `target/release/rooc-feather-solver`.


### Inventory — `input.csv`

One row per feather stack you own. Can be any tier between 1 and 20:

```csv
Feather, Tier, Count
Space, 1, 1171
Light, 1, 763
Justice, 1, 1319
```

| Column    | Description |
|-----------|-------------|
| `Feather` | Feather name (case-sensitive: `Space`, `Time`, `Day`, `Sky`, `Divine`, `Nature`, `Night`, `Terra`, `Light`, `Dark`, `Justice`, `Grace`, `Stats`, `Soul`, `Virtue`, `Mercy`) |
| `Tier`    | Tier of the feathers in this stack (1–20) |
| `Count`   | Number of feathers in this stack |

Non-T1 feathers are converted to T1-equivalents using the cumulative upgrade cost from `data/feather-stats.csv`.

### Data directory — `data/`

| File | Contents |
|------|----------|
| `feather-stats.csv` | Per-feather, per-tier stats and upgrade costs |
| `attack-stats.csv` / `defense-stats.csv` | Base statue stat tables |
| `attack-set-bonuses.csv` / `defense-set-bonuses.csv` | Set bonus tables |
| `pvp-stats.csv` / `pve-stats.csv` | PvP/PvE modifier tables |
| `presets.csv` | Named stat-weight presets |

### Presets

A preset assigns a relative **weight** to each stat. The solver maximises the weighted sum of final statue stats, so higher-weighted stats are prioritised when allocating feathers and tiers.

Weights are normalised internally by each stat's maximum possible T20 value, so a weight of `3` on `PDMG` and a weight of `3` on `IgnorePDEF` carry equal effective importance regardless of the raw magnitude difference between those stats.

Presets are defined in `data/presets.csv` (columns: `preset`, `stat`, `weight`). Stats omitted from a preset have an implicit weight of 0. Available presets:

| Preset | Focus |
|--------|-------|
| `offensive_pvp` | PvP damage output |
| `offensive_pve` | PvE damage output |
| `defensive_pvp` | PvP survivability |
| `defensive_pve` | PvE survivability |
| `shallow_weighting_offensive_pvp` | PvP damage, flatter weight distribution |
| `custom_offensive_pvp` | Custom PvP offensive tuning |

## Usage

```
rooc-feather-solver [OPTIONS] --preset <PRESET>

Options:
  -p, --preset <PRESET>              Preset name to use for stat weights
  -i, --input <INPUT>                Path to inventory CSV [default: input.csv]
  -d, --data <DATA>                  Path to the data directory [default: data]
      --time <TIME>                  Time budget in seconds [default: 30]
      --restarts <RESTARTS>          Number of SA restarts/chains [default: 8]
      --threads <THREADS>            Number of threads [default: 4]
      --seed <SEED>                  Random seed [default: 42]
      --log-every <LOG_EVERY>        Log progress every N iterations per chain [default: 10000]
      --share-interval <N>           Share best solution across chains every N iterations [default: 50000]
  -h, --help                         Print help
```

### Example

```sh
./target/release/rooc-feather-solver \
  --preset offensive_pvp \
  --input input.csv \
  --time 60 \
  --restarts 16 \
  --threads 8
```

## Output

Results are written to:

- `best_solution.txt` — human-readable breakdown with per-statue stats
- `best_solution.json` — machine-readable `{ "attack": [...], "defense": [...] }` where each statue is a list of `[FeatherName, tier]` pairs
