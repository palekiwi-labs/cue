use anyhow::Result;
use crossterm::event::{self, Event, KeyCode, KeyEventKind};
use std::time::Duration;

/// High-level actions the event loop can return to the main run loop.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    Quit,
    Down,
    Up,
    Left,
    Right,
    None,
}

/// Poll for a single key event with a short timeout.
///
/// Returns [`Action::None`] if no event arrives within the tick interval or
/// if the event is not a recognised keypress.
pub fn next_action() -> Result<Action> {
    if event::poll(Duration::from_millis(100))?
        && let Event::Key(key) = event::read()?
    {
        // Only process key-press events; ignore key-release on Windows.
        if key.kind != KeyEventKind::Press {
            return Ok(Action::None);
        }
        let action = match key.code {
            KeyCode::Char('q') | KeyCode::Char('Q') => Action::Quit,
            KeyCode::Char('j') | KeyCode::Down => Action::Down,
            KeyCode::Char('k') | KeyCode::Up => Action::Up,
            KeyCode::Char('h') | KeyCode::Left => Action::Left,
            KeyCode::Char('l') | KeyCode::Right => Action::Right,
            _ => Action::None,
        };
        return Ok(action);
    }
    Ok(Action::None)
}
