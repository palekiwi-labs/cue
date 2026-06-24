use std::sync::mpsc::SyncSender;

use crossterm::event::{self, Event, KeyCode, KeyEventKind};

use crate::app::View;
use crate::event::Action;
use crate::msg::Msg;

/// Spawn a dedicated input thread that translates crossterm events into
/// [`Msg`] variants and posts them to the unified channel.
///
/// The thread runs until the receiver is dropped (i.e. the main loop exits),
/// at which point a send error is treated as a clean shutdown signal.
pub fn spawn(tx: SyncSender<Msg>) {
    std::thread::spawn(move || loop {
        // `event::read()` blocks indefinitely — no polling overhead.
        let ev = match event::read() {
            Ok(ev) => ev,
            Err(_) => break,
        };

        let msg = match ev {
            Event::Key(key) => {
                // Ignore key-release events (Windows fires these).
                if key.kind != KeyEventKind::Press {
                    continue;
                }
                let action = map_key(key.code);
                Msg::Input(action)
            }
            Event::Resize(_, _) => Msg::Redraw,
            _ => continue,
        };

        if tx.send(msg).is_err() {
            break;
        }
    });
}

/// Map a raw [`KeyCode`] to a high-level [`Action`].
fn map_key(code: KeyCode) -> Action {
    match code {
        KeyCode::Char('q') | KeyCode::Char('Q') => Action::Quit,
        KeyCode::Char('j') | KeyCode::Down => Action::Down,
        KeyCode::Char('k') | KeyCode::Up => Action::Up,
        KeyCode::Char('h') | KeyCode::Left => Action::Left,
        KeyCode::Char('l') | KeyCode::Right => Action::Right,
        KeyCode::Char('1') => Action::SwitchView(View::Kanban),
        KeyCode::Char('2') => Action::SwitchView(View::Activity),
        KeyCode::Char('3') => Action::SwitchView(View::Diagnostics),
        KeyCode::Char('r') => Action::Refresh,
        _ => Action::None,
    }
}
