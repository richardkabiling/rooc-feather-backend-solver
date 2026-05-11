use std::collections::HashMap;
use std::sync::{Arc, atomic::{AtomicBool, Ordering}};
use std::thread;
use std::time::{Duration, Instant};

use crossbeam_channel::{bounded, Receiver};
use ratatui::{
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{
        Block, Borders, Gauge, List, ListItem, Paragraph, Row, Table, Cell,
    },
    Terminal, Frame,
};
use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};

use crate::data::GameData;
use crate::data::loader::load_inventory;
use crate::domain::feather::FeatherId;
use crate::domain::inventory::Inventory;
use crate::domain::preset::Preset;
use crate::domain::solution::Solution;
use crate::eval::evaluator::Evaluator;
use crate::solver::{Solver, SolveContext, SolverConfig, SolverEvent};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AppState {
    Setup,
    Running,
    Done,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Panel {
    Inventory,
    Presets,
    SolverConfig,
}

pub struct App {
    pub game:          GameData,
    pub inventory:     Inventory,
    pub presets:       Vec<String>,
    pub selected_preset: usize,
    pub selected_solver: usize,
    pub config:        SolverConfig,
    pub state:         AppState,
    pub active_panel:  Panel,
    pub best_solution: Option<Solution>,
    pub best_obj_history: Vec<f64>,
    pub iters:         u64,
    pub start_time:    Option<Instant>,
}

impl App {
    pub fn new(game: GameData, inventory: Inventory) -> Self {
        let mut preset_names: Vec<String> = game.presets.keys().cloned().collect();
        preset_names.sort();
        App {
            game,
            inventory,
            presets: preset_names,
            selected_preset: 0,
            selected_solver: 0,
            config: SolverConfig::default(),
            state: AppState::Setup,
            active_panel: Panel::Presets,
            best_solution: None,
            best_obj_history: Vec::new(),
            iters: 0,
            start_time: None,
        }
    }

    pub fn run(mut self) -> anyhow::Result<()> {
        enable_raw_mode()?;
        let mut stdout = std::io::stdout();
        execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
        let backend = CrosstermBackend::new(stdout);
        let mut terminal = Terminal::new(backend)?;

        let tick = Duration::from_millis(33); // ~30 Hz
        let mut solver_rx: Option<Receiver<SolverEvent>> = None;
        let cancel = Arc::new(AtomicBool::new(false));

        loop {
            terminal.draw(|f| draw_ui(f, &self))?;

            if event::poll(tick)? {
                if let Event::Key(key) = event::read()? {
                    match key.code {
                        KeyCode::Char('q') => break,
                        KeyCode::Tab => {
                            self.active_panel = match self.active_panel {
                                Panel::Inventory    => Panel::Presets,
                                Panel::Presets      => Panel::SolverConfig,
                                Panel::SolverConfig => Panel::Inventory,
                            };
                        }
                        KeyCode::Up => {
                            if matches!(self.active_panel, Panel::Presets) && self.selected_preset > 0 {
                                self.selected_preset -= 1;
                            }
                        }
                        KeyCode::Down => {
                            if matches!(self.active_panel, Panel::Presets) && self.selected_preset + 1 < self.presets.len() {
                                self.selected_preset += 1;
                            }
                        }
                        KeyCode::Enter if self.state == AppState::Setup => {
                            // Launch solver
                            let preset_name = &self.presets[self.selected_preset];
                            let preset = self.game.presets[preset_name].clone();
                            let inv    = self.inventory.clone();
                            let config = self.config.clone();
                            let cancel2 = cancel.clone();
                            let (tx, rx) = bounded::<SolverEvent>(128);

                            let feathers = self.game.feathers.clone();
                            let attack_bonuses  = self.game.attack_bonuses.clone();
                            let defense_bonuses = self.game.defense_bonuses.clone();

                            thread::spawn(move || {
                                use crate::eval::feather_table::FeatherTable;
                                use crate::eval::set_bonus_table::{build_attack_table, build_defense_table};
                                use crate::eval::normalizer::{compute_norm_factors, effective_weights};
                                use crate::solver::sa::SimulatedAnnealing;
                                use crate::data::GameData;

                                let ft = FeatherTable::new(feathers);
                                let atk_tbl = build_attack_table(&attack_bonuses);
                                let def_tbl = build_defense_table(&defense_bonuses);
                                let norm = compute_norm_factors(&ft);
                                let ew   = effective_weights(&preset.weights, &norm);
                                let eval = Evaluator {
                                    feather_table: ft,
                                    attack_bonuses: atk_tbl,
                                    defense_bonuses: def_tbl,
                                    eff_weights: ew,
                                };
                                let game_data = GameData {
                                    feathers: Vec::new(), // not needed in thread
                                    attack_bonuses: Vec::new(),
                                    defense_bonuses: Vec::new(),
                                    presets: HashMap::new(),
                                };
                                let ctx = SolveContext {
                                    game: &game_data,
                                    eval: &eval,
                                    preset: &preset,
                                    inventory: inv,
                                    config,
                                    cancel: cancel2,
                                };
                                SimulatedAnnealing.solve(&ctx, tx);
                            });

                            solver_rx = Some(rx);
                            self.state = AppState::Running;
                            self.start_time = Some(Instant::now());
                        }
                        KeyCode::Char('s') => {
                            if let Some(sol) = &self.best_solution {
                                // Save to JSON
                                let _ = save_solution(sol);
                            }
                        }
                        _ => {}
                    }
                }
            }

            // Drain solver events
            if let Some(rx) = &solver_rx {
                while let Ok(ev) = rx.try_recv() {
                    match ev {
                        SolverEvent::Progress { iter, best_obj, .. } => {
                            self.iters = iter;
                            self.best_obj_history.push(best_obj);
                            if self.best_obj_history.len() > 100 {
                                self.best_obj_history.remove(0);
                            }
                        }
                        SolverEvent::NewBest(sol) => {
                            self.best_solution = Some(*sol);
                        }
                        SolverEvent::Done(sol) => {
                            self.best_solution = Some(*sol);
                            self.state = AppState::Done;
                        }
                    }
                }
            }
        }

        disable_raw_mode()?;
        execute!(terminal.backend_mut(), LeaveAlternateScreen, DisableMouseCapture)?;
        terminal.show_cursor()?;
        Ok(())
    }
}

fn draw_ui(f: &mut Frame, app: &App) {
    let area = f.area();
    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage(30),
            Constraint::Percentage(40),
            Constraint::Percentage(30),
        ])
        .split(area);

    // Top row: inventory | presets | solver config
    draw_top(f, app, rows[0]);
    // Middle: progress
    draw_progress(f, app, rows[1]);
    // Bottom: results
    draw_results(f, app, rows[2]);
}

fn draw_top(f: &mut Frame, app: &App, area: Rect) {
    let cols = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(33),
            Constraint::Percentage(34),
            Constraint::Percentage(33),
        ])
        .split(area);

    // Inventory panel
    {
        use crate::domain::feather::{Set, SET_COUNT};
        let set_names = ["STDN", "DN", "ST", "LD", "Purple"];
        let items: Vec<ListItem> = set_names.iter().enumerate().map(|(i, name)| {
            let budget = app.inventory.budget[i];
            ListItem::new(format!("{}: {}", name, budget))
        }).collect();
        let border = if app.active_panel == Panel::Inventory { Style::default().fg(Color::Yellow) } else { Style::default() };
        let list = List::new(items).block(Block::default().borders(Borders::ALL).border_style(border).title("Inventory"));
        f.render_widget(list, cols[0]);
    }

    // Preset picker
    {
        let items: Vec<ListItem> = app.presets.iter().enumerate().map(|(i, name)| {
            let style = if i == app.selected_preset { Style::default().fg(Color::Green).add_modifier(Modifier::BOLD) } else { Style::default() };
            ListItem::new(name.as_str()).style(style)
        }).collect();
        let border = if app.active_panel == Panel::Presets { Style::default().fg(Color::Yellow) } else { Style::default() };
        let list = List::new(items).block(Block::default().borders(Borders::ALL).border_style(border).title("Presets"));
        f.render_widget(list, cols[1]);
    }

    // Solver config
    {
        let solver_names = ["Simulated Annealing", "Branch and Bound"];
        let text = format!(
            "Solver: {}\nTime: {}s\nRestarts: {}\nThreads: {}\nSeed: {}\n\n[Enter] to start\n[s] save result\n[q] quit",
            solver_names[app.selected_solver % 2],
            app.config.time_budget_secs,
            app.config.restarts,
            app.config.threads,
            app.config.seed,
        );
        let border = if app.active_panel == Panel::SolverConfig { Style::default().fg(Color::Yellow) } else { Style::default() };
        let p = Paragraph::new(text).block(Block::default().borders(Borders::ALL).border_style(border).title("Solver Config"));
        f.render_widget(p, cols[2]);
    }
}

fn draw_progress(f: &mut Frame, app: &App, area: Rect) {
    let elapsed = app.start_time.map(|t| t.elapsed().as_secs()).unwrap_or(0);
    let total   = app.config.time_budget_secs.max(1);
    let ratio   = (elapsed as f64 / total as f64).min(1.0);
    let best    = app.best_solution.as_ref().map(|s| s.objective).unwrap_or(0.0);

    let text = format!(
        "Iterations: {}  Best: {:.2}  Elapsed: {}s / {}s\nState: {:?}",
        app.iters, best, elapsed, total, app.state
    );
    let p = Paragraph::new(text).block(Block::default().borders(Borders::ALL).title("Progress"));
    f.render_widget(p, area);
}

fn draw_results(f: &mut Frame, app: &App, area: Rect) {
    if let Some(sol) = &app.best_solution {
        let rows: Vec<Row> = sol.statues.iter().enumerate().map(|(i, statue)| {
            let feathers: String = statue.slots.iter().map(|s| format!("{:?}(T{})", s.feather, s.tier.get())).collect::<Vec<_>>().join(", ");
            let score = format!("{:.2}", sol.statue_scores[i]);
            Row::new(vec![
                Cell::from(format!("{:?}", statue.kind)),
                Cell::from(feathers),
                Cell::from(score),
            ])
        }).collect();
        let table = Table::new(rows, [Constraint::Length(10), Constraint::Min(40), Constraint::Length(10)])
            .header(Row::new(vec!["Kind", "Feathers", "Score"]).style(Style::default().add_modifier(Modifier::BOLD)))
            .block(Block::default().borders(Borders::ALL).title(format!("Result (Total: {:.2})", sol.objective)));
        f.render_widget(table, area);
    } else {
        let p = Paragraph::new("No solution yet. Press [Enter] to start.").block(Block::default().borders(Borders::ALL).title("Result"));
        f.render_widget(p, area);
    }
}

fn save_solution(sol: &Solution) -> anyhow::Result<()> {
    use std::fs::File;
    use std::io::Write;

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
    Ok(())
}
