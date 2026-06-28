# Curator UX Improvements

## Intent

Improve the practical usability of `curator` following Phase 6 integration.
Three pain points were observed during daily use:

1. **Opaque activity feed**: All events display fully expanded at all times.
   Session headers are indistinguishable because the short session ID truncates
   to a shared prefix, making all groups look identical.

2. **Missing provenance data**: Events lack the project directory and harness
   context needed to understand where activity originated. Only `SessionIdle`
   carries `project_dir`; other events require a fragile HashMap lookup that
   breaks when a session switches project context mid-turn.

3. **No cross-project view**: Curator is anchored to a single project at
   startup. Registered projects in the `ProjectStore` are invisible to the TUI
   and there is no way to filter activity by project.

## Features

### F1 — Richer event data

Every acuity event carries the originating `project_dir` and `harness`
(e.g. `"opencode"`). These fields are persisted as first-class database columns
and surfaced on `EventRecord` so curator never parses raw payload JSON for
project attribution. A `project_dir` filter is added to `GET /events`.

The `cue-plugins` opencode plugin populates both fields from the data already
available in the plugin closure (`directory` and the hardcoded harness name).

### F2 — Activity feed redesign

The activity feed organises events into a two-level tree:

- **Session-context blocks**: a header line emitted whenever the
  `(session_id, project_dir)` pair changes. Format:
  `── opencode  cue  (session title)`.
- **Agent turns**: each `agent_turn_completed` groups its associated tool calls
  under a `Turn` item keyed by `(session_id, turn_id)`.

By default all turns are **folded** — the feed shows only turn summaries,
making it scannable. The user expands individual turns to inspect tool calls,
which are indented 4 spaces under their parent.

### F3 — Enhanced navigation

New keyboard bindings:
- **Space**, **Left**, **Right** (when in Activity view) — fold / unfold the
  selected turn.
- **Home** / **End** — scroll to top / bottom of the list.
- **PgUp** / **PgDn** — page scroll.

Selection tracks logical item identity (not a raw visual index) so it survives
fold/unfold operations and live SSE event arrivals without drifting onto an
unrelated row.

### F4 — Projects view and cross-project filtering

A new Projects view (key `1`) lists all projects registered in the
`ProjectStore`, showing each project's harness and the timestamp of its most
recent event. Projects are sorted by recency; projects with no events appear at
the bottom.

The user selects or deselects projects (Space/Enter per project; `a` to toggle
all). The selection narrows the Activity and Diagnostics views to matching
`project_dir` values. Zero projects selected means no filter — all events shown.

Views are renumbered: `1`=Projects, `2`=Kanban, `3`=Activity, `4`=Diagnostics.
All help-bar strings updated to match.

The kanban board is unaffected by the project filter in this iteration.

## Out of scope (this iteration)

- Schema version header / backward-compat enforcement
- `harness_version` field
- `hostname` field
- Multi-project kanban
- Collapsible session-context headers (session-level folding)
- Server-side project filtering in the SSE stream
- Single-source-of-truth help-bar text

## Deferred technical debt

The `project_dir` value appears in three places per event: the event struct
field, the SQLite column, and the raw `payload` JSON. This tripling is accepted
for now; a deduplication pass is captured as a separate todo.
