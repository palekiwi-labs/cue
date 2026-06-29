---
title: 'Server: project_dir filter on GET /events'
status: complete
priority: normal
---
# Server: project_dir filter on GET /events

Add an optional `project_dir` query parameter to the `GET /events` endpoint.

Depends on: db-eventrecord-enrichment task.

## Source

- plan: `plan/1782644149-dab157e/curator-improvements.md` (Slice 3)

## Acceptance Criteria

| # | Criterion | Verify by | Evidence |
|---|-----------|-----------|----------|
| 1 | `GET /events?project_dir=<path>` returns only rows matching that project_dir | integration test | |
| 2 | `GET /events` without `project_dir` returns all rows (no regression) | integration test | |
| 3 | Filter composes with existing `session_id` and `event_type` filters | integration test | |
| 4 | All `acuity` workspace tests pass | `cargo test -p acuity` | |
