use std::path::PathBuf;
use clap::Parser;
use anyhow::Result;

use rooc_feather_solver::data::{GameData, loader::load_inventory};
use rooc_feather_solver::tui::app::{run, save_solution};
use rooc_feather_solver::solver::SolverConfig;

#[derive(Parser, Debug)]
#[command(name = "rooc-feather-solver", about = "Feather statue optimizer")]
struct Cli {
    /// Path to the data directory (default: ./data)
    #[arg(short, long, default_value = "data")]
    data: PathBuf,

    /// Path to inventory CSV (default: ./input.csv)
    #[arg(short, long, default_value = "input.csv")]
    input: PathBuf,

    /// Preset name to use for stat weights (e.g. offensive_pvp)
    #[arg(short, long)]
    preset: Option<String>,

    /// Time budget in seconds for the solver
    #[arg(long, default_value_t = 30)]
    time: u64,

    /// Random seed
    #[arg(long, default_value_t = 42)]
    seed: u64,

    /// Number of SA restarts
    #[arg(long, default_value_t = 8)]
    restarts: usize,

    /// Number of threads
    #[arg(long, default_value_t = 4)]
    threads: usize,

    /// Log progress every N iterations per chain
    #[arg(long, default_value_t = 10_000)]
    log_every: u64,

    /// Share best solution across chains every N iterations
    #[arg(long, default_value_t = 50_000)]
    share_interval: u64,
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    let preset = cli.preset.ok_or_else(|| anyhow::anyhow!("--preset is required"))?;

    let game = GameData::load(&cli.data)?;
    let inventory = load_inventory(&cli.input, &game.feathers)?;

    let config = SolverConfig {
        time_budget_secs: cli.time,
        seed:             cli.seed,
        restarts:         cli.restarts,
        threads:          cli.threads,
        log_every:        cli.log_every,
        share_interval:   cli.share_interval,
    };

    let solution = run(game, inventory, &preset, config)?;
    save_solution(&solution)?;
    eprintln!("[info] solution saved to best_solution.txt and best_solution.json");

    Ok(())
}
