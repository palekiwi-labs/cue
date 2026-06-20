---
title: "acuity-api: query API and SSE stream"
status: open
priority: normal
---
# acuity-api: query API and SSE stream

Define `acuity-api`'s read/response types. Implement `acuity`'s outbound
surface: a historical query API and a real-time SSE stream. These are the
endpoints `curator` will consume in the following phase.

Building after a full emitter (Phase 4) ensures real data is available in
SQLite to design query shapes against rather than synthetic fixtures.

## Source

- spec: `.cue/master/spec/acuity/index.md`
- roadmap: `.cue/master/trace/1781942441-cef325f/cue-ecosystem-roadmap.md` (Phase 5)

## Acceptance Criteria

| #  | Criterion                                                              | Verify by                                   | Evidence |
| -- | ---------------------------------------------------------------------- | ------------------------------------------- | -------- |
| 1  | HTTP query endpoint returns sensible results against real stored events | curl query endpoint with real data in SQLite |          |
| 2  | SSE stream delivers live events as an agent runs                       | subscribe to SSE, run agent, observe events  |          |
| 3  | Both are validated independently of `curator`                          | human attestation                            |          |
