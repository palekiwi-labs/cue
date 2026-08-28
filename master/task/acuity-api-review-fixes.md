---
title: Address opus review of acuity query/SSE API
status: complete
priority: high
---
# Address opus review of acuity query/SSE API

Implement the agreed fixes from the consultant-opus code review of the
`feat/acuity-api-mvp` branch (review saved as
`trace/1782211100-a8d0fb9/opus-review-acuity-api-mvp.md`).

## Source

- `trace/1782211100-a8d0fb9/opus-review-acuity-api-mvp.md` (full review)
- `plan/index.md` on this branch (technical approach)

## Scope (agreed with user)

**In scope now:**
- Critical #1: pagination contract — add explicit `next_after: Option<i64>`
  cursor to `EventsPage` (chosen over "return effective limit").
- Critical #2: surface DB errors as `500` instead of masking as empty
  `200 OK` page (plain `StatusCode`, no JSON error body — matches house style).
- Major #3 (partial): extract `SSE_PAGE_SIZE` const + cap drain iterations
  per cycle so a busy stream cannot starve the keepalive/sleep.
- Major #4 (partial): assert `Content-Type: text/event-stream` in SSE tests;
  factor the duplicated `data:`-frame reader into one helper.
- Major #5 (comment-only): document the single-writer / commit-order == seq
  assumption near the SSE poll loop.
- Nit: remove unused `serde_json` dep from `acuity-api/Cargo.toml`.

**Deferred to a follow-up todo:**
- Major #3 full redesign (broadcast-channel notification vs polling).
- Major #4 disconnect/no-leak test.
- Cosmetic nits (pre-existing `expect` at main.rs:312, double-clamp, 80-col).

## Acceptance Criteria

| #   | Criterion (outcome) | Verify by | Evidence |
| --- | ------------------- | --------- | -------- |
| 1   | Paginating with a `limit` above the server clamp no longer silently truncates | `cargo test -p acuity` (new next_after tests) | 3 tests pass (4f292f4) — None on short page, Some on full page, cursor round-trip terminates |
| 2   | A DB failure on `GET /events` returns `500`, not an empty `200` page | `cargo test -p acuity` (new 500 test) | `query_db_failure_returns_500` passes (c753114); pool-close forces real 500 |
| 3   | SSE drain is bounded per cycle and cannot starve keepalive under load | code review of poll loop + existing SSE tests pass | `SSE_MAX_DRAIN_PAGES` bounds the `for` loop; all 3 SSE tests pass (11f7d33) |
| 4   | SSE tests assert `text/event-stream` content type | `cargo test -p acuity` | `sse_response_has_event_stream_content_type` passes (11f7d33) |
| 5   | `acuity-api` no longer declares unused `serde_json` dep | `cargo build -p acuity-api` | builds clean without it (7de1560) |
| 6   | Full workspace builds clean (`clippy` + `fmt` + `test`) | `cargo clippy --all-targets` + `cargo test` | fmt clean, clippy -D warnings clean, all suites pass (35 acuity + others) |
