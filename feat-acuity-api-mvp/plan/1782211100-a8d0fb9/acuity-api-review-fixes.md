---
status: complete
---
# Executive plan: acuity API review fixes

## Foreword

This plan implements the agreed fixes from the consultant-opus code review of
the `feat/acuity-api-mvp` branch (full review at
`trace/1782211100-a8d0fb9/opus-review-acuity-api-mvp.md`; task at
`master/task/1782211100-a8d0fb9/acuity-api-review-fixes.md`).

It addresses two critical correctness bugs (pagination contract + swallowed DB
errors) plus partial SSE hardening and Cargo hygiene. Decisions on the two open
design questions are recorded in the master plan's sibling index and were
confirmed with the user: explicit `next_after: Option<i64>` cursor, plain `500`
status (no JSON body).

`curator` does not yet consume this API, so changing `EventsPage`'s shape has
no breaking consumer. Implementation is TDD vertical slices (one RED test →
minimal GREEN → commit per concern).

Prerequisites: branch is `feat/acuity-api-mvp`; merge base
`8330d03`. All work is in `crates/acuity-api` and `crates/acuity`. Run tests
with `cargo test -p acuity -p acuity-api`.

## Steps

### Slice 1 — Pagination cursor (Critical #1)
- [x] 1.1 RED: add test asserting `next_after` is `None` on a short page (empty
  DB / fewer rows than the clamped limit).
- [x] 1.2 RED: add test asserting `next_after == Some(last.seq)` on a full
  page (insert >limit events, request a small `limit`).
- [x] 1.3 GREEN: add `next_after: Option<i64>` to `EventsPage`; rewrite
  contract doc; change `query_events_after` to return
  `(Vec<EventRecord>, Option<i64>)`; thread through handler.
- [x] 1.4 Lint/fmt/test, commit.

### Slice 2 — Surface DB errors as 500 (Critical #2)
- [x] 2.1 RED: add test that closes the pool then calls `GET /events` and
  asserts `500` (not `200` with empty page).
- [x] 2.2 GREEN: change `query_events` to `Result<Json<EventsPage>, StatusCode>`,
  map `Err` to `INTERNAL_SERVER_ERROR`.
- [x] 2.3 Lint/fmt/test, commit.

### Slice 3 — SSE hardening (Major #3/#4/#5 + nits)
- [x] 3.1 Extract `const SSE_PAGE_SIZE: i64 = 50;`; bound the drain to
  `MAX_DRAIN_PAGES` iterations per cycle so a busy stream can't starve the
  keepalive/sleep.
- [x] 3.2 Add a comment documenting the single-writer / commit-order == seq
  assumption near the SSE poll loop (Major #5).
- [x] 3.3 Add `Content-Type: text/event-stream` assertion to SSE tests;
  factor the duplicated `data:`-frame reader into one helper.
- [x] 3.4 Lint/fmt/test, commit.

### Slice 4 — Cargo hygiene (nit)
- [x] 4.1 Remove unused `serde_json` dep from `acuity-api/Cargo.toml`.
- [x] 4.2 `cargo build -p acuity-api` + full workspace clippy/test, commit.

### Closeout
- [x] 5.1 Final `cargo clippy --all-targets --workspace` + `cargo fmt --check`
  + `cargo test --workspace` green.
- [x] 5.2 `cue-log` milestone; update task acceptance evidence; create deferred
  `todo` for broadcast-channel redesign + disconnect test.
