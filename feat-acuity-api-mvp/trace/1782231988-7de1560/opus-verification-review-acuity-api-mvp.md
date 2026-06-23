# Verification Review: feat/acuity-api-mvp fixes (consultant-opus)

- **Reviewer:** consultant-opus (verification pass — reviewing its own prior review)
- **Base branch:** master
- **Current branch:** feat/acuity-api-mvp
- **Original review HEAD:** a8d0fb9
- **Post-fix HEAD:** 7de1560
- **Diff reviewed:** `.cue/feat-acuity-api-mvp/tmp/review-fixes-a8d0fb9-to-HEAD.diff`
- **Original review:** `.cue/feat-acuity-api-mvp/trace/1782211100-a8d0fb9/opus-review-acuity-api-mvp.md`

## Verdict

**ADDRESSED** — all blocking findings are correctly and completely resolved;
deferrals are acceptable for merge.

## Summary

The author resolved both Critical issues and all three Major issues that were
in scope. The pagination contract is now driven by a server-computed
`next_after` cursor (no more length-heuristic data loss), DB errors surface
as `500`, the SSE drain loop is bounded, content-type is asserted, and the
single-writer assumption is documented. The crate builds clean, clippy is
clean, and all 35 tests pass including four new targeted tests. No
regressions or new panics were introduced; the deliberately deferred items
(broadcast redesign, disconnect test, cosmetic nits) are reasonable to defer.

## Finding-by-finding

**Critical #1 — pagination clamp silent data loss: RESOLVED.**
- `EventsPage` now carries `next_after: Option<i64>`
  (`crates/acuity-api/src/lib.rs:48-53`); `query_events_after` returns
  `(Vec<EventRecord>, Option<i64>)` (`crates/acuity/src/db.rs:94`).
- Cursor logic is correct in all cases (`db.rs:133-137`): full page
  (`records.len() == clamped_limit`) -> `Some(last.seq)`; short page incl.
  empty -> `None`. The decision is based on `clamped_limit`, not the client's
  requested `limit`, so the clamp can no longer leak through.
- **Exact-boundary case** ("exactly `clamped_limit` rows remain"): returns a
  full page with a cursor; the client resumes and `seq > last.seq` yields 0
  matching rows -> `None`. One extra round-trip, no data loss. The comment at
  `db.rs:130-132` documents this precisely.
- **Filters:** `records` already reflects the `session_id`/`event_type` WHERE
  predicates (`db.rs:103-110`), so `next_after` correctly means "more
  matching rows may exist." Verified via `query_session_id_filter`/
  `query_event_type_filter` and the cursor tests.
- Contract doc (`lib.rs:37-46`) is now accurate and explicitly warns against
  the old `events.len() == limit` heuristic.
- Tests `query_next_after_none_on_short_page`,
  `query_next_after_some_on_full_page`,
  `query_next_after_resumes_and_terminates` all pass.
- *Minor residual:* the precise "full page where rows remaining ==
  clamped_limit, then resume yields a literally empty page + None" case is
  covered by logic/comment but not by a dedicated test asserting
  `events.len() == 0`. Non-blocking.

**Critical #2 — swallowed DB errors: RESOLVED.**
- `query_events` now returns `Result<Json<EventsPage>, StatusCode>` and maps
  DB errors to `StatusCode::INTERNAL_SERVER_ERROR`
  (`crates/acuity/src/main.rs:89, 102-106`). The error path is reachable and
  is exercised by `query_db_failure_returns_500` (`tests.rs`), which closes
  the pool and asserts `500`. No remaining masking.

**Major #3 — unbounded SSE drain: RESOLVED (within deferral scope).**
- Inner `loop` replaced with `for _ in 0..SSE_MAX_DRAIN_PAGES`
  (`main.rs:150`), `SSE_MAX_DRAIN_PAGES = 10` (`main.rs:130`),
  `SSE_PAGE_SIZE = 50` (`main.rs:125`) — bound is 500 rows/cycle, sane.
- Both exits — `is_last_page` break (`main.rs:178-180`) and DB-error break
  (`main.rs:184`) — break the `for` and fall through to the 500ms sleep
  (`main.rs:188`), then the outer `loop` re-polls from the last `seq`. No
  starvation; keepalive can now intervene. The magic-number `50` nit is also
  fixed via the constants.
- Full broadcast-channel redesign deferred — acceptable; the bound makes the
  polling approach safe for MVP.

**Major #4 — content-type / framing test: RESOLVED (disconnect deferred).**
- `sse_response_has_event_stream_content_type` (`tests.rs:329-351`) asserts
  `content-type` starts with `text/event-stream` — correct assertion. The
  shared `first_data_frame` helper (`tests.rs:290-312`) is correct and is now
  reused by `sse_first_data_line` and `sse_last_event_id_resumes_from_cursor`,
  removing the duplicated boilerplate nit. Disconnect/no-leak test deferred —
  low risk, acceptable.

**Major #5 — seq/commit-order race: RESOLVED (documentation).**
- A clear comment at `main.rs:138-142` documents the single-writer assumption
  (assignment order == commit order under the ingest path) and explicitly
  states concurrent writers are not supported. Adequate for the "document the
  assumption" recommendation.

**Nit — unused `serde_json` dep: RESOLVED.**
- Removed from `crates/acuity-api/Cargo.toml`; the only remaining reference
  is the doc comment at `lib.rs:20` instructing the consumer to use
  `serde_json::from_str`, which is sane (curator brings its own `serde_json`).
  `Cargo.lock` synced. `acuity-api` and the workspace build clean.

## New issues / regressions

None.
- The `query_events_after` tuple-return signature change is correctly
  propagated to both callers: the query handler (`main.rs:102`) and the SSE
  handler (`main.rs:160-161`, using `next_after.is_none()` for `is_last_page`
  — semantically equivalent to the old `len() < 50`). No other callers exist.
- No new `unwrap`/`panic` in non-test paths; the pre-existing `expect` at
  `handle_event` is unchanged.
- `#[serde(default)]` on `next_after` (`lib.rs:52`) is forward-compatible and
  harmless; curator does not consume `EventsPage` yet, so no backward-compat
  concern.
- The `79a5aa8` rustfmt commit touches unrelated cue/cuelib files but is
  formatting-only; full workspace builds and clippy is clean.

## Disagreements with deferrals

The deferrals are acceptable for merge.
- **Major #3 broadcast redesign:** acceptable — the iteration bound removes
  the starvation hazard, so the latency/efficiency improvement is a genuine
  follow-up, not a blocker.
- **Major #4 disconnect/no-leak test:** acceptable but the *weakest*
  deferral. The spawned poll loop runs `loop { ... }` forever and relies on
  axum dropping the stream on client disconnect to terminate it; that
  behavior is currently unverified. Risk is low (standard axum SSE
  semantics), but recommended to prioritize this test early in follow-up
  since a leak here would be silent and only manifest under production load.
- **Cosmetic nits** (pre-existing `expect`, redundant double-clamp at
  `main.rs:91`/`db.rs:95`, 80-col test lines): trivially deferrable.
