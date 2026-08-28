---
title: "acuity-api: query API and SSE stream"
status: complete
priority: normal
branch: ""
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
| 1  | HTTP query endpoint returns sensible results against real stored events | curl query endpoint with real data in SQLite | PASS — bin/1782211100-a8d0fb9/validate-phase5.sh, all 8 checks green (2026-06-23) |
| 2  | SSE stream delivers live events as an agent runs                       | subscribe to SSE, run agent, observe events  | PASS — bin/1782211100-a8d0fb9/validate-phase5-sse.sh, all 7 checks green (2026-06-23) |
| 3  | Both are validated independently of `curator`                          | human attestation                            | PASS — both validated via raw curl, no curator involvement (user attested 2026-06-23) |
