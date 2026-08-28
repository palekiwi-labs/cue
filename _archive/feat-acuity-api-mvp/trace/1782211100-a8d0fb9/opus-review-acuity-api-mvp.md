# Code Review: feat/acuity-api-mvp (consultant-opus)

- **Reviewer:** consultant-opus
- **Base branch:** master
- **Current branch:** feat/acuity-api-mvp
- **Merge base:** 8330d03cb93269dd0e527dfb68d6a6e84bf810fe
- **Scope:** 468 insertions across 6 files, 4 commits (event query +
  SSE streaming API)

## Commits reviewed

1. `03f7817` feat(acuity-api): define EventRecord, EventsPage, re-export
   AcuityEvent
2. `988f9da` feat(acuity): add query_events_after to db layer
3. `ec33f38` feat(acuity): add GET /events paginated query endpoint
4. `a8d0fb9` feat(acuity): add GET /events/stream SSE endpoint

## Files changed

- crates/acuity-api/Cargo.toml (+2)
- crates/acuity-api/src/lib.rs (+45)
- crates/acuity/Cargo.toml (+3)
- crates/acuity/src/db.rs (+57)
- crates/acuity/src/main.rs (+118)
- crates/acuity/src/tests.rs (+250)

---

## Verdict

**REQUEST CHANGES**

## Summary

A well-structured, well-documented MVP with strong test coverage and clean
separation between the dependency-light `acuity-api` types crate and the
server. The SQL layer is correctly parameterized and the seq-cursor design is
sound in principle. However, there is one genuine pagination-contract bug
(server-side limit clamping silently breaks the documented "is this the last
page?" heuristic), the query endpoint swallows DB errors behind a `200 OK`
empty page, and the SSE drain loop has an unbounded catch-up path with no
disconnect handling. These should be addressed before merge.

---

## Critical issues

### 1. Limit clamping breaks the documented pagination contract -> silent data loss

- **Contract:** `crates/acuity-api/src/lib.rs:37-39` — "if `events.len() ==
  limit` (the requested page size), there may be more rows... If
  `events.len() < limit`, this is the final page."
- **Clamp:** `crates/acuity/src/db.rs:91` — `let clamped_limit =
  limit.clamp(1, 500);`
- **Problem:** A client requests `limit=1000`, the server clamps to `500` and
  returns 500 rows. The client applies the documented heuristic:
  `events.len() (500) < limit (1000)` -> concludes "final page" -> stops
  paginating and silently drops every row past 500. The client cannot observe
  the server's clamp because the effective limit is never returned.
- **Why it matters:** Silent, undetectable data loss for any consumer
  (`curator`) that follows the documented contract with a limit above 500.
  Cursor-based pagination should not depend on the client knowing the server's
  internal page size.
- **Suggested fix:** Add an explicit pagination signal to `EventsPage` rather
  than relying on length comparison. Options: (a) `next_after: Option<i64>`
  set to the last `seq` when a full (clamped) page was returned and `None`
  otherwise; or (b) return the effective `limit` the server applied. Have the
  server compute "more rows exist" by fetching `clamped_limit` and comparing
  against `clamped_limit`, then expose that decision to the client. Update the
  lib.rs contract doc accordingly.

### 2. `GET /events` masks database errors as `200 OK` with an empty page

- **Location:** `crates/acuity/src/main.rs:103-106`
  ```rust
  Err(err) => {
      error!("query_events failed: {}", err);
      Json(acuity_api::EventsPage { events: vec![] })
  }
  ```
- **Problem:** A DB failure is indistinguishable from "no matching events" —
  both yield `200 OK {"events":[]}`. A client paginating through history will
  interpret a transient DB error as "final page reached" (per the contract
  above) and terminate cleanly while missing data.
- **Why it matters:** Errors that should surface as `5xx` are hidden;
  consumers cannot retry because they never learn the request failed. Combined
  with issue #1, this makes data loss undetectable.
- **Suggested fix:** Return `Result<Json<EventsPage>, StatusCode>` (or a typed
  error response) and map DB errors to `500 INTERNAL_SERVER_ERROR`. The
  handler signature should propagate the error rather than coercing it to an
  empty success.

---

## Major issues

### 3. SSE inner drain loop is unbounded and starves keepalive / cancellation under load

- **Location:** `crates/acuity/src/main.rs:134-160`. The inner `loop` only
  `break`s when `records.len() < 50` (`is_last_page`) or on DB error.
- **Problem:** Under a sustained write rate of >=50 events per poll cycle,
  `is_last_page` is never true, so the inner loop never breaks, never reaches
  the `sleep` at line 161, and never lets the 15s keepalive logic intervene.
  There are `.await` points so the task yields to the runtime, but a single
  slow consumer will accumulate unbounded backlog with no flow control, and
  the loop can monopolize progress.
- **Why it matters:** On a busy server an SSE client effectively gets a tight
  catch-up loop with no upper bound and no opportunity for graceful behavior;
  pathological inputs degrade fairness.
- **Suggested fix:** Cap the number of drain iterations per cycle (e.g., drain
  at most N pages, then sleep regardless), or yield/sleep a short interval
  between full pages. Consider an event-notification mechanism (broadcast
  channel on insert) instead of fixed 500ms polling for lower latency and
  bounded work.

### 4. No disconnect/cancellation test and no verification of `text/event-stream` content type

- **Location:** `crates/acuity/src/tests.rs:292-388`. The SSE tests assert
  delivery of the first/second data frame but never assert the response
  `Content-Type: text/event-stream` header, and never exercise client
  disconnect. The poll loop at `main.rs:130-163` runs forever; correctness
  under client drop relies entirely on axum dropping the stream.
- **Why it matters:** SSE framing (`Content-Type`, `id:`/`data:` lines) and
  disconnect cleanup are the core contract of the endpoint and are currently
  unverified. A regression that breaks the content type or leaks the polling
  task would pass CI.
- **Suggested fix:** Assert `response.headers()["content-type"]` starts with
  `text/event-stream`. Add a test that drops the response body mid-stream and
  confirms no panic/hang (and ideally that the spawned poll loop terminates).

### 5. Polling-based stream cannot observe a `seq` gap / out-of-order commit

- **Location:** `crates/acuity/src/main.rs:135,139`. The stream advances
  `seq = record.seq` and queries `seq > cursor`. SQLite `AUTOINCREMENT`
  (db.rs:14) guarantees monotonic assignment, but `seq` is assigned at INSERT;
  with concurrent inserts the commit order can momentarily lag assignment
  order. A poll that reads while a lower-`seq` row is still uncommitted will
  advance past it and never re-read it.
- **Why it matters:** Classic "missed event due to gap in monotonic id" race
  in poll-based tailing. With the current single-writer ingest path it is
  unlikely, but it is a latent correctness hazard worth noting.
- **Suggested fix:** Acceptable for MVP given low concurrency, but document
  the assumption (single-writer / commit-order == seq-order) near the poll
  loop, or add a small safety lag. At minimum capture this as a known
  limitation.

---

## Minor issues / nits

- **Unused dependency:** `crates/acuity-api/Cargo.toml:8` adds `serde_json =
  "1"` but `acuity-api/src/lib.rs` never references it (only mentioned in a
  doc comment at lib.rs:20). This pulls an unused dep into `curator`'s graph —
  the opposite of the "dependency-light" goal stated at lib.rs:4-5. Remove it
  unless a follow-up commit uses it.
- **`unwrap`/`expect` in non-test path:** `crates/acuity/src/main.rs:312` —
  `String::from_utf8(...).expect("serde_json validated UTF-8")`. The reasoning
  is sound (from_slice already validated UTF-8), but this predates the branch;
  the cheaper, panic-free `String::from_utf8_lossy` or restructuring to keep
  the validated `&str` would be more robust. Pre-existing — flagging only for
  awareness.
- **`after` clamped twice:** `main.rs:91` clamps `after` to `>= 0`, and
  `db.rs:91` independently clamps `limit`. The `after` normalization in the
  handler is fine, but note `seq > after` with a negative `after` is already
  harmless in SQL; the clamp is belt-and-suspenders. Not a problem, just
  redundant with the doc claim.
- **`EventsQuery.after` uses `#[serde(default)]` (=0)** at `main.rs:73-74`
  while `limit` has an explicit `default_limit`. Consistent and fine, but
  consider rejecting malformed `after`/`limit` query strings with `400` rather
  than letting axum's `Query` extractor reject them with a default 400 that has
  no helpful body — current behavior is acceptable but undocumented.
- **Magic number `50`** appears twice in the SSE drain (`main.rs:135,137`).
  Extract a `const SSE_PAGE_SIZE: i64 = 50;` so the page size and the `< 50`
  short-page check cannot drift apart.
- **Test helper formatting:** `crates/acuity/src/tests.rs:70` and the new
  helpers exceed the project's 80-col guideline. Several lines (e.g.,
  tests.rs:70, 338, 375) run long. Non-blocking.
- **Duplicated SSE frame-reading boilerplate:** `tests.rs:303-324` and
  `tests.rs:364-387` repeat the same frame/`data:` extraction loop. Could be
  factored into a shared helper to reduce drift.

---

## Strengths

- **Excellent SQL safety:** `db.rs:84-130` uses `QueryBuilder` with
  `push_bind` for every dynamic value (`after`, `session_id`, `event_type`,
  `limit`) — fully parameterized, no injection surface, and the optional
  filters compose cleanly.
- **Clean crate boundary and documentation:** `acuity-api` is intentionally
  kept light with a clear, well-written rationale (lib.rs:1-9) for the
  `AcuityEvent` re-export, and the `EventRecord`/payload "raw wire bytes, not
  re-serialized" invariant is consistently documented across lib.rs, db.rs,
  and main.rs.
- **Solid query-endpoint test coverage:** the `GET /events` tests cover empty
  DB, cursor filtering, both equality filters, limit enforcement, the
  500-clamp, and exact field mapping (tests.rs:394-511) — meaningful
  assertions, not just status checks.

---

## Note on second reviewer

`diff-reviewer-gemini-3.5-flash` was also dispatched but returned an empty
result body (task reported complete but emitted no review text). Not captured
here.
