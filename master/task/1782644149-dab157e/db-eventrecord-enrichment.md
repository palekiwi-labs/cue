---
title: 'DB + EventRecord: project_dir and harness columns'
status: open
priority: high
---
# DB + EventRecord: project_dir and harness columns

Add `project_dir` and `harness` as first-class SQLite columns in `acuity` and
as fields on `EventRecord` in `acuity-api`. Add a startup column-mismatch guard.

Depends on: schema-enrichment task (needs the new accessors).

## Source

- spec: `spec/1782644149-dab157e/curator-improvements.md` (F1)
- plan: `plan/1782644149-dab157e/curator-improvements.md` (Slice 2)

## Acceptance Criteria

| # | Criterion | Verify by | Evidence |
|---|-----------|-----------|----------|
| 1 | `events` table has `project_dir TEXT NOT NULL` and `harness TEXT NOT NULL` columns | `PRAGMA table_info(events)` in test | |
| 2 | `idx_events_project` index created | DB schema inspection in test | |
| 3 | Startup column-mismatch guard: stale v1 DB → clear error, refuses to start | unit test with a v1 schema file | |
| 4 | `insert_event` binds `project_dir` and `harness` | DB test: fetch row, assert fields persisted | |
| 5 | `query_events_after` SELECTs and maps `project_dir` and `harness` | DB query test | |
| 6 | `EventRecord` in `acuity-api` has `project_dir: String` and `harness: String` | code review | |
| 7 | All `acuity` workspace tests pass | `cargo test -p acuity` | |
