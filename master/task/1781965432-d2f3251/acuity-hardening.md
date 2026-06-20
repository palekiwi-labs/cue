---
title: "Hardening: auth, retention, multi-host"
status: open
priority: low
---
# Hardening: auth, retention, multi-host

Address the deferred production concerns. This task is a placeholder and will
likely be split into multiple tasks when the time comes.

Concerns to address:
- Auth/trust boundary for `acuity` POST endpoints (mandatory before any
  multi-host deployment)
- SQLite retention and pruning policy
- Second harness plugin in `cue-plugins`
- Multi-host connection config for `curator <-> acuity`

## Source

- spec: `.cue/master/spec/acuity/index.md`
- roadmap: `.cue/master/trace/1781942441-cef325f/cue-ecosystem-roadmap.md` (Phase 7)

## Acceptance Criteria

| #  | Criterion                                                              | Verify by                                        | Evidence |
| -- | ---------------------------------------------------------------------- | ------------------------------------------------ | -------- |
| 1  | `acuity` runs on a separate networked host with authenticated ingest   | deploy to separate host, send authenticated POST |          |
| 2  | Unauthenticated POSTs are rejected                                     | send unauthenticated POST, observe 401           |          |
| 3  | SQLite growth is bounded by a retention/pruning policy                 | human attestation                                |          |
| 4  | `curator` connects to a remote `acuity` via config                     | human attestation                                |          |
