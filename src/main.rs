use std::path::PathBuf;
use clap::Parser;
use anyhow::Result;

use rooc_feather_solver::data::{GameData, loader::load_inventory};
use rooc_feather_solver::tui::app::App;

#[derive(Parser, Debug)]
#[command(name = "rooc-feather-solver", about = "Feather statue optimizer")]
struct Cli {
    /// Path to the data directory (default: ./data)
    #[arg(short, long, default_value = "data")]
    data: PathBuf,

    /// Path to inventory CSV (default: ./input.csv)
    #[arg(short, long, default_value = "input.csv")]
    input: PathBuf,
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    let game = GameData::load(&cli.data)?;
    let inventory = load_inventory(&cli.input, &game.feathers)?;

    let app = App::new(game, inventory);
    app.run()?;

    Ok(())
}
