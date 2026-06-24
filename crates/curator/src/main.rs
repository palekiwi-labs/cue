// curator: TUI kanban board for cue artifacts.
mod app;
mod event;
mod input;
mod msg;
mod sse;
mod tui;
mod ui;

use anyhow::Result;
use app::App;
use clap::Parser;
use cuelib::artifact::read_artifacts;
use event::{Action, next_action};
use std::{env, path::PathBuf};

#[derive(Parser)]
#[command(
    name = "curator",
    about = "TUI kanban board for cue task artifacts",
    version
)]
struct Cli {
    /// Root of the cue project (defaults to current working directory).
    #[arg(long, value_name = "DIR")]
    root: Option<PathBuf>,

    /// Branch to read tasks from (default: master).
    #[arg(long, default_value = "master")]
    branch: String,
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    let root = match cli.root {
        Some(p) => p,
        None => env::current_dir()?,
    };

    let tasks = read_artifacts(&root, &cli.branch, "task")?;

    // Install a panic hook that restores the terminal before printing the
    // panic message, so the backtrace is readable and the shell is not left
    // in raw mode.
    let default_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let _ = tui::restore();
        default_hook(info);
    }));

    let mut app = App::new(tasks);
    let mut terminal = tui::init()?;

    let run_result = run(&mut terminal, &mut app);

    // Always restore the terminal, even if run() returned an error.
    // Preserve and return the run error if restore succeeds.
    let restore_result = tui::restore();
    run_result?;
    restore_result?;
    Ok(())
}

fn run(terminal: &mut tui::Tui, app: &mut App) -> Result<()> {
    loop {
        terminal.draw(|frame| ui::render(frame, app))?;

        match next_action()? {
            Action::Quit => break,
            Action::Down => app.scroll_down(),
            Action::Up => app.scroll_up(),
            Action::Left => app.move_left(),
            Action::Right => app.move_right(),
            // Handled in Slice 5 unified run loop; no-op for now.
            Action::SwitchView(_) | Action::Refresh | Action::None => {}
        }
    }
    Ok(())
}
