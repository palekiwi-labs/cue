use crate::app::View;

/// High-level actions the run loop dispatches on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    Quit,
    Down,
    Up,
    Left,
    Right,
    /// Switch the active view (keys 1/2/3).
    SwitchView(View),
    /// Force-reload artifacts from disk (`r` key).
    Refresh,
    None,
}
