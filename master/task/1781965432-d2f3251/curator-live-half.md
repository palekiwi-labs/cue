---
title: "curator: live acuity integration"
status: open
priority: normal
---
# curator: live acuity integration

Wire `acuity-api` into `curator` as a dependency. Add the live activity view:
SSE-driven real-time agent events and historical aggregates (e.g. token counts
per task, session idle state) overlaid on the artifact kanban from Phase 2.

This is the convergence phase — every input (artifact kanban, acuity ingest,
acuity read model) is independently proven before they are joined here.

## Source

- spec: `.cue/master/spec/curator/index.md`
- roadmap: `.cue/master/trace/1781942441-cef325f/cue-ecosystem-roadmap.md` (Phase 6)

## Acceptance Criteria

| #  | Criterion                                                                              | Verify by                                  | Evidence |
| -- | -------------------------------------------------------------------------------------- | ------------------------------------------ | -------- |
| 1  | `curator` shows the artifact kanban and live agent events updating in real time        | run live agent with acuity up, observe TUI |          |
| 2  | Full ecosystem loop is closed: agent -> plugin -> acuity -> curator                    | human attestation                          |          |
