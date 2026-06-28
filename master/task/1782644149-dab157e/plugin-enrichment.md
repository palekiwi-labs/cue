---
title: 'Plugin: emit project_dir and harness on all events'
status: open
priority: high
---
# Plugin: emit project_dir and harness on all events

Update `acuity-plugin.ts` in the `cue-plugins` repo to include `project_dir`
and `harness` on every posted event payload.

**Deploy after** the acuity server is live with the v2 DB schema (db-eventrecord-enrichment task).

## Source

- spec: `spec/1782644149-dab157e/curator-improvements.md` (F1)
- plan: `plan/1782644149-dab157e/curator-improvements.md` (Slice 4)

## Acceptance Criteria

| # | Criterion | Verify by | Evidence |
|---|-----------|-----------|----------|
| 1 | All four event payload objects include `project_dir: directory` | code review of acuity-plugin.ts | |
| 2 | All four event payload objects include `harness: "opencode"` | code review of acuity-plugin.ts | |
| 3 | Live smoke test: run an agent session, query `GET /events`, confirm rows have non-empty `project_dir` and `harness` | manual | |
