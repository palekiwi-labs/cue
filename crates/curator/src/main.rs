// curator: TUI kanban board for cue artifacts.
mod app;
mod event;
mod input;
mod msg;
mod sse;
mod tui;
mod ui;

use anyhow::Result;
use app::{App, View};
use clap::Parser;
use cuelib::artifact::read_artifacts;
use event::Action;
use msg::{Msg, SseStatus};
use std::{
    env,
    path::{Path, PathBuf},
    sync::mpsc::Receiver,
};

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

    /// Base URL of the acuity server (e.g. http://localhost:3030).
    /// Falls back to $ACUITY_URL if not set. Omit to run in kanban-only mode.
    #[arg(long, env = "ACUITY_URL", value_name = "URL")]
    acuity_url: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum LoopControl {
    Continue,
    Quit,
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

    let (tx, rx) = std::sync::mpsc::sync_channel::<Msg>(4096);

    // Always spawn the input thread.
    input::spawn(tx.clone());

    // Spawn the SSE thread only when an acuity URL is configured; otherwise
    // notify the app immediately that SSE is disabled.
    if let Some(url) = cli.acuity_url {
        sse::spawn(url, tx.clone());
    } else {
        let _ = tx.send(Msg::SseStatus(SseStatus::Disabled));
    }

    let mut app = App::new(tasks);
    let mut terminal = tui::init()?;

    let run_result = run(&mut terminal, &mut app, rx, &root, &cli.branch);

    // Always restore the terminal, even if run() returned an error.
    // Preserve and return the run error if restore succeeds.
    let restore_result = tui::restore();
    run_result?;
    restore_result?;
    Ok(())
}

fn run(
    terminal: &mut tui::Tui,
    app: &mut App,
    rx: Receiver<Msg>,
    root: &Path,
    branch: &str,
) -> Result<()> {
    loop {
        terminal.draw(|frame| ui::render(frame, app))?;

        // Block until at least one message arrives, then drain all pending.
        // This gives instant keyboard response and batches SSE bursts.
        match rx.recv() {
            Ok(msg) => {
                if process_msg(msg, app, root, branch)? == LoopControl::Quit {
                    return Ok(());
                }
            }
            Err(_) => break, // all senders dropped — shouldn't happen normally
        }
        while let Ok(msg) = rx.try_recv() {
            if process_msg(msg, app, root, branch)? == LoopControl::Quit {
                return Ok(());
            }
        }
    }
    Ok(())
}

fn process_msg(msg: Msg, app: &mut App, root: &Path, branch: &str) -> Result<LoopControl> {
    match msg {
        Msg::Input(Action::Quit) => return Ok(LoopControl::Quit),
        Msg::Input(Action::SwitchView(v)) => app.active_view = v,
        Msg::Input(Action::Refresh) => reload_tasks(app, root, branch)?,
        Msg::Input(Action::Down) => match app.active_view {
            View::Kanban => app.scroll_down(),
            View::Activity => app.scroll_down_activity(),
            View::Diagnostics => app.scroll_down_diagnostics(),
        },
        Msg::Input(Action::Up) => match app.active_view {
            View::Kanban => app.scroll_up(),
            View::Activity => app.scroll_up_activity(),
            View::Diagnostics => app.scroll_up_diagnostics(),
        },
        Msg::Input(Action::Left) => app.move_left(),
        Msg::Input(Action::Right) => app.move_right(),
        Msg::Input(Action::None) => {}
        Msg::Redraw => {}
        Msg::Sse(record) => app.push_event(record),
        Msg::SseStatus(s) => app.acuity_status = s.into(),
    }
    Ok(LoopControl::Continue)
}

fn reload_tasks(app: &mut App, root: &Path, branch: &str) -> Result<()> {
    let tasks = read_artifacts(root, branch, "task")?;
    app.reload_kanban(tasks);
    Ok(())
}
