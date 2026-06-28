---
status: open
---
## Foreword

This plan implements the curator UX improvements spec across 8 slices.
Slices 1-4 are cross-crate data-layer changes; slices 5-8 are curator TUI
changes. Every slice must end with a green test suite and a demoable state.

Spec: `spec/curator/curator-improvements.md`

**Dependency order:**
- Slices 1 and 2 gate slices 5-8: curator needs `project_dir` + `harness` on
  `EventRecord` before any rendering work can begin.
- Slice 4 (plugin, external repo) is independent and must be deployed **after**
  the Slice 2 server is live.
- Slices 5 → 6 → 7 → 8 are strictly ordered within curator.

---

## Slice 1 — `acuity-schema`: new fields and accessors

**Scope:** `crates/acuity-schema/src/lib.rs`, codegen binary.

Changes:
- Add `project_dir: String` and `harness: String` to all four event structs
  (`SessionIdle`, `AgentTurnCompleted`, `ToolCallRequested`, `ToolCallCompleted`).
  `SessionIdle` already has `project_dir`; add `harness` there and add both to
  the other three.
- Add `project_dir(&self) -> &str` and `harness(&self) -> &str` accessors to
  `AcuityEvent` impl, mirroring the existing `session_id()` / `turn_id()` pattern.
- Regenerate `types.ts` via `nix run .#update-types` (or the codegen binary directly).
- Update all existing fixture constructors in the schema test suite with the new
  required fields. Add accessor tests.

**Test exit:** `cargo test -p acuity-schema` green.
**Demo:** `cargo test -p acuity-schema`.

---

## Slice 2 — `acuity` + `acuity-api`: DB columns, `EventRecord`, column guard

**Scope:** `crates/acuity/src/db.rs`, `crates/acuity-api/src/lib.rs`.

Changes:
- New DDL: add `project_dir TEXT NOT NULL` and `harness TEXT NOT NULL` columns
  to the `events` table. Add `idx_events_project ON events(project_dir)` index.
- **Startup column-mismatch guard**: on `connect()`, run
  `PRAGMA table_info(events)` and verify the expected columns exist. If not,
  log a clear error ("stale events.db detected — delete the file and restart")
  and return `Err`. This prevents silent insert failures against a v1 file.
- Update `insert_event` to bind `project_dir` and `harness` using the new
  accessors from Slice 1.
- Update `query_events_after` SELECT to include `project_dir` and `harness`;
  update the row-mapping closure to populate `EventRecord`.
- Add `project_dir: String` and `harness: String` to `EventRecord` in
  `acuity-api/src/lib.rs`.
- Update all `db.rs` test fixture constructors to include the new fields.
  Add a `project_dir` column-presence test and a stale-file-detection test.

**Test exit:** `cargo test -p acuity` green.
**Demo:** `cargo test -p acuity`.

---

## Slice 3 — `acuity` server: `project_dir` filter on `GET /events`

**Scope:** `crates/acuity/src/main.rs`, `crates/acuity/src/db.rs`.

Changes:
- Add `project_dir: Option<String>` to `EventsQuery` in `main.rs`.
- Pass it through to `query_events_after`; the function already has the
  dynamic query-builder pattern — add the predicate the same way `session_id`
  and `event_type` are handled.
- Add a `project_dir` filter test in `tests.rs` mirroring the existing
  `session_id` filter test.

**Test exit:** `cargo test -p acuity` green.
**Demo:** `curl 'http://localhost:33222/events?project_dir=/my/project'` returns
only matching rows.

---

## Slice 4 — `cue-plugins`: populate new fields (external repo, deploy last)

**Scope:** `~/.config/opencode/plugin/palekiwi-labs/cue-plugins/src/opencode/acuity-plugin.ts`.

Changes:
- Add `project_dir: directory` and `harness: "opencode"` to all four event
  payload objects. `directory` is already in the plugin closure.
- No other changes.

**Deployment order:** server (Slice 2) must be live before this is deployed.
Plugin is in the external `cue-plugins` repo; change tracked separately.

**Test exit:** manual smoke — run an agent session, query `GET /events`, confirm
rows have non-empty `project_dir` and `harness` columns.
**Demo:** `curl http://localhost:33222/events | jq '.[0].project_dir'`.

---

## Slice 5 — `curator`: `ActivityItem` pure build function

**Scope:** new `crates/curator/src/activity.rs` (or inlined in `app.rs`).

This is the highest-risk slice. Over-test it.

Changes:
- Define `ActivityItem` enum:
  ```
  SessionHeader { session_id, project_dir, harness, session_title }
  Turn { turn_id, session_id, agent_turn: &EventRecord, tool_calls: Vec<&EventRecord> }
  Standalone(&EventRecord)
  ```
  Prefer `&EventRecord` borrows over owned clones (items are render-pass-scoped).
- Implement `build_activity_items(events, sessions, fold_state) -> Vec<ActivityItem>`.
  Algorithm (reverse-chrono iteration):
  1. Track `prev_context: Option<(session_id, project_dir)>`. On change → emit
     `SessionHeader`. Resolve `session_title` from `sessions` HashMap.
  2. `agent_turn_completed` → emit `Turn`; insert into `HashMap<(session_id, turn_id), item_idx>`.
  3. `tool_call_*` → look up `(record.session_id, record.turn_id)` in the HashMap;
     if found append to that Turn's `tool_calls`; otherwise emit `Standalone`.
  4. `session_idle` and other events → emit `Standalone`.
- Update `push_event` in `app.rs` to set `SessionSummary.project_dir` from
  `record.project_dir` on every event (not just `SessionIdle`).

Required test cases (these are the Opus-identified failure modes):
- Two interleaved sessions with the same `turn_id` — tool calls must not
  cross-contaminate.
- Orphan tool call whose parent turn was evicted from the ring buffer —
  must render as `Standalone`, not panic.
- Tool call arriving (by `seq`) before its `agent_turn_completed` — grouping
  must still work correctly after full build.
- Session-context header emitted on `project_dir` change within same `session_id`.
- `fold_state` empty → all turns count as folded (headers + turn summaries only).
- `fold_state` containing a turn_id → that turn expands to include tool calls.

**Test exit:** `cargo test -p curator` green (pure unit tests, no TUI frame).
**Demo:** unit test output.

---

## Slice 6 — `curator`: activity view renders from `ActivityItem` + fold state

**Scope:** `crates/curator/src/app.rs`, `crates/curator/src/ui.rs`.

Changes:
- Add `fold_state: HashSet<(String, String)>` (keyed by `(session_id, turn_id)`)
  to `App`. Default: empty (all folded).
- Add `sel_activity_id: ActivitySelId` (enum: `Header(...)`, `Turn(session, turn)`,
  `Standalone(seq)`) to `App`, replacing the raw `sel_activity: usize` index.
  On each scroll, rebuild the items list, resolve the identity's new visual index,
  and scroll to it.
- Rewrite `render_activity` to consume `build_activity_items`. Render items in
  order: `SessionHeader` as a styled DarkGray line; `Turn` as a turn-summary line
  (folded) or turn + indented tool calls (unfolded); `Standalone` as a plain row.
- New header format: `── opencode  cue  (session title)` (harness, project_dir
  basename, session_title truncated to available width).
- Tool call indentation: 4-space prefix prepended to the tool-call `ListItem`.
- `App::toggle_fold_at_cursor()`: resolves `sel_activity_id`, extracts the
  `(session_id, turn_id)` key, and toggles it in `fold_state`.

**Test exit:** `cargo test -p curator` green.
**Demo:** launch curator against live acuity; feed is compact (folded); Space
expands a turn to reveal tool calls.

---

## Slice 7 — `curator`: navigation + view renumber

**Scope:** `crates/curator/src/event.rs`, `input.rs`, `main.rs`, `app.rs`, `ui.rs`.

Changes:
- Extend `View` enum: `Projects` (new), shift others to `Kanban/Activity/Diagnostics`.
- New `Action` variants: `ToggleFold`, `ScrollToTop`, `ScrollToBottom`, `PageUp`,
  `PageDown`, `ToggleSelect`.
- `input.rs` key map:
  - `'1'` → `SwitchView(Projects)`, `'2'` → Kanban, `'3'` → Activity, `'4'` → Diagnostics.
  - Space → `ToggleFold`.
  - `Home` / `End` → `ScrollToTop` / `ScrollToBottom`.
  - `PageUp` / `PageDown` → `PageUp` / `PageDown`.
  - Left/Right remain as `Action::Left` / `Action::Right`.
- `main.rs` / `process_msg` dispatch:
  - Left/Right: column nav in Kanban, `toggle_fold_at_cursor()` in Activity,
    no-op elsewhere.
  - `ToggleFold` → `toggle_fold_at_cursor()` (same as Left/Right in Activity).
  - `ScrollToTop/Bottom` → set `sel_activity_id` to first/last item.
  - `PageUp/Down` → advance `sel_activity_id` by `page_height` items.
- Update **all three** help-bar strings in `ui.rs` to reflect the new bindings.
  Ensure the Projects help bar is included when the view is written in Slice 8.

**Test exit:** `cargo test -p curator` green; nav-math unit tests for clamping.
**Demo:** all four view keys work; Space folds/unfolds; Home/End/PgUp/PgDn navigate.

---

## Slice 8 — `curator`: Projects view + project filter

**Scope:** `crates/curator/src/app.rs`, `main.rs`, `ui.rs`.

Changes:
- Add to `App`:
  ```rust
  pub project_store: ProjectStore,
  pub sel_projects: usize,
  pub selected_project_dirs: HashSet<String>,  // empty = no filter
  ```
- Load `ProjectStore` at startup (non-fatal — use empty store on error, log warning).
  Reload on `Action::Refresh`.
- `render_projects` (new function in `ui.rs`): scrollable list, one row per
  registered project. Columns: `[x]`/`[ ]` checkbox, harness (derived from most
  recent event in the ring buffer for that `project_dir`, or `—`), project key,
  last-event timestamp (from `SessionSummary` aggregated by `project_dir`).
  Sort by last-event descending; zero-event projects at the bottom with `—`.
- Toggle per-project selection: Space/Enter → insert/remove `project_dir` from
  `selected_project_dirs`. Key `a`: if all selected, deselect all; otherwise select
  all.
- Apply filter in `render_activity` and `render_diagnostics`:
  if `selected_project_dirs` is non-empty, skip events whose `record.project_dir`
  is not in the set. Zero-selected = passthrough (show all).
- Add a one-line hint in `render_kanban` that the project filter does not apply
  to the kanban board.
- Projects help-bar string added to `ui.rs`.

**Test exit:** `cargo test -p curator` green. Filter unit tests: empty set =
passthrough; non-empty set = subset; zero-event project renders without panic;
sort ordering correct.
**Demo:** select one project in Projects view; Activity narrows to that project's
events only.
