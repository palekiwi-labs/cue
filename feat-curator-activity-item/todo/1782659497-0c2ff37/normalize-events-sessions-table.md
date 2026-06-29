---
status: open
priority: low
---
# Normalize events storage with a `sessions` dimension table

## Context

The acuity DB has a single `events` table (`crates/acuity/src/db.rs:12-29`).
Session-level attributes — `project_dir` and `harness` — are denormalized onto
**every** event row:

- struct fields: `crates/acuity-schema/src/lib.rs:22-23, 33-34, 45-46, 71-72`
- table columns: `crates/acuity/src/db.rs:19-20`
- accessor methods: `crates/acuity-schema/src/lib.rs:118-136`

In practice `project_dir` is identical across the entire table (one workspace)
and `harness` is always `"opencode"`. Confirmed against a 465-row snapshot
where every row carried `/home/pl/code/palekiwi-labs/cue`.

## Why it's debt (the smell)

- **Duplicates invariant session-level data** across every row (storage + index
  bloat on `idx_events_project`).
- **Carries a recurring maintenance tax:** every denormalized column forces
  synchronized changes across four crates (schema struct → enum accessor → DDL
  → column guard → insert binds → SELECT mapping → `EventRecord`) and grows the
  column-mismatch guard at `crates/acuity/src/db.rs:57-78` (the "stale
  events.db detected" check).
- **Sets a precedent that tempts further denormalization** — it nearly drove us
  to add `parent_id`/`agent`/`model` as per-row columns in Slice 6 (Option V).
  We chose Option L specifically to avoid deepening it; this todo records the
  underlying debt so it is not lost.

## Why normalize — and why it matters for the planned "pi" harness

A second agent harness ("pi") is planned. Each session belongs to exactly one
harness. With a `sessions` table:

- `harness` (alongside `project_dir`, `parent_id`, `agent`, `model`, `title`,
  token totals, `error_count`) lives **once per session**; the `events` table
  keeps only event-intrinsic fields (`seq`, `received_at`, `event_type`,
  `turn_id`, `payload`) plus a `session_id` FK.
- Adding "pi" then requires **no schema changes** — `harness` is a value
  (`"opencode" | "pi" | …`), not a structure. Cross-harness queries join
  against `sessions`, not scan every event. A new harness cannot corrupt or
  compromise existing data because the dimension is isolated.
- Removes the column-mismatch guard's per-column growth and the four-crate
  churn for every new session attribute.
- Enables **persistent session summaries**: curator currently rebuilds
  `SessionSummary` in memory from the event stream on every launch
  (`crates/curator/src/app.rs:178-243`); a `sessions` table lets aggregates
  persist across restarts.

## When to action (trigger)

Action this todo when **any** of:
1. we add the "pi" harness (or any second harness), OR
2. we want session summaries to persist across curator restarts, OR
3. cross-workspace / cross-harness aggregation lands.

## Do NOT action now

Working fine at current scale. Slice 6a/6b focus on lineage capture and
display. Captured here so the decision and rationale are not lost.

## Migration shape (when actioned)

- Introduce `sessions` table: `id` PK, `project_id`, `workspace_id`, `parent_id`
  (self-ref), `harness`, `agent`, `model`, `title`, token aggregates
  (`input`/`output`/`reasoning`/`cache_read`/`cache_write`), `error_count`,
  timestamps. Index `parent_id` (mirrors opencode's `session_parent_idx`).
- `events` table drops `project_dir` / `harness` columns and gains `session_id`
  FK (cascade). Keeps `turn_id` for the intra-session tool-call grouping.
- Curator query path joins `sessions` for rendering; SSE `EventRecord` shape
  updated to carry the joined session descriptor (or curator issues a separate
  sessions fetch on launch).
- Drop-and-recreate the DB (no back-compat, per prototyping constraints — do
  not bump schema versions).
- The acuity plugin becomes one-of-many: a future "pi" plugin emits the same
  acuity wire events with `harness: "pi"`; no acuity-schema change needed.
