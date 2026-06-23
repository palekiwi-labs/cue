---
status: open
---
# Phase 5 — acuity-api read model: query API + SSE

Branch: `feat/acuity-api-mvp`
Implements: `task/1781965432-d2f3251/acuity-read-model.md`

## Problem & Scope

Phase 5 adds the **outbound surface** to `acuity`: a historical query endpoint
and a real-time SSE stream. These are the endpoints `curator` will consume in
Phase 6. Both must be validated independently with `curl` before Phase 6
begins.

The `acuity-api` crate (currently a stub with a single comment) is the shared
type library. `acuity` depends on it for response serialization; `curator` will
depend on it for deserialization.

## Architecture Decisions

### Type separation

`acuity-api` defines **response types only** — no sqlx, no HTTP concerns.
`acuity` maps from sqlx rows to `acuity_api::EventRecord` manually in `db.rs`.
This keeps `acuity-api` dependency-light (only `serde`), making it safe for
`curator` to depend on without pulling in the server stack.

Query param structs (`EventsQuery`) are HTTP-level concerns and live in
`acuity` as axum extractors.

### SSE implementation: poll-based for the MVP

Two options were considered:

| Approach | Latency | Complexity | Race-free |
| -------- | ------- | ---------- | --------- |
| Broadcast channel (push) | ~0 ms | High (AppState changes, dedup) | No (window) |
| SQLite poll (500 ms) | ~500 ms | Low | Yes |

**Decision: poll-based for Phase 5.** 500 ms latency is acceptable for a
dev-facing TUI (Phase 6). The broadcast channel is a Phase 7 optimization.
Polling avoids the subscribe-then-replay race condition entirely and keeps
`AppState` unchanged.

### Pagination cursor: `seq`

The `seq INTEGER PRIMARY KEY AUTOINCREMENT` column was designed as the SSE
resume cursor in Phase 3. The query endpoint exposes it as an `after` query
param; SSE clients send it in the `Last-Event-ID` header. Both paths use the
same `db::query_events_after` function.

## New Endpoints

| Method | Path | Purpose |
| ------ | ---- | ------- |
| `GET` | `/events` | Paginated historical query |
| `GET` | `/events/stream` | Real-time SSE stream |

Existing `POST /events` is unchanged.

### `GET /events` query params

| Param | Type | Default | Notes |
| ----- | ---- | ------- | ----- |
| `after` | `i64` | 0 | Return only rows with `seq > after` |
| `limit` | `i64` | 100 | Max rows per page (capped at 500 server-side) |
| `session_id` | `String` | — | Optional filter |
| `event_type` | `String` | — | Optional filter (`session_idle` etc.) |

Response: `application/json`, body is `EventsPage`.

### `GET /events/stream`

Standard SSE. Clients set `Last-Event-ID` on reconnect.
Each SSE event has `id: <seq>` and `data: <EventRecord as JSON>`.
Keep-alive pings every 15 s.

## Type Definitions

### `acuity-api/src/lib.rs`

```rust
pub struct EventRecord {
    pub seq:         i64,
    pub received_at: String,       // ISO-8601 UTC ("2026-06-22T10:00:00Z")
    pub event_type:  String,       // "session_idle" | ...
    pub session_id:  String,
    pub turn_id:     Option<String>,
    pub payload:     String,       // raw JSON wire bytes
}

pub struct EventsPage {
    pub events: Vec<EventRecord>,
}
```

Both types get `Serialize + Deserialize`.

`acuity-api` also re-exports `acuity_schema::AcuityEvent` so that `curator`
has a single import path for parsing the `payload` field in Phase 6, rather
than depending on both `acuity-api` and `acuity-schema` separately.

## Correctness Constraints (from Opus review)

- **`limit` clamping**: clamp to `1..=500` server-side before passing to SQL.
  Never pass a raw user-supplied integer into `LIMIT`.
- **`after` normalisation**: negative values are harmless (returns everything)
  but normalise to 0.
- **SSE poll loop — drain-first**: the inner loop must drain the page fully
  (keep querying until the returned slice is shorter than the batch size)
  before sleeping. A fixed `limit: 50` per tick creates lag during bursts.
- **SSE error handling**: DB errors in the poll loop must be logged, not
  swallowed silently via `.unwrap_or_default()`.
- **`Last-Event-ID` parsing**: non-numeric or absent header falls back to 0
  (mirrors the defensive schema-header parsing in `main.rs:165-179`).
- **`KeepAlive` interval**: explicit `Duration::from_secs(15)` to keep the
  code and spec in sync.

## Implementation Plan

1. Define `EventRecord`, `EventsPage`, and re-export `AcuityEvent`
   in `acuity-api/src/lib.rs`; add `acuity-schema` dep.
2. Add `db::query_events_after` to `acuity/src/db.rs` (QueryBuilder,
   optional filters, capped limit).
3. Wire `acuity-api` + `async-stream` deps into `acuity/Cargo.toml`.
4. Add `GET /events` handler (`EventsQuery` extractor, `Json<EventsPage>`).
5. Add `GET /events/stream` handler — drain-first poll loop with
   `async_stream::stream!`, `Sse<impl Stream>`, explicit 15 s keep-alive.
6. Register both routes in `make_app`.
7. Write unit tests:
   - Query endpoint: insert events, query with various params (after, limit,
     session_id, event_type filters), assert response shape.
   - SSE smoke test: oneshot request, read first `data:` line, assert valid
     `EventRecord` with correct `id:`.
8. SSE live acceptance via `curl --no-buffer -H "Accept: text/event-stream"`.

## File Changeset

| File | Change |
| ---- | ------ |
| `crates/acuity-api/src/lib.rs` | `EventRecord`, `EventsPage`, re-export `AcuityEvent` |
| `crates/acuity-api/Cargo.toml` | add `acuity-schema`, `serde_json` |
| `crates/acuity/Cargo.toml` | add `acuity-api`, `async-stream` |
| `crates/acuity/src/db.rs` | add `query_events_after` |
| `crates/acuity/src/main.rs` | add handlers, extend `make_app` router |
| `crates/acuity/src/tests.rs` | query + SSE smoke tests |

No Nix/NixOS changes needed for this phase (`acuity` flake derivation builds
the full crate; new routes are compiled in automatically).

## Out of Scope

- `curator` integration (Phase 6)
- Broadcast channel / sub-500 ms SSE latency (Phase 7)
- Session aggregate endpoint (`/sessions`) — deferred; Phase 6 may not need it
- Auth on read endpoints (Phase 7)
