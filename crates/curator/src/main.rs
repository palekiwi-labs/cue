// curator: TUI kanban board for cue artifacts.
mod app;
mod event;
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

    let root = cli
        .root
        .unwrap_or_else(|| env::current_dir().expect("cannot read cwd"));

    let tasks = read_artifacts(&root, &cli.branch, "task")?;

    let mut app = App::new(tasks);
    let mut terminal = tui::init()?;

    let result = run(&mut terminal, &mut app);

    tui::restore()?;
    result
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
            Action::None => {}
        }

        if app.should_quit {
            break;
        }
    }
    Ok(())
}
