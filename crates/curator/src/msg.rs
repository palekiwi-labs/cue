use acuity_api::EventRecord;

use crate::event::Action;

/// Unified message type posted into the main run-loop channel.
///
/// Two producer threads — the crossterm input thread and the SSE thread —
/// both send `Msg` variants over a single `std::sync::mpsc::SyncSender`.
/// The main loop blocks on `rx.recv()`, drains all pending messages, then
/// redraws once.
#[derive(Debug)]
pub enum Msg {
    /// A key or mouse action from the input thread.
    Input(Action),
    /// Terminal resize event — triggers a redraw without changing app state.
    Redraw,
    /// A live event record received from the acuity SSE stream.
    Sse(EventRecord),
    /// A change in the SSE connection state.
    SseStatus(SseStatus),
}

/// Connection state of the SSE stream to acuity.
#[derive(Debug, Clone)]
pub enum SseStatus {
    /// Successfully connected and receiving events.
    Connected,
    /// Attempting to (re-)connect; `attempt` is 1-based.
    Reconnecting { attempt: u32 },
    /// No acuity URL was configured — SSE is intentionally disabled.
    Disabled,
}
