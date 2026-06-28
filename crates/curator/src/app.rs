use std::collections::{HashMap, VecDeque};

use acuity_api::{AcuityEvent, EventRecord};
use cuelib::artifact::{ArtifactMeta, TaskStatus};

use crate::msg::SseStatus;

/// Which top-level view is currently displayed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum View {
    Kanban,
    Activity,
    Diagnostics,
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

/// Per-session aggregate built incrementally by [`App::push_event`].
///
/// Lives in [`App::sessions`] and is updated *before* ring-buffer eviction,
/// so totals survive even when old events are evicted from the ring buffer.
#[derive(Debug, Clone)]
pub struct SessionSummary {
    /// Stored for Slice 8 unit tests; not yet read by rendering code.
    #[allow(dead_code)]
    pub session_id: String,
    pub project_dir: String,
    pub session_title: Option<String>,
    /// ISO-8601 timestamp of the earliest received event for this session.
    /// Stored for Slice 8 unit tests and future display; not yet read by rendering.
    #[allow(dead_code)]
    pub first_seen: String,
    /// ISO-8601 timestamp of the most recently received event for this session.
    pub last_seen: String,
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub error_count: u32,
}

/// Maximum number of events retained in the ring buffer.
pub const EVENT_CAP: usize = 2_000;

/// Application state for the curator kanban board.
pub struct App {
    // --- Kanban ---
    /// Tasks in the Open column.
    pub open: Vec<ArtifactMeta>,
    /// Tasks in the In Progress column.
    pub in_progress: Vec<ArtifactMeta>,
    /// Tasks in the Complete column.
    pub complete: Vec<ArtifactMeta>,

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

    /// Selection index for the Activity Feed view.
    pub sel_activity: usize,
    /// Selection index for the Diagnostics view.
    pub sel_diagnostics: usize,
}

impl App {
    /// Classify `tasks` into kanban columns using the typed `TaskStatus` from
    /// `cuelib`. Tasks whose status parses as `Closed`, or whose status field
    /// is absent / unrecognised, are silently excluded (they are not kanban-
    /// visible by definition).
    pub fn new(tasks: Vec<ArtifactMeta>) -> Self {
        let (open, in_progress, complete) = classify_tasks(tasks);

        Self {
            open,
            in_progress,
            complete,
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
        }
    }

    /// Re-classify a new set of tasks into the kanban columns, resetting all
    /// column selection indices. Called on `Action::Refresh`.
    pub fn reload_kanban(&mut self, tasks: Vec<ArtifactMeta>) {
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
            });

        // Always update last_seen if this event is newer.
        if record.received_at > entry.last_seen {
            entry.last_seen.clone_from(&record.received_at);
        }

        match record.event_type.as_str() {
            "session_idle" => {
                if let Ok(AcuityEvent::SessionIdle(ev)) =
                    serde_json::from_str::<AcuityEvent>(&record.payload)
                {
                    entry.project_dir.clone_from(&ev.project_dir);
                    entry.session_title.clone_from(&ev.session_title);
                    // first_seen is only set once (on insert).
                }
            }
            "agent_turn_completed" => {
                if let Ok(AcuityEvent::AgentTurnCompleted(ev)) =
                    serde_json::from_str::<AcuityEvent>(&record.payload)
                {
                    entry.input_tokens += ev.input_tokens.unwrap_or(0) as u64;
                    entry.output_tokens += ev.output_tokens.unwrap_or(0) as u64;
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
        if self.events.len() >= EVENT_CAP {
            self.events.pop_front();
        }

        // --- 3. Append ---
        self.events.push_back(record);
    }

    /// Number of events that pass the Diagnostics filter (`tool_call_*`).
    pub fn diagnostics_len(&self) -> usize {
        self.events
            .iter()
            .filter(|e| e.event_type.starts_with("tool_call_"))
            .count()
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
    pub fn column_tasks(&self, col: Column) -> &[ArtifactMeta] {
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

    /// Move the Activity Feed selection down (newest-first display order).
    pub fn scroll_down_activity(&mut self) {
        let len = self.events.len();
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
    tasks: Vec<ArtifactMeta>,
) -> (Vec<ArtifactMeta>, Vec<ArtifactMeta>, Vec<ArtifactMeta>) {
    let mut open = Vec::new();
    let mut in_progress = Vec::new();
    let mut complete = Vec::new();

    for task in tasks {
        match task.status::<TaskStatus>() {
            Some(TaskStatus::Open) => open.push(task),
            Some(TaskStatus::InProgress) => in_progress.push(task),
            Some(TaskStatus::Complete) => complete.push(task),
            Some(TaskStatus::Closed) | None => {}
        }
    }

    open.sort_by_key(|t| priority_rank(t.priority_raw.as_deref()));
    in_progress.sort_by_key(|t| priority_rank(t.priority_raw.as_deref()));
    complete.sort_by_key(|t| priority_rank(t.priority_raw.as_deref()));

    (open, in_progress, complete)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use acuity_schema::{
        AcuityEvent, AgentTurnCompleted, SessionIdle, ToolCallCompleted, ToolCallRequested,
    };

    // --- Helpers ---

    fn make_record(seq: i64, session_id: &str, event: AcuityEvent) -> EventRecord {
        EventRecord {
            seq,
            received_at: format!("2026-01-01T00:00:{seq:02}Z"),
            event_type: event.event_type().to_string(),
            session_id: session_id.to_string(),
            turn_id: event.turn_id().map(str::to_string),
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
        // Push EVENT_CAP more events so the idle is evicted.
        for i in 1..=(EVENT_CAP as i64) {
            app.push_event(turn(i, "s1", 1, 1));
        }
        // The ring buffer no longer contains seq=0, but sessions map must not.
        assert!(!app.events.iter().any(|e| e.seq == 0));
        assert_eq!(app.sessions["s1"].project_dir, "/my/project");
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

    #[test]
    fn priority_sort_in_app_new_is_critical_high_normal_low() {
        let tasks = vec![
            make_task("low-task", "open", Some("low")),
            make_task("critical-task", "open", Some("critical")),
            make_task("normal-task", "open", None),
            make_task("high-task", "open", Some("high")),
        ];
        let app = App::new(tasks);
        let titles: Vec<&str> = app.open.iter().map(|t| t.title.as_str()).collect();
        assert_eq!(
            titles,
            &["critical-task", "high-task", "normal-task", "low-task"]
        );
    }

    // --- reload_kanban (Slice 8 Tier 1) ---

    #[test]
    fn reload_kanban_reclassifies_resets_selection_and_sorts() {
        let initial = vec![
            make_task("t1", "open", None),
            make_task("t2", "open", None),
            make_task("t3", "open", None),
        ];
        let mut app = App::new(initial);
        // Move selections away from 0 to confirm reset.
        app.sel_open = 2;
        app.sel_in_progress = 1;

        let new_tasks = vec![
            make_task("n-low", "open", Some("low")),
            make_task("n-critical", "open", Some("critical")),
            make_task("n-progress", "in-progress", None),
        ];
        app.reload_kanban(new_tasks);

        // Reclassification correct.
        assert_eq!(app.open.len(), 2);
        assert_eq!(app.in_progress.len(), 1);
        assert_eq!(app.complete.len(), 0);

        // Priority sort applied to the new Open column.
        assert_eq!(app.open[0].title, "n-critical");
        assert_eq!(app.open[1].title, "n-low");

        // All selection indices reset.
        assert_eq!(app.sel_open, 0);
        assert_eq!(app.sel_in_progress, 0);
        assert_eq!(app.sel_complete, 0);
    }
}
