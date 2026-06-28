---
title: 'Curator: ActivityItem pure build function'
status: open
priority: high
---
# Curator: ActivityItem pure build function

Define the `ActivityItem` enum and implement the `build_activity_items` pure
function that transforms the events ring buffer into a structured rendering model.

Depends on: db-eventrecord-enrichment task (needs `project_dir` + `harness` on `EventRecord`).

This is the highest-risk slice. Over-test it.

## Source

- spec: `spec/1782644149-dab157e/curator-improvements.md` (F2)
- plan: `plan/1782644149-dab157e/curator-improvements.md` (Slice 5)

## Acceptance Criteria

| # | Criterion | Verify by | Evidence |
|---|-----------|-----------|----------|
| 1 | `ActivityItem` enum defined with `SessionHeader`, `Turn`, and `Standalone` variants | code review | |
| 2 | `build_activity_items` is a pure function (no side effects, no App mutation) | code review | |
| 3 | Tool-call grouping keyed on `(session_id, turn_id)` via HashMap — not positional adjacency | code review + test | |
| 4 | Two interleaved sessions with the same `turn_id` do not cross-contaminate | unit test | |
| 5 | Orphan tool call (parent turn evicted from ring buffer) renders as `Standalone`, no panic | unit test | |
| 6 | Tool call arriving by `seq` before its `agent_turn_completed` still groups correctly | unit test | |
| 7 | Session-context header emitted on `project_dir` change within the same `session_id` | unit test | |
| 8 | Empty `fold_state` → tool calls counted but not included in item list (all folded) | unit test | |
| 9 | Non-empty `fold_state` → matching turn includes its tool calls | unit test | |
| 10 | `push_event` sets `SessionSummary.project_dir` from every event type (not only `SessionIdle`) | unit test | |
| 11 | All curator workspace tests pass | `cargo test -p curator` | |
