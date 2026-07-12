---
title: Normalize events storage with a sessions dimension table
status: open
priority: low
refs:
- .cue/feat-curator-activity-item/todo/1782659497-0c2ff37/normalize-events-sessions-table.md
- .cue/master/spec/index.md
---
# Normalize events storage with a sessions dimension table

The acuity DB has a single `events` table where session-level attributes
(`project_dir`, `harness`) are denormalized onto every event row. In practice
both are workspace-constant today. Introduce a `sessions` dimension table so
session-level data lives once per session and the `events` table keeps only
event-intrinsic fields plus a `session_id` FK.

## Source

- Original todo (rationale + migration sketch): `.cue/feat-curator-activity-item/todo/1782659497-0c2ff37/normalize-events-sessions-table.md`
- Spec: `.cue/master/spec/index.md`

## Trigger

Action when **any of**:
1. Adding the "pi" harness (or any second harness), OR
2. Wanting session summaries to persist across curator restarts, OR
3. Cross-workspace / cross-harness aggregation lands.

Not urgent at current scale (single harness, single workspace).

## Acceptance Criteria

| # | Criterion (outcome) | Verify by | Evidence |
|---|---------------------|-----------|----------|
| 1 | `sessions` table exists with id PK, parent_id (self-ref), harness, agent, model, title, token aggregates, error_count, timestamps | schema inspection | |
| 2 | `events` table drops `project_dir`/`harness` columns and gains `session_id` FK (cascade) | schema inspection | |
| 3 | Curator query path joins `sessions` for rendering | code review | |
| 4 | SSE `EventRecord` shape carries the joined session descriptor (or curator issues a separate sessions fetch on launch) | code review | |
| 5 | Drop-and-recreate DB works (no back-compat per prototyping constraints) | manual restart | |
| 6 | Adding a new harness value requires no schema change (harness is a value, not a structure) | test with "pi" harness string | |
| 7 | `cargo test --workspace` green | test run | |
| 8 | `cargo clippy --workspace -- -D warnings` clean | clippy run | |