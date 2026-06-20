---
title: "acuity: full event model + SQLite persistence"
status: open
priority: normal
---
# acuity: full event model + SQLite persistence

Extend `acuity-schema` with all three lifecycle event types
(session.idle, tool-call-requested, tool-call-completed) and the ingest
envelope (`seq`, `received_at`). Regenerate `types.ts`. Add SQLite persistence
to `acuity`'s POST ingest handler.

Builds on the stateless MVP ingest path from Phase 1 — the POST endpoint and
header validation are already proven; this phase widens the schema and adds
storage.

## Source

- spec: `.cue/master/spec/acuity/index.md`
- roadmap: `.cue/master/trace/1781942441-cef325f/cue-ecosystem-roadmap.md` (Phase 3)

## Acceptance Criteria

| #  | Criterion                                                                 | Verify by                                    | Evidence |
| -- | ------------------------------------------------------------------------- | -------------------------------------------- | -------- |
| 1  | `curl` POSTs of all three event types are accepted by acuity              | curl each event type, inspect response       |          |
| 2  | Events land in SQLite with correct `seq` and `received_at` fields         | inspect SQLite after POSTs                   |          |
| 3  | A wrong `X-Acuity-Schema` version is cleanly rejected                     | send bad-version POST with curl, observe 400 |          |
