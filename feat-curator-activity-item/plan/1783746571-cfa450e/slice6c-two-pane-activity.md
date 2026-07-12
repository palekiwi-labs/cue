---
status: complete
refs:
- .cue/feat-curator-activity-item/spec/index.md
- .cue/feat-curator-activity-item/plan/1782659497-0c2ff37/slice6b-rendering.md
---
## Foreword

This plan redesigns the curator Activity view from a single flat-event pane
into a two-pane master-detail layout, fixing two UX defects found during Slice
6b manual QA:

1. **Header placement**: harness + project appeared in the pane border title
   (`" Activity Feed · opencode / cue.nvim "`) — far from the session content
   and visually disconnected. They should appear on the session row itself.
2. **Session interleaving**: flat reverse-chrono iteration caused events from
   concurrent sessions to interleave, and sessions jumped position on every
   new event arrival.

### Design

```
┌ Sessions ───────────────┐┌ Events · TASK | Migrate to nixpkgs 26.05 ────┐
│> cue  opencode  TASK..  ││ 2026-07-11T07:41:57  agent_turn_completed ... │
│  cue  opencode  Upg..   ││ 2026-07-11T07:41:57  tool_call_completed  ... │
│                         ││ 2026-07-11T07:41:57  tool_call_requested   ... │
└─────────────────────────┘└────────────────────────────────────────────────┘
 q quit  1/2/3 views  Tab pane  z expand  j/k navigate  |  acuity: connected
```

Two panes, left-right: `Constraint::Ratio(1,3)` Sessions / `Constraint::Ratio(2,3)` Events.

### Key decisions

- **Session selection is identity-based**: `sel_session_id: Option<String>` tracks
  the selected session's id string. When a new session prepends to Pane 1 the
  cursor stays on the same session (not on the same visual row index). Index-based
  selection would jump unpredictably across sessions.
- **Event selection is index-based**: `sel_activity: usize` indexes into the
  selected session's events. Resets to 0 on session change. Acceptable for now
  since shifts are within one session only.
- **Invariant**: any mutation of `sel_session_id` immediately resets `sel_activity = 0`.
- **`z` expands** the active pane to fullwidth; pressing again collapses to split.
  `Tab` / `Shift-Tab` switch panes (stays expanded if already expanded).
- **`sorted_sessions()` filters** to sessions with at least one visible event in
  the ring buffer. Sessions whose events have all been evicted simply disappear
  from the list rather than showing an empty events pane.
- **`ensure_session_selection()`** is called in `process_msg` after `push_event` —
  not inside `push_event` itself (keeps the data-ingest method pure and its
  20 existing tests unchanged).
- **`activity.rs` is untouched**. `build_activity_items` (12 tests, turn-folding
  scaffolding) finds its correct home in stage-C Pane 2 (turn expand/collapse
  with a `session_id` filter parameter). Stage-B Pane 2 uses a simpler per-session
  flat filter.
- **Colors**: session row spans: project=Magenta, harness=Blue,
  title=Cyan+Bold (titled) / DarkGray (placeholder).
- **`activity_block_title()`** is deleted. Block titles are now per-pane:
  `" Sessions "` and `" Events · {title} "`.
- **`activity_help_line()`** is a new function separate from `status_help_line()`
  so the Diagnostics view's help bar is unaffected by the added Tab/z hints.

### Repos and paths

- **cue repo:** `/home/pl/code/palekiwi-labs/cue` (branch `feat/curator-activity-item`)
- **Relevant files:** `crates/curator/src/{app,event,input,main,ui,activity}.rs`

---

## Steps

### Step 1 — App state + helpers (`app.rs`, TDD)

- [x] Add `pub enum ActivityPane { Sessions, Events }` to `app.rs`.
- [x] Add three new fields to `App`:
  ```rust
  pub active_activity_pane: ActivityPane,  // default: Sessions
  pub sel_session_id: Option<String>,       // identity-based; None until first event
  pub pane_expanded: bool,                  // default: false
  ```
- [x] Initialize in `App::new()`.
- [x] Remove `#[allow(dead_code)]` from `SessionSummary.first_seen` and
  `SessionSummary.session_id` — both are now read by rendering/sorting.
- [x] Add `#[allow(dead_code)]` to `activity_len()` — kept for its two tests,
  no longer called in production after this change.
- [x] **RED**: write failing tests for all new methods (see below), confirm
  compilation error.
- [x] Implement `sorted_sessions(&self) -> Vec<&SessionSummary>`: filter to
  sessions with `session_event_len > 0`, sort `first_seen` desc with
  `session_id` asc tiebreak (HashMap iteration is non-deterministic — tiebreak
  is required for frame-stable ordering).
- [x] Implement `session_event_len(&self, session_id: &str) -> usize`: count
  visible (non-hidden) events for a single session via `is_hidden_in_activity`.
- [x] Implement `ensure_session_selection(&mut self)`: if `sel_session_id` is
  `None` or its session has no visible events, set to `sorted_sessions().first()`
  and reset `sel_activity = 0`. Idempotent — safe to call every event arrival.
- [x] Implement `scroll_down_sessions(&mut self)`: find `sel_session_id` in
  `sorted_sessions()` via `.iter().position(...)`, advance to next, update
  `sel_session_id`, reset `sel_activity = 0`. No-op when list is empty or
  already at last.
- [x] Implement `scroll_up_sessions(&mut self)`: mirror with
  `saturating_sub`. No-op when at first or empty.
- [x] Implement `switch_activity_pane(&mut self)`: toggle
  `active_activity_pane` between Sessions and Events.
- [x] Implement `toggle_pane_expand(&mut self)`: toggle `pane_expanded`.
- [x] Update `scroll_down_activity`: clamp against
  `session_event_len(sel_session_id)` instead of `activity_len()`. Use
  `sel_session_id.as_deref().map(|id| self.session_event_len(id)).unwrap_or(0)`.
- [x] Update `scroll_up_activity`: unchanged logic; already uses
  `saturating_sub`.
- [x] **GREEN**: all new and updated tests pass.
- [x] `cargo test -p curator && cargo clippy -p curator -- -D warnings`

New tests required:
- `sorted_sessions_newest_first` — two sessions with different first_seen, assert order
- `sorted_sessions_tiebreak_by_session_id` — same first_seen, assert deterministic order
- `sorted_sessions_excludes_sessions_with_no_visible_events`
- `session_event_len_counts_only_target_session`
- `session_event_len_excludes_hidden_events`
- `ensure_session_selection_sets_top_on_none`
- `ensure_session_selection_resets_on_eviction` — sel points to a session with no events → auto-reselect
- `ensure_session_selection_is_idempotent` — called twice, no double-reset
- `scroll_down_sessions_advances_identity` — sel_session_id changes to next session
- `scroll_down_sessions_resets_sel_activity` — sel_activity goes to 0 on session change
- `scroll_down_sessions_no_op_at_end`
- `scroll_up_sessions_retreats_identity`
- `scroll_up_sessions_no_op_at_start`
- `scroll_sessions_empty_list_no_op`
- `switch_activity_pane_toggles`
- `toggle_pane_expand_toggles`
- Update: `scroll_down_activity_clamps_at_activity_len_not_events_len` →
  set up `sel_session_id` pointing at a session and assert clamp against
  that session's event count

### Step 2 — Input / event / main wiring (`event.rs`, `input.rs`, `main.rs`)

- [x] `event.rs`: add two unit variants (Copy-safe, no payload):
  ```rust
  SwitchPane,
  ToggleExpand,
  ```
- [x] `input.rs:map_key`: add bindings:
  ```rust
  KeyCode::Tab | KeyCode::BackTab => Action::SwitchPane,
  KeyCode::Char('z')              => Action::ToggleExpand,
  ```
- [x] `main.rs`: add `ActivityPane` to the `use app::{App, View}` import.
- [x] `main.rs:process_msg`: make `Action::Down` and `Action::Up` pane-aware
  inside `View::Activity`:
  ```rust
  View::Activity => match app.active_activity_pane {
      ActivityPane::Sessions => app.scroll_down_sessions(),
      ActivityPane::Events   => app.scroll_down_activity(),
  },
  ```
- [x] `main.rs:process_msg`: handle new actions:
  ```rust
  Msg::Input(Action::SwitchPane)   => app.switch_activity_pane(),
  Msg::Input(Action::ToggleExpand) => app.toggle_pane_expand(),
  ```
- [x] `main.rs:process_msg`: call `ensure_session_selection` after
  `push_event` in the `Msg::Sse` arm:
  ```rust
  Msg::Sse(record) => {
      app.push_event(record);
      app.ensure_session_selection();
  },
  ```
- [x] `cargo test -p curator && cargo clippy -p curator -- -D warnings`

### Step 3 — Render rework (`ui.rs`, TDD for pure helpers)

- [x] **RED**: write failing tests for `project_basename`.
- [x] Extract `fn project_basename(project_dir: &str) -> &str` (logic currently
  in the soon-to-be-deleted `activity_block_title`; `rsplit('/').next()` with
  empty guard).
- [x] **GREEN**: `project_basename` tests pass.
- [x] Delete `activity_block_title()` and its two tests
  (`activity_block_title_no_sessions`, `activity_block_title_with_session`).
- [x] New `fn render_sessions_pane(frame, app, area)`:
  - calls `app.sorted_sessions()` for the ordered list;
  - one row per session — `Line::from(vec![Span::styled(project, Magenta), Span::raw(" "), Span::styled(harness, Blue), Span::raw(" "), Span::styled(label, title_style)])` where `label` comes from `session_label()` (unchanged function);
  - active pane: `highlight_style = bg(DarkGray) + BOLD`; inactive: `Modifier::DIM`;
  - block title: `" Sessions "`.
- [x] New `fn render_events_pane(frame, app, area)`:
  - filters `app.events` by `sel_session_id` (`.as_deref()` guard), reverse-chrono;
  - empty state: single dim `ListItem` with `"  (no events)"`;
  - per-event rendering unchanged from current `render_activity` event rows;
  - block title: `session_label(app.sessions.get(sel_session_id), sel_session_id)`
    returns `(label, is_placeholder)` — title is `" Events · {label} "`;
  - active/inactive highlight same pattern as sessions pane.
- [x] New `fn activity_help_line(status: &AcuityStatus) -> Line<'static>`: adds
  `Tab pane` and `z expand` hints. Separate from `status_help_line` so
  Diagnostics is unaffected.
- [x] Rework `render_activity`:
  ```rust
  fn render_activity(frame: &mut Frame, app: &App) {
      let (view_area, help_area) = layout_with_help(frame.area());
      match (app.pane_expanded, app.active_activity_pane) {
          (true, ActivityPane::Sessions) =>
              render_sessions_pane(frame, app, view_area),
          (true, ActivityPane::Events) =>
              render_events_pane(frame, app, view_area),
          (false, _) => {
              let [sessions_area, events_area] = Layout::horizontal([
                  Constraint::Ratio(1, 3),
                  Constraint::Ratio(2, 3),
              ]).areas(view_area);
              render_sessions_pane(frame, app, sessions_area);
              render_events_pane(frame, app, events_area);
          }
      }
      frame.render_widget(activity_help_line(&app.acuity_status), help_area);
  }
  ```
- [x] `cargo test -p curator && cargo clippy -p curator -- -D warnings`

### Step 4 — `activity.rs` doc-comment update

- [x] Update the doc on `build_activity_items` at `activity.rs:66-74` to
  reflect the two-pane context:
  - Stage-B Pane 2 uses a simple per-session flat event filter (not this function).
  - Stage-C will wire this function into Pane 2 by adding a
    `session_id: Option<&str>` filter at the top of the `events.iter().rev()`
    loop; whether to suppress `SessionHeader` in single-session mode is an open
    design decision for stage C.
  - Note: entire module may be replaced or cleaned up in the stage-C PR if the
    two-pane design renders the current grouping model obsolete.
- [x] `cargo test --workspace && cargo clippy --workspace -- -D warnings`

### Step 5 — Deferred todos

- [x] Create todo `improve-session-data-loading.md`: on startup fetch last N
  sessions' full event sets (not a flat event limit); per-session ring-buffer
  eviction (evict oldest complete session, not oldest individual event).
- [x] Create todo `collect-user-turn-messages.md`: capture human prompts
  (user messages that initiate turns). New event type in the pipeline
  (plugin → acuity → curator). Currently only agent events are visible.

### Step 6 — Manual live QA

- [ ] Events are grouped by session with no interleaving
- [ ] Sessions sorted newest-started on top; layout is stable — new session
  adds one row at top, existing sessions don't move
- [ ] Cursor stays on same session when a new session arrives (identity-based)
- [ ] `j/k` routes correctly to the active pane
- [ ] `Tab` switches panes; inactive pane selection is dimmed
- [ ] `z` expands active pane to fullwidth; pressing again restores split
- [ ] Session rows show color-coded project (Magenta) / harness (Blue) /
  title (Cyan+Bold or DarkGray placeholder)
- [ ] Events pane block title shows selected session title
- [ ] Empty events pane shows `(no events)` placeholder
- [ ] Diagnostics view unaffected (help bar unchanged)
- [ ] Mark plan `status: complete`; transition `activity-feed-rendering.md`
  master task to `complete` (fill evidence for manual QA criterion)

---

## Out of scope

- Stage-C turn-folding (Pane 2 expand/collapse individual turns)
- Per-session scroll position memory (Pane 2 always starts at top on session switch)
- Seq-based event selection stability within Pane 2
- Session grouping by project (Pane 1 grouping headers)
- User message collection (todo created in Step 5)
- Session data loading improvements (todo created in Step 5)
