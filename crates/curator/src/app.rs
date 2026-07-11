use std::collections::{HashMap, VecDeque};
use std::path::PathBuf;

use acuity_api::{AcuityEvent, EventRecord};
use cuelib::artifact::{ArtifactMeta, TaskStatus};
use cuelib::project::ProjectStore;

use crate::msg::SseStatus;

/// Which top-level view is currently displayed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum View {
    Kanban,
    Activity,
    Diagnostics,
}

/// Three-state layout for the Activity view.
///
/// Transitions:
/// - `SessionsFull` ↔ Enter ↔ `Split`
/// - `SessionsFull` | `Split` + Right → `DetailFull`
/// - `DetailFull` + Escape → `Split`
/// - Left: no-op in Activity view (user spec)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActivityLayout {
    /// Default: sessions list fullscreen. j/k navigates sessions.
    SessionsFull,
    /// Split: sessions pane left (focused, j/k), detail pane right (static).
    Split,
    /// Detail pane fullscreen. j/k navigates events list.
    DetailFull,
}

/// Which kanban column is currently active.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Column {
    Open,
    InProgress,
    Complete,
}

impl Column {
    pub fn title(&self) -> &'static str {
        match self {
            Self::Open => "Open",
            Self::InProgress => "In Progress",
            Self::Complete => "Complete",
        }
    }

    pub fn left(&self) -> Self {
        match self {
            Self::Open => Self::Open,
            Self::InProgress => Self::Open,
            Self::Complete => Self::InProgress,
        }
    }

    pub fn right(&self) -> Self {
        match self {
            Self::Open => Self::InProgress,
            Self::InProgress => Self::Complete,
            Self::Complete => Self::Complete,
        }
    }
}

/// Connection status of the acuity SSE stream, as surfaced in the UI.
///
/// Mirrors [`SseStatus`] from `msg.rs` but lives in `app.rs` so the app
/// state is decoupled from the message transport type.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AcuityStatus {
    Disabled,
    Connected,
    Reconnecting { attempt: u32 },
}

impl From<SseStatus> for AcuityStatus {
    fn from(s: SseStatus) -> Self {
        match s {
            SseStatus::Connected => AcuityStatus::Connected,
            SseStatus::Reconnecting { attempt } => AcuityStatus::Reconnecting { attempt },
            SseStatus::Disabled => AcuityStatus::Disabled,
        }
    }
}

/// A kanban card with project attribution.
///
/// Wraps [`ArtifactMeta`] and adds which project (key + root path) the task
/// belongs to, as loaded from the [`ProjectStore`].
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct KanbanTask {
    /// The underlying artifact metadata.
    pub meta: ArtifactMeta,
    /// Key of the project this task belongs to (from ProjectStore).
    pub project_key: String,
    /// Filesystem root of the project (first path in ProjectStore).
    pub project_root: PathBuf,
}

/// Per-session aggregate built incrementally by [`App::push_event`].
///
/// Lives in [`App::sessions`] and is updated *before* ring-buffer eviction,
/// so totals survive even when old events are evicted from the ring buffer.
#[derive(Debug, Clone)]
pub struct SessionSummary {
    /// Session identifier; read by sorted_sessions and rendering code.
    pub session_id: String,
    pub project_dir: String,
    pub session_title: Option<String>,
    /// ISO-8601 timestamp of the earliest received event for this session.
    /// Read by sorted_sessions for newest-first ordering.
    pub first_seen: String,
    /// ISO-8601 timestamp of the most recently received event for this session.
    pub last_seen: String,
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub error_count: u32,
    /// Harness identifier, e.g. `"opencode"`. Populated from every event.
    pub harness: String,
    /// The session that spawned this one (sub-agent back-edge). `None` for
    /// primary sessions. Set-once-if-some: a later null does not clobber it.
    pub parent_id: Option<String>,
    /// Agent identifier, e.g. `"claude"`. Last-writer-wins.
    pub agent: Option<String>,
    /// Flat `"providerID/modelID"` string. Last-writer-wins.
    pub model: Option<String>,
    /// Count of visible (non-hidden) events for this session currently in
    /// the ring buffer. Maintained incrementally in `push_event` (increment
    /// on append, decrement on eviction) so `session_event_len` is O(1).
    pub visible_event_count: u32,
    /// Unique agent identifiers seen for this session, in first-seen order.
    /// Accumulated in `push_event`; survives ring-buffer eviction (unlike
    /// the old scan-the-ring-buffer approach which lost data on eviction).
    pub unique_agents: Vec<String>,
    /// Unique model identifiers seen for this session, in first-seen order.
    /// Accumulated in `push_event` from both `session_updated` and
    /// `agent_turn_completed` events.
    pub unique_models: Vec<String>,
}

/// Maximum number of events retained in the ring buffer.
pub const EVENT_CAP: usize = 2_000;

/// Application state for the curator kanban board.
pub struct App {
    // --- Kanban ---
    /// Tasks in the Open column.
    pub open: Vec<KanbanTask>,
    /// Tasks in the In Progress column.
    pub in_progress: Vec<KanbanTask>,
    /// Tasks in the Complete column.
    pub complete: Vec<KanbanTask>,
    /// True when the ProjectStore had no entries at the last load.
    pub kanban_empty_store: bool,

    /// Active (focused) column.
    pub active_col: Column,

    /// Selected item index within each column.
    pub sel_open: usize,
    pub sel_in_progress: usize,
    pub sel_complete: usize,

    // --- Live views ---
    /// Which top-level view is currently displayed.
    pub active_view: View,

    /// SSE connection status surfaced in the status bar.
    pub acuity_status: AcuityStatus,

    /// Ring buffer of live events — oldest at front, newest at back.
    /// Capped at [`EVENT_CAP`] entries.
    pub events: VecDeque<EventRecord>,

    /// Per-session aggregates, updated incrementally in [`App::push_event`]
    /// before ring-buffer eviction so totals survive eviction.
    pub sessions: HashMap<String, SessionSummary>,

    /// Selection index for the Activity Feed view (indexes the visible events
    /// of the selected session).
    pub sel_activity: usize,
    /// Selection index for the Diagnostics view.
    pub sel_diagnostics: usize,

    // --- Two-pane Activity view state ---
    /// Layout state of the Activity view.
    pub activity_layout: ActivityLayout,
    /// Identity-based session selection: tracks the session_id of the selected
    /// session in Pane 1. `None` until at least one event has arrived.
    pub sel_session_id: Option<String>,
}

impl App {
    /// Classify `tasks` into kanban columns using the typed `TaskStatus` from
    /// `cuelib`. Tasks whose status parses as `Closed`, or whose status field
    /// is absent / unrecognised, are silently excluded (they are not kanban-
    /// visible by definition).
    pub fn new(tasks: Vec<KanbanTask>) -> Self {
        let (open, in_progress, complete) = classify_tasks(tasks);

        Self {
            open,
            in_progress,
            complete,
            kanban_empty_store: false,
            active_col: Column::Open,
            sel_open: 0,
            sel_in_progress: 0,
            sel_complete: 0,

            active_view: View::Kanban,
            acuity_status: AcuityStatus::Disabled,
            events: VecDeque::new(),
            sessions: HashMap::new(),
            sel_activity: 0,
            sel_diagnostics: 0,
            activity_layout: ActivityLayout::SessionsFull,
            sel_session_id: None,
        }
    }

    /// Re-classify a new set of tasks into the kanban columns, resetting all
    /// column selection indices. Called on `Action::Refresh`.
    pub fn reload_kanban(&mut self, tasks: Vec<KanbanTask>) {
        let (open, in_progress, complete) = classify_tasks(tasks);
        self.open = open;
        self.in_progress = in_progress;
        self.complete = complete;
        self.sel_open = 0;
        self.sel_in_progress = 0;
        self.sel_complete = 0;
    }

    /// Ingest one live event: update the session summary map, evict from the
    /// ring buffer if at capacity, then append.
    ///
    /// Session summaries are updated *before* eviction so they survive even
    /// when old events age out of the ring buffer.
    pub fn push_event(&mut self, record: EventRecord) {
        // --- 1. Update session summary ---
        let entry = self
            .sessions
            .entry(record.session_id.clone())
            .or_insert_with(|| SessionSummary {
                session_id: record.session_id.clone(),
                project_dir: String::new(),
                session_title: None,
                first_seen: record.received_at.clone(),
                last_seen: record.received_at.clone(),
                input_tokens: 0,
                output_tokens: 0,
                error_count: 0,
                harness: record.harness.clone(),
                parent_id: None,
                agent: None,
                model: None,
                visible_event_count: 0,
                unique_agents: Vec::new(),
                unique_models: Vec::new(),
            });

        // Always update last_seen if this event is newer.
        if record.received_at > entry.last_seen {
            entry.last_seen.clone_from(&record.received_at);
        }

        // Always reflect the ingest-time project_dir from EventRecord (set at
        // DB ingest from the event payload). Non-empty on all events since
        // Slice 2. Updating unconditionally ensures non-idle events populate
        // the summary even when no SessionIdle has arrived yet.
        if !record.project_dir.is_empty() {
            entry.project_dir.clone_from(&record.project_dir);
        }

        match record.event_type.as_str() {
            "session_idle" => {
                if let Ok(AcuityEvent::SessionIdle(ev)) =
                    serde_json::from_str::<AcuityEvent>(&record.payload)
                {
                    // project_dir already updated unconditionally above.
                    entry.session_title.clone_from(&ev.session_title);
                    // first_seen is only set once (on insert).
                }
            }
            "session_updated" => {
                if let Ok(AcuityEvent::SessionUpdated(ev)) =
                    serde_json::from_str::<AcuityEvent>(&record.payload)
                {
                    // Envelope + last-writer-wins metadata.
                    entry.harness.clone_from(&ev.harness);
                    if let Some(v) = &ev.title {
                        entry.session_title = Some(v.clone());
                    }
                    if let Some(v) = &ev.agent {
                        entry.agent = Some(v.clone());
                        if !entry.unique_agents.contains(v) {
                            entry.unique_agents.push(v.clone());
                        }
                    }
                    if let Some(v) = &ev.model {
                        entry.model = Some(v.clone());
                        if !entry.unique_models.contains(v) {
                            entry.unique_models.push(v.clone());
                        }
                    }
                    // parent_id is set-once-if-some: a later null must not
                    // clobber a known parent (mirrors the project_dir guard).
                    if entry.parent_id.is_none() && ev.parent_id.is_some() {
                        entry.parent_id = ev.parent_id.clone();
                    }
                }
            }
            "agent_turn_completed" => {
                if let Ok(AcuityEvent::AgentTurnCompleted(ev)) =
                    serde_json::from_str::<AcuityEvent>(&record.payload)
                {
                    entry.input_tokens += ev.input_tokens.unwrap_or(0) as u64;
                    entry.output_tokens += ev.output_tokens.unwrap_or(0) as u64;
                    if let Some(m) = &ev.model {
                        entry.model = Some(m.clone());
                        if !entry.unique_models.contains(m) {
                            entry.unique_models.push(m.clone());
                        }
                    }
                }
            }
            "tool_call_completed" => {
                if let Ok(AcuityEvent::ToolCallCompleted(ev)) =
                    serde_json::from_str::<AcuityEvent>(&record.payload)
                    && ev.is_error
                {
                    entry.error_count += 1;
                }
            }
            _ => {}
        }

        // --- 2. Evict oldest if at capacity ---
        if self.events.len() >= EVENT_CAP
            && let Some(evicted) = self.events.pop_front()
            && !crate::ui::is_hidden_in_activity(&evicted.event_type)
            && let Some(s) = self.sessions.get_mut(&evicted.session_id)
        {
            s.visible_event_count = s.visible_event_count.saturating_sub(1);
        }

        // --- 3. Append + maintain visible count ---
        let is_visible = !crate::ui::is_hidden_in_activity(&record.event_type);
        let rec_session_id = record.session_id.clone();
        self.events.push_back(record);
        if is_visible
            && let Some(s) = self.sessions.get_mut(&rec_session_id)
        {
            s.visible_event_count += 1;
        }
    }

    /// Number of events that pass the Diagnostics filter (`tool_call_*`).
    pub fn diagnostics_len(&self) -> usize {
        self.events
            .iter()
            .filter(|e| e.event_type.starts_with("tool_call_"))
            .count()
    }

    /// Number of events visible in the Activity Feed (i.e. not hidden).
    ///
    /// Hides `session_updated` rows whose payload is already absorbed into
    /// `SessionSummary` before render. Mirrors `diagnostics_len`.
    ///
    /// Kept for its two unit tests; no longer called in production (the
    /// two-pane view uses `session_event_len` per session instead).
    #[allow(dead_code)]
    pub fn activity_len(&self) -> usize {
        self.events
            .iter()
            .filter(|e| !crate::ui::is_hidden_in_activity(&e.event_type))
            .count()
    }

    /// Number of visible (non-hidden) events for a single session.
    ///
    /// Reads from the cached `visible_event_count` on `SessionSummary`
    /// (maintained incrementally in `push_event`) — O(1), not a ring scan.
    /// Used by the two-pane Events pane to clamp `sel_activity`.
    pub fn session_event_len(&self, session_id: &str) -> usize {
        self.sessions
            .get(session_id)
            .map(|s| s.visible_event_count as usize)
            .unwrap_or(0)
    }

    /// Return a sorted list of sessions that have at least one visible event.
    ///
    /// Sorted newest-`first_seen` first, then `session_id` ascending as a
    /// deterministic tiebreak (HashMap iteration order is non-deterministic).
    pub fn sorted_sessions(&self) -> Vec<&SessionSummary> {
        let mut sessions: Vec<&SessionSummary> = self
            .sessions
            .values()
            .filter(|s| self.session_event_len(&s.session_id) > 0)
            .collect();
        // Newest first; tiebreak by session_id for determinism.
        sessions.sort_by(|a, b| {
            b.first_seen
                .cmp(&a.first_seen)
                .then_with(|| a.session_id.cmp(&b.session_id))
        });
        sessions
    }

    /// Ensure a valid session is selected in Pane 1.
    ///
    /// If `sel_session_id` is `None` or its session has no visible events,
    /// auto-select the first session from `sorted_sessions()` and reset
    /// `sel_activity = 0`. Safe to call after every event arrival —
    /// idempotent when a valid session is already selected.
    pub fn ensure_session_selection(&mut self) {
        let needs_reset = match &self.sel_session_id {
            None => true,
            Some(id) => self.session_event_len(id) == 0,
        };
        if needs_reset {
            self.sel_session_id = self
                .sorted_sessions()
                .first()
                .map(|s| s.session_id.clone());
            self.sel_activity = 0;
        } else if let Some(id) = self.sel_session_id.as_deref() {
            // Clamp sel_activity against the current visible event count so a
            // stale index (from ring-buffer eviction shrinking the session's
            // visible events below sel_activity) doesn't strand the highlight.
            // The renderer masks this via .min(len-1), but the stored index
            // must be corrected or scroll-up appears broken.
            let len = self.session_event_len(id);
            if len > 0 {
                self.sel_activity = self.sel_activity.min(len - 1);
            }
        }
    }

    /// Move the session selection down in Pane 1 (one step toward older sessions).
    ///
    /// Updates `sel_session_id` to the next session in `sorted_sessions()` and
    /// resets `sel_activity = 0`. No-op when at the last session or list is empty.
    pub fn scroll_down_sessions(&mut self) {
        let sessions = self.sorted_sessions();
        if sessions.is_empty() {
            return;
        }
        let current_pos = self
            .sel_session_id
            .as_deref()
            .and_then(|id| sessions.iter().position(|s| s.session_id == id));
        let next_pos = match current_pos {
            Some(pos) if pos + 1 < sessions.len() => pos + 1,
            Some(_) => return, // already at last
            None => 0,
        };
        self.sel_session_id = Some(sessions[next_pos].session_id.clone());
        self.sel_activity = 0;
    }

    /// Move the session selection up in Pane 1 (one step toward newer sessions).
    ///
    /// Updates `sel_session_id` to the previous session and resets
    /// `sel_activity = 0`. No-op when at the first session or list is empty.
    pub fn scroll_up_sessions(&mut self) {
        let sessions = self.sorted_sessions();
        if sessions.is_empty() {
            return;
        }
        let current_pos = self
            .sel_session_id
            .as_deref()
            .and_then(|id| sessions.iter().position(|s| s.session_id == id));
        let prev_pos = match current_pos {
            Some(0) => return, // already at first
            Some(pos) => pos - 1,
            None => 0,
        };
        self.sel_session_id = Some(sessions[prev_pos].session_id.clone());
        self.sel_activity = 0;
    }

    /// Toggle the detail pane visibility: `SessionsFull ↔ Split`.
    /// No-op when in `DetailFull` (Escape handles the return from there).
    pub fn toggle_detail_pane(&mut self) {
        self.activity_layout = match self.activity_layout {
            ActivityLayout::SessionsFull => ActivityLayout::Split,
            ActivityLayout::Split => ActivityLayout::SessionsFull,
            ActivityLayout::DetailFull => ActivityLayout::DetailFull,
        };
    }

    /// Enter the fullscreen detail view from any other layout state.
    /// No-op if already in `DetailFull`.
    pub fn enter_detail_full(&mut self) {
        match self.activity_layout {
            ActivityLayout::SessionsFull | ActivityLayout::Split => {
                self.activity_layout = ActivityLayout::DetailFull;
            }
            ActivityLayout::DetailFull => {}
        }
    }

    /// Return from fullscreen detail to the split layout.
    /// No-op if not in `DetailFull`.
    pub fn return_from_detail_full(&mut self) {
        if self.activity_layout == ActivityLayout::DetailFull {
            self.activity_layout = ActivityLayout::Split;
        }
    }

    /// Move the selection down within the active column.
    pub fn scroll_down(&mut self) {
        match self.active_col {
            Column::Open => {
                let len = self.open.len();
                if len > 0 {
                    self.sel_open = (self.sel_open + 1).min(len - 1);
                }
            }
            Column::InProgress => {
                let len = self.in_progress.len();
                if len > 0 {
                    self.sel_in_progress = (self.sel_in_progress + 1).min(len - 1);
                }
            }
            Column::Complete => {
                let len = self.complete.len();
                if len > 0 {
                    self.sel_complete = (self.sel_complete + 1).min(len - 1);
                }
            }
        }
    }

    /// Move the selection up within the active column.
    pub fn scroll_up(&mut self) {
        match self.active_col {
            Column::Open => {
                self.sel_open = self.sel_open.saturating_sub(1);
            }
            Column::InProgress => {
                self.sel_in_progress = self.sel_in_progress.saturating_sub(1);
            }
            Column::Complete => {
                self.sel_complete = self.sel_complete.saturating_sub(1);
            }
        }
    }

    /// Switch the active column to the left.
    pub fn move_left(&mut self) {
        self.active_col = self.active_col.left();
    }

    /// Switch the active column to the right.
    pub fn move_right(&mut self) {
        self.active_col = self.active_col.right();
    }

    /// Return the tasks for a given column.
    pub fn column_tasks(&self, col: Column) -> &[KanbanTask] {
        match col {
            Column::Open => &self.open,
            Column::InProgress => &self.in_progress,
            Column::Complete => &self.complete,
        }
    }

    /// Return the current selection index for a given column.
    pub fn column_sel(&self, col: Column) -> usize {
        match col {
            Column::Open => self.sel_open,
            Column::InProgress => self.sel_in_progress,
            Column::Complete => self.sel_complete,
        }
    }

    /// Move the Activity Feed (Events pane) selection down.
    ///
    /// Clamps at `session_event_len(sel_session_id)-1` — the last visible row
    /// for the selected session. No-op when no session is selected or the
    /// session has no visible events.
    pub fn scroll_down_activity(&mut self) {
        let len = self
            .sel_session_id
            .as_deref()
            .map(|id| self.session_event_len(id))
            .unwrap_or(0);
        if len > 0 {
            self.sel_activity = (self.sel_activity + 1).min(len - 1);
        }
    }

    /// Move the Activity Feed selection up.
    pub fn scroll_up_activity(&mut self) {
        self.sel_activity = self.sel_activity.saturating_sub(1);
    }

    /// Move the Diagnostics view selection down (tool-call events only).
    pub fn scroll_down_diagnostics(&mut self) {
        let len = self.diagnostics_len();
        if len > 0 {
            self.sel_diagnostics = (self.sel_diagnostics + 1).min(len - 1);
        }
    }

    /// Move the Diagnostics view selection up.
    pub fn scroll_up_diagnostics(&mut self) {
        self.sel_diagnostics = self.sel_diagnostics.saturating_sub(1);
    }
}

// ---------------------------------------------------------------------------
// Private helpers
// ---------------------------------------------------------------------------

/// Rank a priority string as a sortable integer (lower = higher priority).
fn priority_rank(p: Option<&str>) -> u8 {
    match p {
        Some("critical") => 0,
        Some("high") => 1,
        Some("low") => 3,
        _ => 2, // "normal" or absent
    }
}

/// Classify a flat task list into (open, in_progress, complete) column vecs,
/// each sorted by priority (critical → high → normal → low).
///
/// Closed or unrecognised statuses are silently excluded — not kanban-visible.
fn classify_tasks(
    tasks: Vec<KanbanTask>,
) -> (Vec<KanbanTask>, Vec<KanbanTask>, Vec<KanbanTask>) {
    let mut open = Vec::new();
    let mut in_progress = Vec::new();
    let mut complete = Vec::new();

    for task in tasks {
        match task.meta.status::<TaskStatus>() {
            Some(TaskStatus::Open) => open.push(task),
            Some(TaskStatus::InProgress) => in_progress.push(task),
            Some(TaskStatus::Complete) => complete.push(task),
            Some(TaskStatus::Closed) | None => {}
        }
    }

    open.sort_by_key(|t| priority_rank(t.meta.priority_raw.as_deref()));
    in_progress.sort_by_key(|t| priority_rank(t.meta.priority_raw.as_deref()));
    complete.sort_by_key(|t| priority_rank(t.meta.priority_raw.as_deref()));

    (open, in_progress, complete)
}

/// Collect task cards from all projects registered in `store`.
///
/// For each project key, only the **first** path is used (D3: single-path
/// per project). Projects whose root directory is missing or unreadable are
/// silently skipped.
pub fn collect_tasks(store: &ProjectStore, branch: &str) -> Vec<KanbanTask> {
    let mut tasks = Vec::new();
    for (key, paths) in store.entries() {
        let root = match paths.first() {
            Some(p) => p,
            None => continue,
        };
        let metas = match cuelib::artifact::read_artifacts(root, branch, "task") {
            Ok(m) => m,
            Err(_) => continue,
        };
        for meta in metas {
            tasks.push(KanbanTask {
                meta,
                project_key: key.clone(),
                project_root: root.clone(),
            });
        }
    }
    tasks
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use acuity_schema::{
        AcuityEvent, AgentTurnCompleted, SessionIdle, SessionUpdated, ToolCallCompleted,
        ToolCallRequested,
    };

    // --- Helpers ---

    fn make_record(seq: i64, session_id: &str, event: AcuityEvent) -> EventRecord {
        EventRecord {
            seq,
            received_at: format!("2026-01-01T00:00:{seq:02}Z"),
            event_type: event.event_type().to_string(),
            session_id: session_id.to_string(),
            turn_id: event.turn_id().map(str::to_string),
            project_dir: event.project_dir().to_string(),
            harness: event.harness().to_string(),
            payload: serde_json::to_string(&event).unwrap(),
        }
    }

    fn idle(seq: i64, session_id: &str, project_dir: &str) -> EventRecord {
        make_record(
            seq,
            session_id,
            AcuityEvent::SessionIdle(SessionIdle {
                session_id: session_id.to_string(),
                project_dir: project_dir.to_string(),
                harness: "opencode".to_string(),
                session_title: Some(format!("proj-{project_dir}")),
            }),
        )
    }

    fn turn(seq: i64, session_id: &str, input: u32, output: u32) -> EventRecord {
        make_record(
            seq,
            session_id,
            AcuityEvent::AgentTurnCompleted(AgentTurnCompleted {
                session_id: session_id.to_string(),
                turn_id: "t1".to_string(),
                project_dir: "/home/pl/code".to_string(),
                harness: "opencode".to_string(),
                input_tokens: Some(input),
                output_tokens: Some(output),
                model: Some("anthropic/claude-sonnet".to_string()),
            }),
        )
    }

    fn tool_done(seq: i64, session_id: &str, is_error: bool) -> EventRecord {
        make_record(
            seq,
            session_id,
            AcuityEvent::ToolCallCompleted(ToolCallCompleted {
                session_id: session_id.to_string(),
                turn_id: "t1".to_string(),
                project_dir: "/home/pl/code".to_string(),
                harness: "opencode".to_string(),
                tool_call_id: format!("c{seq}"),
                tool_name: "bash".to_string(),
                is_error,
                error_text: if is_error {
                    Some("fail".to_string())
                } else {
                    None
                },
            }),
        )
    }

    fn tool_req(seq: i64, session_id: &str) -> EventRecord {
        make_record(
            seq,
            session_id,
            AcuityEvent::ToolCallRequested(ToolCallRequested {
                session_id: session_id.to_string(),
                turn_id: "t1".to_string(),
                project_dir: "/home/pl/code".to_string(),
                harness: "opencode".to_string(),
                tool_call_id: format!("c{seq}"),
                tool_name: "bash".to_string(),
                args: serde_json::Value::Null,
            }),
        )
    }

    fn session_updated(
        seq: i64,
        session_id: &str,
        parent_id: Option<&str>,
        agent: Option<&str>,
        model: Option<&str>,
        title: Option<&str>,
    ) -> EventRecord {
        make_record(
            seq,
            session_id,
            AcuityEvent::SessionUpdated(SessionUpdated {
                session_id: session_id.to_string(),
                project_dir: "/home/pl/code".to_string(),
                harness: "opencode".to_string(),
                parent_id: parent_id.map(str::to_string),
                agent: agent.map(str::to_string),
                model: model.map(str::to_string),
                title: title.map(str::to_string),
            }),
        )
    }

    fn empty_app() -> App {
        App::new(vec![])
    }

    // --- push_event: eviction ---

    #[test]
    fn ring_buffer_capped_at_event_cap() {
        let mut app = empty_app();
        for i in 0..=(EVENT_CAP as i64) {
            app.push_event(idle(i, "s1", "/p"));
        }
        assert_eq!(app.events.len(), EVENT_CAP);
    }

    #[test]
    fn ring_buffer_oldest_evicted_first() {
        let mut app = empty_app();
        for i in 0..=(EVENT_CAP as i64) {
            app.push_event(idle(i, "s1", "/p"));
        }
        // Oldest event (seq 0) should be gone; newest still present.
        assert_eq!(app.events.front().unwrap().seq, 1);
        assert_eq!(app.events.back().unwrap().seq, EVENT_CAP as i64);
    }

    // --- push_event: token accumulation ---

    #[test]
    fn token_accumulation_sums_across_turns() {
        let mut app = empty_app();
        app.push_event(turn(1, "s1", 100, 200));
        app.push_event(turn(2, "s1", 50, 75));
        let s = &app.sessions["s1"];
        assert_eq!(s.input_tokens, 150);
        assert_eq!(s.output_tokens, 275);
    }

    // --- push_event: project attribution survives eviction ---

    #[test]
    fn project_dir_survives_ring_buffer_eviction() {
        let mut app = empty_app();
        // First event is a SessionIdle that sets project_dir.
        app.push_event(idle(0, "s1", "/my/project"));
        // Push EVENT_CAP more events so the idle is evicted. Each turn carries
        // "/home/pl/code" in its EventRecord.project_dir (see the `turn` helper).
        // Since push_event now updates project_dir from every event, the final
        // value reflects the most recent event rather than the evicted idle.
        for i in 1..=(EVENT_CAP as i64) {
            app.push_event(turn(i, "s1", 1, 1));
        }
        // The ring buffer no longer contains seq=0. The sessions map still has
        // a non-empty project_dir sourced from the subsequent turn events.
        assert!(!app.events.iter().any(|e| e.seq == 0));
        assert_eq!(app.sessions["s1"].project_dir, "/home/pl/code");
    }

    // --- push_event: error counting ---

    #[test]
    fn error_count_increments_on_is_error_true() {
        let mut app = empty_app();
        app.push_event(tool_done(1, "s1", true));
        app.push_event(tool_done(2, "s1", false));
        app.push_event(tool_done(3, "s1", true));
        assert_eq!(app.sessions["s1"].error_count, 2);
    }

    // --- push_event: session_idle sets metadata ---

    #[test]
    fn session_idle_sets_project_dir_and_title() {
        let mut app = empty_app();
        app.push_event(idle(1, "s1", "/home/user/code"));
        let s = &app.sessions["s1"];
        assert_eq!(s.project_dir, "/home/user/code");
        assert_eq!(s.session_title.as_deref(), Some("proj-/home/user/code"));
    }

    // --- push_event: project_dir set from non-idle first event ---

    #[test]
    fn project_dir_set_from_non_idle_first_event() {
        // Regression: push_event previously only set project_dir from session_idle.
        // This test ensures it is also set from agent_turn_completed (or any event
        // that carries project_dir in EventRecord, which all events do since Slice 2).
        let mut app = empty_app();
        // Only push a turn — no idle event.
        app.push_event(turn(1, "s1", 10, 20));
        assert_eq!(app.sessions["s1"].project_dir, "/home/pl/code");
    }

    // --- push_event: session_updated lineage ingest (Slice 6a) ---

    #[test]
    fn push_session_updated_populates_summary() {
        let mut app = empty_app();
        app.push_event(session_updated(
            1,
            "s1",
            Some("ses_parent"),
            Some("claude"),
            Some("anthropic/claude-sonnet"),
            Some("hack"),
        ));
        let s = &app.sessions["s1"];
        assert_eq!(s.parent_id.as_deref(), Some("ses_parent"));
        assert_eq!(s.agent.as_deref(), Some("claude"));
        assert_eq!(s.model.as_deref(), Some("anthropic/claude-sonnet"));
        assert_eq!(s.session_title.as_deref(), Some("hack"));
        assert_eq!(s.harness, "opencode");
    }

    #[test]
    fn parent_id_set_once_not_clobbered_by_later_null() {
        // First session_updated establishes the parent. A later one carrying
        // parent_id=null must NOT clobber the known parent.
        let mut app = empty_app();
        app.push_event(session_updated(1, "s1", Some("ses_parent"), None, None, None));
        app.push_event(session_updated(2, "s1", None, Some("opus"), None, Some("new title")));
        assert_eq!(app.sessions["s1"].parent_id.as_deref(), Some("ses_parent"));
        // Other fields are last-writer-wins.
        assert_eq!(app.sessions["s1"].agent.as_deref(), Some("opus"));
        assert_eq!(app.sessions["s1"].session_title.as_deref(), Some("new title"));
    }

    #[test]
    fn title_updates_last_writer_wins() {
        let mut app = empty_app();
        app.push_event(session_updated(1, "s1", None, None, None, Some("first")));
        app.push_event(session_updated(2, "s1", None, None, None, Some("second")));
        assert_eq!(app.sessions["s1"].session_title.as_deref(), Some("second"));
    }

    #[test]
    fn agent_and_model_populated() {
        let mut app = empty_app();
        app.push_event(session_updated(1, "s1", None, Some("glm"), Some("zai/glm-5"), None));
        assert_eq!(app.sessions["s1"].agent.as_deref(), Some("glm"));
        assert_eq!(app.sessions["s1"].model.as_deref(), Some("zai/glm-5"));
    }

    #[test]
    fn session_updated_optional_none_leaves_fields_none() {
        // A primary session with no lineage: all optional fields stay None.
        let mut app = empty_app();
        app.push_event(session_updated(1, "s1", None, None, None, None));
        let s = &app.sessions["s1"];
        assert_eq!(s.parent_id, None);
        assert_eq!(s.agent, None);
        assert_eq!(s.model, None);
        assert_eq!(s.session_title, None);
    }

    #[test]
    fn agent_turn_completed_sets_model() {
        let mut app = empty_app();
        app.push_event(turn(1, "s1", 100, 200));
        assert_eq!(
            app.sessions["s1"].model.as_deref(),
            Some("anthropic/claude-sonnet")
        );
    }

    // --- diagnostics_len ---

    #[test]
    fn diagnostics_len_counts_only_tool_call_events() {
        let mut app = empty_app();
        app.push_event(idle(1, "s1", "/p"));
        app.push_event(tool_req(2, "s1"));
        app.push_event(turn(3, "s1", 10, 20));
        app.push_event(tool_done(4, "s1", false));
        assert_eq!(app.diagnostics_len(), 2);
    }

    // --- activity_len ---

    #[test]
    fn activity_len_excludes_hidden_session_updated() {
        let mut app = empty_app();
        app.push_event(session_updated(1, "s1", None, None, None, Some("title")));
        app.push_event(turn(2, "s1", 10, 20));
        app.push_event(session_updated(3, "s1", None, None, None, Some("title2")));
        app.push_event(tool_done(4, "s1", false));
        assert_eq!(app.activity_len(), 2);
    }

    #[test]
    fn activity_len_zero_when_only_session_updated() {
        let mut app = empty_app();
        app.push_event(session_updated(1, "s1", None, None, None, None));
        app.push_event(session_updated(2, "s1", None, None, None, Some("x")));
        assert_eq!(app.activity_len(), 0);
    }

    // --- scroll_down_activity clamp (correctness keystone) ---

    #[test]
    fn scroll_down_activity_clamps_at_activity_len_not_events_len() {
        // 4 events in session s1, 2 hidden (session_updated) -> 2 visible.
        let mut app = empty_app();
        app.push_event(session_updated(1, "s1", None, None, None, Some("a")));
        app.push_event(turn(2, "s1", 10, 20));
        app.push_event(session_updated(3, "s1", None, None, None, Some("b")));
        app.push_event(tool_done(4, "s1", false));
        assert_eq!(app.session_event_len("s1"), 2);

        // Point sel_session_id at s1 so scroll_down_activity uses s1's count.
        app.sel_session_id = Some("s1".to_string());

        // Scrolling down past the end must clamp at session_event_len-1 == 1,
        // never reach index 2 or 3 (which point at hidden events).
        app.scroll_down_activity();
        app.scroll_down_activity();
        app.scroll_down_activity();
        assert_eq!(app.sel_activity, 1, "clamps at last visible index");
    }

    #[test]
    fn scroll_down_activity_no_op_when_all_hidden() {
        let mut app = empty_app();
        app.push_event(session_updated(1, "s1", None, None, None, None));
        assert_eq!(app.session_event_len("s1"), 0);
        // sel_session_id = None -> session_event_len returns 0 -> no-op.
        app.scroll_down_activity();
        assert_eq!(app.sel_activity, 0);
    }

    // --- priority sort (Slice 8 Tier 1) ---

    fn make_task(title: &str, status: &str, priority: Option<&str>) -> cuelib::artifact::ArtifactMeta {
        cuelib::artifact::ArtifactMeta {
            title: title.to_string(),
            status_raw: Some(status.to_string()),
            priority_raw: priority.map(str::to_string),
            artifact_type: "task".to_string(),
            path: std::path::PathBuf::from(format!("/tmp/{title}.md")),
        }
    }

    fn make_kanban_task(title: &str, status: &str, priority: Option<&str>) -> KanbanTask {
        KanbanTask {
            meta: make_task(title, status, priority),
            project_key: "local:test".to_string(),
            project_root: std::path::PathBuf::from("/tmp/test"),
        }
    }

    #[test]
    fn priority_sort_in_app_new_is_critical_high_normal_low() {
        let tasks = vec![
            make_kanban_task("low-task", "open", Some("low")),
            make_kanban_task("critical-task", "open", Some("critical")),
            make_kanban_task("normal-task", "open", None),
            make_kanban_task("high-task", "open", Some("high")),
        ];
        let app = App::new(tasks);
        let titles: Vec<&str> = app.open.iter().map(|t| t.meta.title.as_str()).collect();
        assert_eq!(
            titles,
            &["critical-task", "high-task", "normal-task", "low-task"]
        );
    }

    // --- reload_kanban (Slice 8 Tier 1) ---

    #[test]
    fn reload_kanban_reclassifies_resets_selection_and_sorts() {
        let initial = vec![
            make_kanban_task("t1", "open", None),
            make_kanban_task("t2", "open", None),
            make_kanban_task("t3", "open", None),
        ];
        let mut app = App::new(initial);
        // Move selections away from 0 to confirm reset.
        app.sel_open = 2;
        app.sel_in_progress = 1;

        let new_tasks = vec![
            make_kanban_task("n-low", "open", Some("low")),
            make_kanban_task("n-critical", "open", Some("critical")),
            make_kanban_task("n-progress", "in-progress", None),
        ];
        app.reload_kanban(new_tasks);

        // Reclassification correct.
        assert_eq!(app.open.len(), 2);
        assert_eq!(app.in_progress.len(), 1);
        assert_eq!(app.complete.len(), 0);

        // Priority sort applied to the new Open column.
        assert_eq!(app.open[0].meta.title, "n-critical");
        assert_eq!(app.open[1].meta.title, "n-low");

        // All selection indices reset.
        assert_eq!(app.sel_open, 0);
        assert_eq!(app.sel_in_progress, 0);
        assert_eq!(app.sel_complete, 0);
    }

    // --- sorted_sessions ---

    /// Push a non-hidden event into the app with a specific received_at time.
    fn push_turn_at(app: &mut App, seq: i64, session_id: &str, received_at: &str) {
        use acuity_schema::AgentTurnCompleted;
        let event = AcuityEvent::AgentTurnCompleted(AgentTurnCompleted {
            session_id: session_id.to_string(),
            turn_id: "t1".to_string(),
            project_dir: "/p".to_string(),
            harness: "opencode".to_string(),
            input_tokens: Some(1),
            output_tokens: Some(1),
            model: None,
        });
        app.events.push_back(EventRecord {
            seq,
            received_at: received_at.to_string(),
            event_type: event.event_type().to_string(),
            session_id: session_id.to_string(),
            turn_id: event.turn_id().map(str::to_string),
            project_dir: "/p".to_string(),
            harness: "opencode".to_string(),
            payload: serde_json::to_string(&event).unwrap(),
        });
        // Manually set first_seen for deterministic ordering tests.
        let entry = app.sessions.entry(session_id.to_string()).or_insert_with(|| {
            SessionSummary {
                session_id: session_id.to_string(),
                project_dir: "/p".to_string(),
                session_title: None,
                first_seen: received_at.to_string(),
                last_seen: received_at.to_string(),
                input_tokens: 0,
                output_tokens: 0,
                error_count: 0,
                harness: "opencode".to_string(),
                parent_id: None,
                agent: None,
                model: None,
                visible_event_count: 0,
                unique_agents: Vec::new(),
                unique_models: Vec::new(),
            }
        });
        if received_at < entry.first_seen.as_str() {
            entry.first_seen = received_at.to_string();
        }
        if received_at > entry.last_seen.as_str() {
            entry.last_seen = received_at.to_string();
        }
        // push_turn_at bypasses push_event, so maintain the visible count manually.
        entry.visible_event_count += 1;
    }

    #[test]
    fn sorted_sessions_newest_first() {
        let mut app = empty_app();
        push_turn_at(&mut app, 1, "s_older", "2026-01-01T00:00:01Z");
        push_turn_at(&mut app, 2, "s_newer", "2026-01-01T00:00:02Z");
        let ids: Vec<&str> = app.sorted_sessions().iter().map(|s| s.session_id.as_str()).collect();
        assert_eq!(ids, vec!["s_newer", "s_older"]);
    }

    #[test]
    fn sorted_sessions_tiebreak_by_session_id() {
        let mut app = empty_app();
        // Both sessions get the same first_seen timestamp.
        push_turn_at(&mut app, 1, "ses_b", "2026-01-01T00:00:01Z");
        push_turn_at(&mut app, 2, "ses_a", "2026-01-01T00:00:01Z");
        let ids: Vec<&str> = app.sorted_sessions().iter().map(|s| s.session_id.as_str()).collect();
        // Alphabetical ascending tiebreak -> ses_a before ses_b.
        assert_eq!(ids, vec!["ses_a", "ses_b"]);
    }

    #[test]
    fn sorted_sessions_excludes_sessions_with_no_visible_events() {
        let mut app = empty_app();
        // s1 has a turn (visible). s2 only has session_updated (hidden).
        app.push_event(turn(1, "s1", 10, 20));
        app.push_event(session_updated(2, "s2", None, None, None, Some("t")));
        let ids: Vec<&str> = app.sorted_sessions().iter().map(|s| s.session_id.as_str()).collect();
        assert_eq!(ids, vec!["s1"]);
    }

    // --- session_event_len ---

    #[test]
    fn session_event_len_counts_only_target_session() {
        let mut app = empty_app();
        app.push_event(turn(1, "s1", 10, 20));
        app.push_event(turn(2, "s2", 10, 20));
        app.push_event(turn(3, "s2", 10, 20));
        assert_eq!(app.session_event_len("s1"), 1);
        assert_eq!(app.session_event_len("s2"), 2);
    }

    #[test]
    fn session_event_len_excludes_hidden_events() {
        let mut app = empty_app();
        app.push_event(turn(1, "s1", 10, 20));
        app.push_event(session_updated(2, "s1", None, None, None, None));
        // 1 visible (turn), 1 hidden (session_updated).
        assert_eq!(app.session_event_len("s1"), 1);
    }

    #[test]
    fn visible_event_count_tracks_eviction() {
        let mut app = empty_app();
        for i in 0..5 {
            app.push_event(turn(i, "s1", 1, 1));
        }
        assert_eq!(app.session_event_len("s1"), 5);
        // Fill the ring buffer to evict 2 of s1's 5 events.
        // 5 s1 + 1995 s2 = 2000 (at cap); each additional s2 evicts one from front.
        for i in 0..1997 {
            app.push_event(turn(100 + i, "s2", 1, 1));
        }
        assert_eq!(app.session_event_len("s1"), 3);
        assert_eq!(app.session_event_len("s2"), 1997);
    }

    #[test]
    fn visible_event_count_recovers_after_full_eviction() {
        // A session whose events are fully evicted (count reaches 0) must
        // correctly increment back to 1 when a new event arrives.
        let mut app = empty_app();
        app.push_event(turn(1, "s1", 1, 1));
        // Fill buffer to fully evict s1's single event.
        for i in 0..EVENT_CAP {
            app.push_event(turn(100 + i as i64, "s2", 1, 1));
        }
        assert_eq!(app.session_event_len("s1"), 0);
        // Push a new s1 event — count must go 0 -> 1.
        app.push_event(turn(999, "s1", 1, 1));
        assert_eq!(app.session_event_len("s1"), 1);
    }

    #[test]
    fn unique_agents_accumulate_and_survive_eviction() {
        let mut app = empty_app();
        app.push_event(session_updated(
            1, "s1", None, Some("plan"), None, Some("t1"),
        ));
        app.push_event(session_updated(
            2, "s1", None, Some("build"), None, None,
        ));
        assert_eq!(app.sessions["s1"].unique_agents, vec!["plan", "build"]);
        // Evict all s1 events by filling the buffer with s2 turns.
        for i in 0..(EVENT_CAP + 1) {
            app.push_event(turn(100 + i as i64, "s2", 1, 1));
        }
        // unique_agents survives eviction (cached on summary, not ring buffer).
        assert_eq!(app.sessions["s1"].unique_agents, vec!["plan", "build"]);
    }

    #[test]
    fn unique_models_accumulate_from_turn_and_session_updated() {
        let mut app = empty_app();
        // turn() carries model "anthropic/claude-sonnet".
        app.push_event(turn(1, "s1", 1, 1));
        // session_updated with a different model.
        app.push_event(session_updated(
            2, "s1", None, None, Some("google/gemini-flash"), None,
        ));
        assert_eq!(
            app.sessions["s1"].unique_models,
            vec!["anthropic/claude-sonnet", "google/gemini-flash"],
        );
    }

    #[test]
    fn unique_agents_dedup_repeats() {
        let mut app = empty_app();
        app.push_event(session_updated(
            1, "s1", None, Some("plan"), None, Some("t1"),
        ));
        app.push_event(session_updated(2, "s1", None, Some("plan"), None, None));
        assert_eq!(app.sessions["s1"].unique_agents, vec!["plan"]);
    }

    #[test]
    fn unique_models_dedup_same_model_across_turns() {
        let mut app = empty_app();
        // Both turns use the same model from the turn() helper.
        app.push_event(turn(1, "s1", 1, 1));
        app.push_event(turn(2, "s1", 1, 1));
        assert_eq!(
            app.sessions["s1"].unique_models,
            vec!["anthropic/claude-sonnet"],
        );
    }

    // --- ensure_session_selection ---

    #[test]
    fn ensure_session_selection_sets_top_on_none() {
        let mut app = empty_app();
        app.push_event(turn(1, "s1", 10, 20));
        assert_eq!(app.sel_session_id, None);
        app.ensure_session_selection();
        assert_eq!(app.sel_session_id.as_deref(), Some("s1"));
        assert_eq!(app.sel_activity, 0);
    }

    #[test]
    fn ensure_session_selection_resets_on_eviction() {
        // sel points to a session whose events have all been evicted.
        let mut app = empty_app();
        app.push_event(turn(1, "s1", 10, 20));
        app.push_event(turn(2, "s2", 10, 20));
        // Force sel to s_evicted (a session with zero visible events).
        app.sel_session_id = Some("s_evicted".to_string());
        app.ensure_session_selection();
        // Should auto-select the top visible session (s2 newer than s1).
        let sel = app.sel_session_id.as_deref();
        assert!(sel == Some("s1") || sel == Some("s2"),
            "should auto-select a session with events, got {sel:?}");
        assert_eq!(app.sel_activity, 0);
    }

    #[test]
    fn ensure_session_selection_is_idempotent() {
        let mut app = empty_app();
        for i in 0..10 {
            app.push_event(turn(i, "s1", 10, 20));
        }
        app.ensure_session_selection();
        app.sel_activity = 3; // Valid scroll position within 10 events.
        app.ensure_session_selection(); // Second call — s1 still valid.
        // sel_activity must NOT change when it's a valid index.
        assert_eq!(app.sel_activity, 3);
    }

    #[test]
    fn ensure_session_selection_clamps_stale_sel_activity() {
        // After partial ring-buffer eviction, sel_activity can exceed the
        // session's current visible event count. ensure_session_selection
        // must clamp it so the highlight doesn't appear stranded.
        let mut app = empty_app();
        for i in 0..5 {
            app.push_event(turn(i, "ses_A", 1, 1));
        }
        app.sel_session_id = Some("ses_A".to_string());
        app.sel_activity = 39; // Stale — far beyond the 5 visible events.
        app.ensure_session_selection();
        assert_eq!(
            app.sel_activity, 4,
            "stale sel_activity clamped to visible count - 1"
        );
    }

    // --- scroll_down_sessions / scroll_up_sessions ---

    #[test]
    fn scroll_down_sessions_advances_identity() {
        let mut app = empty_app();
        push_turn_at(&mut app, 1, "s_newer", "2026-01-01T00:00:02Z");
        push_turn_at(&mut app, 2, "s_older", "2026-01-01T00:00:01Z");
        app.sel_session_id = Some("s_newer".to_string());
        app.scroll_down_sessions();
        assert_eq!(app.sel_session_id.as_deref(), Some("s_older"));
    }

    #[test]
    fn scroll_down_sessions_resets_sel_activity() {
        let mut app = empty_app();
        push_turn_at(&mut app, 1, "s_newer", "2026-01-01T00:00:02Z");
        push_turn_at(&mut app, 2, "s_older", "2026-01-01T00:00:01Z");
        app.sel_session_id = Some("s_newer".to_string());
        app.sel_activity = 3;
        app.scroll_down_sessions();
        assert_eq!(app.sel_activity, 0, "sel_activity resets on session change");
    }

    #[test]
    fn scroll_down_sessions_no_op_at_end() {
        let mut app = empty_app();
        push_turn_at(&mut app, 1, "s_newer", "2026-01-01T00:00:02Z");
        push_turn_at(&mut app, 2, "s_older", "2026-01-01T00:00:01Z");
        app.sel_session_id = Some("s_older".to_string());
        app.scroll_down_sessions();
        assert_eq!(app.sel_session_id.as_deref(), Some("s_older"), "no-op at last");
    }

    #[test]
    fn scroll_up_sessions_retreats_identity() {
        let mut app = empty_app();
        push_turn_at(&mut app, 1, "s_newer", "2026-01-01T00:00:02Z");
        push_turn_at(&mut app, 2, "s_older", "2026-01-01T00:00:01Z");
        app.sel_session_id = Some("s_older".to_string());
        app.scroll_up_sessions();
        assert_eq!(app.sel_session_id.as_deref(), Some("s_newer"));
    }

    #[test]
    fn scroll_up_sessions_no_op_at_start() {
        let mut app = empty_app();
        push_turn_at(&mut app, 1, "s_newer", "2026-01-01T00:00:02Z");
        push_turn_at(&mut app, 2, "s_older", "2026-01-01T00:00:01Z");
        app.sel_session_id = Some("s_newer".to_string());
        app.scroll_up_sessions();
        assert_eq!(app.sel_session_id.as_deref(), Some("s_newer"), "no-op at first");
    }

    #[test]
    fn scroll_sessions_empty_list_no_op() {
        let mut app = empty_app();
        // No events — both scroll methods are no-ops.
        app.scroll_down_sessions();
        assert_eq!(app.sel_session_id, None);
        app.scroll_up_sessions();
        assert_eq!(app.sel_session_id, None);
    }

    // --- ActivityLayout transitions ---

    #[test]
    fn toggle_detail_pane_sessions_full_to_split() {
        let mut app = empty_app();
        assert_eq!(app.activity_layout, ActivityLayout::SessionsFull);
        app.toggle_detail_pane();
        assert_eq!(app.activity_layout, ActivityLayout::Split);
    }

    #[test]
    fn toggle_detail_pane_split_to_sessions_full() {
        let mut app = empty_app();
        app.activity_layout = ActivityLayout::Split;
        app.toggle_detail_pane();
        assert_eq!(app.activity_layout, ActivityLayout::SessionsFull);
    }

    #[test]
    fn toggle_detail_pane_noop_in_detail_full() {
        let mut app = empty_app();
        app.activity_layout = ActivityLayout::DetailFull;
        app.toggle_detail_pane();
        assert_eq!(app.activity_layout, ActivityLayout::DetailFull);
    }

    #[test]
    fn enter_detail_full_from_sessions_full() {
        let mut app = empty_app();
        app.enter_detail_full();
        assert_eq!(app.activity_layout, ActivityLayout::DetailFull);
    }

    #[test]
    fn enter_detail_full_from_split() {
        let mut app = empty_app();
        app.activity_layout = ActivityLayout::Split;
        app.enter_detail_full();
        assert_eq!(app.activity_layout, ActivityLayout::DetailFull);
    }

    #[test]
    fn enter_detail_full_noop_when_already_full() {
        let mut app = empty_app();
        app.activity_layout = ActivityLayout::DetailFull;
        app.enter_detail_full();
        assert_eq!(app.activity_layout, ActivityLayout::DetailFull);
    }

    #[test]
    fn return_from_detail_full_goes_to_split() {
        let mut app = empty_app();
        app.activity_layout = ActivityLayout::DetailFull;
        app.return_from_detail_full();
        assert_eq!(app.activity_layout, ActivityLayout::Split);
    }

    #[test]
    fn return_from_detail_full_noop_in_sessions_full() {
        let mut app = empty_app();
        app.return_from_detail_full();
        assert_eq!(app.activity_layout, ActivityLayout::SessionsFull);
    }

    #[test]
    fn return_from_detail_full_noop_in_split() {
        let mut app = empty_app();
        app.activity_layout = ActivityLayout::Split;
        app.return_from_detail_full();
        assert_eq!(app.activity_layout, ActivityLayout::Split);
    }

    // --- collect_tasks (Slice 1) ---

    #[test]
    fn collect_tasks_multi_project() {
        use std::io::Write;
        use tempfile::TempDir;

        // Create two project temp dirs each with a task artifact.
        let proj_a = TempDir::new().unwrap();
        let proj_b = TempDir::new().unwrap();

        fn write_task(root: &std::path::Path, name: &str, status: &str) {
            let dir = root.join(".cue").join("master").join("task");
            std::fs::create_dir_all(&dir).unwrap();
            let mut f = std::fs::File::create(dir.join(format!("{name}.md"))).unwrap();
            writeln!(f, "---").unwrap();
            writeln!(f, "title: {name}").unwrap();
            writeln!(f, "status: {status}").unwrap();
            writeln!(f, "---").unwrap();
        }

        write_task(proj_a.path(), "task-alpha", "open");
        write_task(proj_b.path(), "task-beta", "in-progress");

        let mut store = cuelib::project::ProjectStore::default();
        store.add_path("local:proj-a", proj_a.path());
        store.add_path("local:proj-b", proj_b.path());

        let tasks = collect_tasks(&store, "master");
        assert_eq!(tasks.len(), 2, "one task per project");

        let mut titles: Vec<&str> = tasks.iter().map(|t| t.meta.title.as_str()).collect();
        titles.sort();
        assert_eq!(titles, &["task-alpha", "task-beta"]);
    }

    #[test]
    fn collect_tasks_skips_missing_root() {
        let mut store = cuelib::project::ProjectStore::default();
        store.add_path("local:missing", std::path::PathBuf::from("/nonexistent/path/xyz"));

        // Should not panic; returns empty.
        let tasks = collect_tasks(&store, "master");
        assert!(tasks.is_empty());
    }

    #[test]
    fn collect_tasks_uses_first_path_only() {
        use std::io::Write;
        use tempfile::TempDir;

        let proj_a = TempDir::new().unwrap();
        let proj_b = TempDir::new().unwrap();

        fn write_task(root: &std::path::Path, name: &str) {
            let dir = root.join(".cue").join("master").join("task");
            std::fs::create_dir_all(&dir).unwrap();
            let mut f = std::fs::File::create(dir.join(format!("{name}.md"))).unwrap();
            writeln!(f, "---\ntitle: {name}\nstatus: open\n---").unwrap();
        }

        write_task(proj_a.path(), "first-path-task");
        write_task(proj_b.path(), "second-path-task");

        let mut store = cuelib::project::ProjectStore::default();
        // Add both paths under the same key — only first should be read.
        store.add_path("local:shared", proj_a.path());
        store.add_path("local:shared", proj_b.path());

        let tasks = collect_tasks(&store, "master");
        assert_eq!(tasks.len(), 1);
        assert_eq!(tasks[0].meta.title, "first-path-task");
    }

    #[test]
    fn collect_tasks_empty_store_returns_empty() {
        let store = cuelib::project::ProjectStore::default();
        let tasks = collect_tasks(&store, "master");
        assert!(tasks.is_empty());
    }
}
