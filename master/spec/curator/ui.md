# Curator UI/UX Specification

`curator` is a read-only TUI dashboard for the cue ecosystem. It surfaces the
state of cue work artifacts and live agent observability in three views,
toggled with `1`, `2`, `3` (gitui-style).

Curator does not mutate artifacts in-process. The only mutation side-channel
is launching `$EDITOR` on a selected artifact; curator rescans the affected
view after the editor closes so the display never goes stale.

## Configuration

- `acuity_url` — base URL of the acuity server (SSE + query API).
- Project registry — read via `cuelib`; determines the set of projects
  available to the kanban view's multi-project mode.

## Global Keybindings

| Key | Action |
| --- | ------ |
| `1` `2` `3` | Switch view (kanban / activity / diagnostics) |
| `r` | Refresh / rescan the active view's data source |
| `q` | Quit |
| `Up` `Down` | Navigate list selection |
| `Enter` | Drill into the selected item (view-specific) |
| `f` | Open the filter panel for the active view |

## View 1 — Kanban

A three-column board of `task` artifacts read from `.cue/master/task/` via
`cuelib`. Columns segment by status: **Open**, **In-Progress**, **Complete**.
`Closed` tasks are excluded from the board.

### Data

- `task` artifacts on the master branch only.
- Per card: title, status, priority, branch.
- Within a column, cards are ordered by priority descending
  (critical -> high -> normal -> low).

### Filters

- Status (column visibility).
- Priority band.
- Project scope: toggle between single-project and multi-project (all
  registered projects) board.

### Actions

- `f` — open filter panel.
- `r` — rescan `.cue/`.
- `e` — open the selected card in `$EDITOR`; auto-rescan on editor close.
- `Enter` — reserved for card detail (deferred).

### MVP

- Three-column board, status + priority filters, fixed priority-desc ordering.
- Multi-project toggle (single <-> all registered).
- `r` rescan, `e` open in `$EDITOR` + auto-rescan, `1/2/3` switch, `q` quit.

### Deferred

- Fuzzy search (`/`) — Phase 6 follow-up.
- User-selectable sort orders — Phase 6 follow-up.
- Plans/todos columns or drill-in — Phase 7.
- Card detail pane (rendered markdown body) — Phase 7 (`$EDITOR` covers the
  read-the-body need for MVP).

## View 2 — Activity Feed

A reverse-chronological list of agent activity sourced from acuity.

### Data Arrival

Curator opens a single SSE connection to `GET /events/stream` with **no
`Last-Event-ID`**. The acuity handler drains the full backlog from `seq > 0`
and then tails live; there is no separation between history and live updates.
Every SSE message is treated identically: deserialize the `EventRecord`,
deserialize its `payload` to `AcuityEvent`, and push into the in-memory store.

Curator maintains a **ring buffer** (capped, e.g. 2000 events) discarding the
oldest entries as new ones arrive, bounding memory without any server-side
change. On disconnect, curator reconnects sending `Last-Event-ID: <last seq>`
so the drain resumes from the cursor rather than from `0`.

### Session State

Session summaries are **derived client-side** by folding the in-memory event
stream into a map keyed by `session_id`:

- Identity and project: `SessionIdle { session_id, project_dir, session_title }`.
- Activity span: min/max `received_at` across the session's events.
- Token totals: summed `input_tokens` / `output_tokens` from
  `AgentTurnCompleted` payloads.
- Error counts: `tool_call_completed` payloads with `is_error: true`.

Cross-project attribution uses `project_dir` from `SessionIdle`, satisfying
the "remind me what I did yesterday across which sessions" use case. A
`/sessions` aggregate endpoint is a future optimization, not a prerequisite.

### Display

- Flat reverse-chronological list, navigable with `Up`/`Down`.
- Per row: `received_at`, `event_type`, short `session_id`, and a one-line
  summary derived from the payload.
- Session grouping shown as derived headers for project attribution.

### Filters

MVP has no user-controlled filter; the default is "all events in the ring
buffer, newest first".

### MVP

- Single SSE connection (backlog drain + live tail).
- Flat list, up/down navigation, per-row summary.
- Client-side session grouping as headers.
- Ring buffer cap; reconnect-with-cursor.

### Deferred

- Collapsing/drilling into sessions -> turns -> tool calls — Phase 6 follow-up.
- User-controlled filtering of the feed — Phase 7.
- Bounded-history windows ("last 24h") — Phase 7.

## View 3 — Diagnostics

A live list of tool-call activity, emphasizing tool usage and errors. Reuses
the **same SSE subscription** as View 2 — one stream, two views — filtered
client-side to tool-call events.

### Data

- `tool_call_requested` and `tool_call_completed` events.
- `tool_name`, `is_error`, and `error_text` are read from the deserialized
  payload (no schema change required).

### Display

- Live event list, visually distinguishing `tool_call_requested` vs
  `tool_call_completed`.
- Success vs error highlighted (color/icon) from `is_error`.
- Error text surfaced for failed calls.

### MVP

- Live tool-call list reusing the activity feed's SSE stream.
- Call-type and success/error distinction with error highlighting.

### Deferred

- DB retention / housekeeping (record counts, delete historical data) —
  Phase 7+. This will require a new write/admin endpoint on acuity; noted
  here as a future concern, not scoped.
- Aggregation views (error rates, call frequency, latency) — later,
  demand-driven.

## acuity-api Evolution Notes

The acuity-api read surface (`EventRecord`, `EventsPage`, the `AcuityEvent`
re-export) is complete for the curator MVP. No change blocks the current
`feat/acuity-api-mvp` PR. Future additions are additive:

- `/sessions` aggregate endpoint — Phase 6+, only if client-side folding
  proves too slow or memory-hungry.
- `since=<timestamp>` query param — Phase 6, only if the launch backlog
  drain becomes perceptibly heavy. Note: acuity has no index on
  `received_at`; a timestamp filter today is a full scan.
- Aggregation endpoints (error rates, counts) — later, demand-driven;
  derivable client-side until then.

`tool_name` promotion to a top-level `EventRecord` field is **rejected**: it
is the sole breaking change (new required field on a shared struct) and
contradicts the deliberate "row mirrors DB column" design. Curator reads
`tool_name` from the deserialized payload.

## Trigger Conditions (when to promote deferred work)

- **Fuzzy search** — when a single project's task count makes a kanban column
  exceed one screen.
- **Collapsing/drilling in activity feed** — when a typical session produces
  enough tool-call events that the flat list buries session boundaries.
- **`/sessions` endpoint** — when client-side folding is demonstrably too
  slow or memory-hungry (unlikely at single-developer volumes).
- **`since` param** — when the SSE backlog drain on launch becomes
  perceptibly heavy.
