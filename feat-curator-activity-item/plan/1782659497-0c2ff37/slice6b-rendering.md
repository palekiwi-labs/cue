---
status: open
---
## Foreword

Consumes the 6a lineage data to make the curator activity feed legible.
Retains the **flat reverse-chrono layout** (stage C owns the nested tree).
Kills the 8-char session-id collision (`ui.rs:214`), surfaces title + agent +
lineage in headers, and adds dev debug columns so the data relationships can be
verified by eye.

This is the "verify our assumptions" interim view (stage B) before the stage-C
nested tree. It does **not** wire `build_activity_items` yet — its
`(session_id, project_dir)` header key is known-degenerate (`project_dir` is
workspace-constant, confirmed across all 465 rows of the snapshot DB) and will
be reworked in stage C with `parent_id`-based nesting.

**Branch:** `feat/curator-activity-item`
**Test exit:** `cargo test -p curator` green + `cargo clippy -p curator -- -D warnings` clean.

> **Note:** This plan is intentionally lighter than 6a. It will be fleshed out
> fully when 6a's live data is in hand, since rendering details may shift based
> on what the real `SessionUpdated` rows reveal.

### Steps

- [ ] **1. `crates/curator/src/ui.rs` `session_header` (`213-234`) — kill the truncation.**
  Remove `.get(..8)`. Resolve `sessions.get(&record.session_id)` for `title` / `agent` / `parent_id` / `harness`. Format primary vs child distinctly, e.g.
  primary: `── <title> · <agent> · primary (…<last6>) ──`
  child:   `── <title> · <agent> · child of …<parent_last6> (…<last6>) ──`
  Define fallbacks for unknown sessions (no `SessionSummary` entry yet).

- [ ] **2. `crates/curator/src/ui.rs` `render_activity` (`152-210`) — debug columns.**
  Add a compact lineage suffix per row (agent or model tag, compact `turn_id`), within column-width limits — possibly behind a dev toggle. Preserve the selection-index ↔ visual-row mapping (`idx == app.sel_activity`).

- [ ] **3. `crates/curator/src/activity.rs` — document the degenerate header key.**
  Add a module-level note that `(session_id, project_dir)` collapses to `session_id` (project_dir constant) and is deferred to stage C. Do not wire `build_activity_items` in.

- [ ] **4. Tests.**
  Unit tests for the new header formatting (extract a pure helper if helpful); assert primary vs child vs unknown-session strings.

- [ ] **5. Verify.**
  `cargo test` + clippy. Manual: replay the DB-snapshot scenario; confirm the three interleaved sessions now render with distinct, legible headers and visible per-row lineage.

### Out of scope

- Stage C: nested tree, folding child sessions under parent turns, the `build_activity_items` rewrite with `parent_id`-based nesting.
- Sessions-table normalization (see todo `normalize-events-sessions-table.md`).
