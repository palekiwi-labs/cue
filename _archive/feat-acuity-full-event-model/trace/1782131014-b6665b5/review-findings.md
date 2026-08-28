# Code Review Findings — feat/acuity-full-event-model

Reviewers: Gemini Flash, Claude Sonnet, Claude Opus (verification)
Branch diff base: b6665b5
Date: 2026-06-22

---

## Finding 1 — Scattered multi-file TypeScript output (CONFIRMED)

**Severity:** Major

Inner structs (`SessionIdle`, `AgentTurnCompleted`, `ToolCallRequested`,
`ToolCallCompleted`) derive `TS` but have no `#[ts(export_to = "types.ts")]`
annotation. Only `AcuityEvent` has it.

ts-rs 12.0.1 `export_all_into` recursively exports `T` and all dependencies,
each to its own `output_path()`. Without `export_to`, the default path is
`<TypeName>.ts`. So the inner structs land in separate files with cross-file
imports into `types.ts`. The codegen binary's success message
`"wrote {}/types.ts"` is therefore misleading.

**Fix:** Add `#[ts(export_to = "types.ts")]` to all four inner structs.

---

## Finding 2 — ts-rs emits intersection types, not flat objects (CONFIRMED)

**Severity:** Major

`AcuityEvent` uses newtype variants (`SessionIdle(SessionIdle)`, etc.) with
`#[serde(tag = "type")]`. ts-rs-macros 12.0.1 generates:

```typescript
{ "type": "session_idle" } & SessionIdle
```

This is structurally distinct from what serde actually produces at runtime
(`{"type":"session_idle","session_id":"...","project_dir":"..."}`). Downstream
Zod schema generators or JSON-schema tools may behave unexpectedly.

**Fix:** Add `#[ts(export_to = "types.ts")]` on the inner structs (same as
Finding 1). With all types in the same file the intersection form is still
type-safe in TypeScript narrowing; the alternative of inline enum variants
is more invasive. Accepted trade-off: fix Finding 1, accept intersection
form as the current output shape.

---

## Finding 3 — `sqlx::query` drops multi-statement SQL (REFUTED)

**Severity:** Labeled Critical by Sonnet — WRONG

sqlx 0.8.6 SQLite backend does NOT drop extra statements. `execute()` routes
through `execute_many` → `fetch_many` with `.try_collect()`, and the SQLite
worker iterates every semicolon-separated statement via `prepare_next` until
the tail is exhausted. All three DDL statements execute correctly.

Note: the multi-statement `query` path is `#[deprecated]` in sqlx 0.8. Using
`sqlx::raw_sql(SCHEMA_SQL)` would be more idiomatic but the existing code is
functionally correct. Low-priority style nit, not a bug.

**Action:** No fix required for correctness. `raw_sql` migration is optional.

---

## Finding 4 — Flaky sleep-based Gotify test (CONFIRMED)

**Severity:** Major

`valid_session_idle_forwards_to_gotify` uses `tokio::time::sleep(50ms)` to
wait for a fire-and-forget `tokio::spawn` before wiremock verifies on drop.
The `JoinHandle` is dropped inside `handle_event` and is not accessible to
the test. A fixed sleep is inherently racy under CI load.

**Fix:** Poll `mock_server.received_requests()` in a loop until count reaches
1 (or timeout). This removes the timing dependency without requiring a code
change to the handler.

---

## Finding 5 — `from_utf8_lossy` unnecessary (CONFIRMED)

**Severity:** Nit

In `handle_event`, `String::from_utf8_lossy(&body).into_owned()` is called
after `serde_json::from_slice(&body)` has already succeeded. JSON requires
UTF-8; a successful parse guarantees the body is valid UTF-8. The lossy
replacement path can never be taken.

**Fix:** Use `String::from_utf8(body.to_vec()).expect("serde_json already validated UTF-8")` or `std::str::from_utf8(&body).unwrap().to_owned()`.

---

## Finding 6 — `notify_gotify` log message hardcoded (CONFIRMED, currently harmless)

**Severity:** Minor

`info!("forwarded session.idle to Gotify")` is hardcoded in `notify_gotify`,
which receives `title` and `message` as parameters. There is exactly one call
site and it is behind a `SessionIdle` guard, so the string is accurate today.
Latent maintenance hazard if a future variant triggers notifications.

**Fix:** Change to `info!("forwarded event to Gotify: {}", title)` or similar.

---

## Finding 7 — `received_at: &str` has no format enforcement (CONFIRMED, mitigated)

**Severity:** Minor

`insert_event` accepts `received_at: &str` with no format validation. A
malformed timestamp would be stored silently and break `ORDER BY received_at`
queries (SQLite lexicographic ordering works only when format is consistent).

Mitigated: the sole production call site uses
`chrono::Utc::now().to_rfc3339_opts(SecondsFormat::Secs, true)`.

**Fix:** Change `insert_event` signature to accept `chrono::DateTime<chrono::Utc>`
and format inside the function. This enforces correctness at the type boundary.

---

## Finding 8 — No `received_at` assertion in DB tests (CONFIRMED)

**Severity:** Minor

`fetch_row` helper selects `event_type, session_id, turn_id, payload` — no
`received_at`. No test verifies the timestamp column is stored or correctly
formatted. This is the primary temporal ordering column.

**Fix:** Add a test that inserts an event and asserts `received_at` matches
the expected ISO-8601 `Z` format.

---

## Summary

| # | Finding | Verdict | Action |
|---|---------|---------|--------|
| 1 | Scattered TS output | CONFIRMED | Fix: add `#[ts(export_to)]` to inner structs |
| 2 | ts-rs intersection types | CONFIRMED | Accept intersection form; fix via Finding 1 |
| 3 | sqlx multi-statement drop | REFUTED | No fix needed |
| 4 | Flaky sleep test | CONFIRMED | Fix: poll wiremock requests |
| 5 | `from_utf8_lossy` nit | CONFIRMED | Fix: use `from_utf8().unwrap()` |
| 6 | Hardcoded log string | CONFIRMED | Fix: interpolate `title` |
| 7 | `received_at` type safety | CONFIRMED | Fix: accept `DateTime<Utc>` |
| 8 | Missing `received_at` test | CONFIRMED | Fix: add assertion |
