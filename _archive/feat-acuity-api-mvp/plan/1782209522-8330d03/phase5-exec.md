---
status: complete
---
# Phase 5 Executive Plan — acuity read model

## Foreword

This plan executes Phase 5 of the cue ecosystem roadmap on the
`feat/acuity-api-mvp` branch. It adds `GET /events` (paginated query) and
`GET /events/stream` (SSE) to `acuity`, and populates `acuity-api` with the
shared response types `curator` will consume in Phase 6.

Prerequisites: Phases 0–4 are merged to master. `acuity-api` is a stub crate.
The SQLite schema (`events` table with `seq` as autoincrement PK) is stable.

Implements: `task/1781965432-d2f3251/acuity-read-model.md`
Master plan: `plan/index.md`
Opus review incorporated (limit clamping, drain-first SSE, error logging,
defensive Last-Event-ID parsing, acuity-schema re-export).

## Steps

- [x] Update master plan with Opus review findings
- [x] **Slice 1 — acuity-api types**
  - [x] Add `acuity-schema` + `serde_json` deps to `acuity-api/Cargo.toml`
  - [x] Define `EventRecord`, `EventsPage`, re-export `AcuityEvent` in `acuity-api/src/lib.rs`
  - [x] `cargo check -p acuity-api` green
  - [x] Commit: `feat(acuity-api): define EventRecord, EventsPage, re-export AcuityEvent`
- [x] **Slice 2 — DB query function**
  - [x] Add `query_events_after` to `acuity/src/db.rs` (QueryBuilder, optional filters, capped limit)
  - [x] `cargo test -p acuity` green (existing tests still pass)
  - [x] Commit: `feat(acuity): add query_events_after to db layer`
- [x] **Slice 3 — GET /events (RED → GREEN)**
  - [x] Add `acuity-api` dep to `acuity/Cargo.toml`
  - [x] Write failing tests: query returns events, after-cursor filter, session_id filter, event_type filter, limit clamping
  - [x] Implement `query_events` handler + `EventsQuery` extractor, extend router
  - [x] All tests green
  - [x] Commit: `feat(acuity): add GET /events paginated query endpoint`
- [x] **Slice 4 — GET /events/stream (SSE)**
  - [x] Add `async-stream` + `futures-core` deps to `acuity/Cargo.toml`
  - [x] Write SSE smoke test (oneshot, first data: frame)
  - [x] Implement `sse_handler`: drain-first poll loop, explicit 15 s keep-alive, defensive Last-Event-ID parsing, logged DB errors
  - [x] Smoke test green
  - [x] Commit: `feat(acuity): add GET /events/stream SSE endpoint`
- [x] **Acceptance verification**
  - [x] `cargo test --workspace` all green
  - [x] `cargo clippy --workspace -- -D warnings` clean
  - [x] curl validation script — all 8 checks green (bin/1782211100-a8d0fb9/validate-phase5.sh)
  - [x] SSE live validation — all 7 checks green (bin/1782211100-a8d0fb9/validate-phase5-sse.sh)
  - [x] Human attestation AC #2 and #3
  - [x] Update task status to complete
