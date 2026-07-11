// curator: TUI kanban board for cue artifacts.
mod activity;
mod app;
mod event;
mod input;
mod msg;
mod sse;
mod tui;
mod ui;

use anyhow::Result;
use app::{ActivityLayout, App, View};
use clap::Parser;
use event::Action;
use msg::{Msg, SseStatus};
use std::sync::mpsc::Receiver;

#[derive(Parser)]
#[command(
    name = "curator",
    about = "TUI kanban board for cue task artifacts",
    version
)]
struct Cli {
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

    let store = cuelib::project::ProjectStore::load().unwrap_or_default();
    let tasks = app::collect_tasks(&store, &cli.branch);

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

    // Drop the original tx so that when all spawned thread senders have
    // exited, rx.recv() returns Err and run() can exit cleanly via that path.
    drop(tx);

    let mut app = App::new(tasks);
    app.kanban_empty_store = store.entries().is_empty();

    let mut terminal = tui::init()?;

    let run_result = run(&mut terminal, &mut app, rx, &cli.branch);

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
    branch: &str,
) -> Result<()> {
    loop {
        terminal.draw(|frame| ui::render(frame, app))?;

        // Block until at least one message arrives, then drain all pending.
        // This gives instant keyboard response and batches SSE bursts.
        match rx.recv() {
            Ok(msg) => {
                if process_msg(msg, app, branch)? == LoopControl::Quit {
                    return Ok(());
                }
            }
            Err(_) => {
                // All senders dropped — both leaf threads have exited.
                // This is the clean-shutdown path when threads terminate
                // before a Quit message is received.
                return Ok(());
            }
        }
        while let Ok(msg) = rx.try_recv() {
            if process_msg(msg, app, branch)? == LoopControl::Quit {
                return Ok(());
            }
        }
    }
}

fn process_msg(msg: Msg, app: &mut App, branch: &str) -> Result<LoopControl> {
    match msg {
        Msg::Input(Action::Quit) => return Ok(LoopControl::Quit),
        Msg::Input(Action::SwitchView(v)) => app.active_view = v,
        Msg::Input(Action::Refresh) => reload_tasks(app, branch)?,
        Msg::Input(Action::Down) => match app.active_view {
            View::Kanban => app.scroll_down(),
            View::Activity => match app.activity_layout {
                ActivityLayout::DetailFull => app.scroll_down_activity(),
                _ => app.scroll_down_sessions(),
            },
            View::Diagnostics => app.scroll_down_diagnostics(),
        },
        Msg::Input(Action::Up) => match app.active_view {
            View::Kanban => app.scroll_up(),
            View::Activity => match app.activity_layout {
                ActivityLayout::DetailFull => app.scroll_up_activity(),
                _ => app.scroll_up_sessions(),
            },
            View::Diagnostics => app.scroll_up_diagnostics(),
        },
        Msg::Input(Action::Left) => {
            // Left navigates kanban columns only. No-op in Activity (user spec)
            // and Diagnostics.
            if app.active_view == View::Kanban {
                app.move_left();
            }
        }
        Msg::Input(Action::Right) => match app.active_view {
            View::Kanban => app.move_right(),
            View::Activity => app.enter_detail_full(),
            View::Diagnostics => {}
        },
        Msg::Input(Action::Enter) => {
            if app.active_view == View::Activity {
                app.toggle_detail_pane();
            }
        }
        Msg::Input(Action::Escape) => {
            if app.active_view == View::Activity {
                app.return_from_detail_full();
            }
        }
        Msg::Input(Action::None) => {}
        Msg::Redraw => {}
        Msg::Sse(record) => {
            app.push_event(record);
            app.ensure_session_selection();
        }
        Msg::SseStatus(s) => app.acuity_status = s.into(),
    }
    Ok(LoopControl::Continue)
}

fn reload_tasks(app: &mut App, branch: &str) -> Result<()> {
    let store = cuelib::project::ProjectStore::load().unwrap_or_default();
    let tasks = app::collect_tasks(&store, branch);
    app.kanban_empty_store = store.entries().is_empty();
    app.reload_kanban(tasks);
    Ok(())
}
