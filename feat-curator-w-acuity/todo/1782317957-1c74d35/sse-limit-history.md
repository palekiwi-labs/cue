---
status: open
priority: normal
---
# Slice 2 — Bounded SSE initial history via `?limit_history=N`

## Background

On first connect curator sends `Last-Event-ID: 0`, causing the acuity server
to replay the entire event database (unbounded). The client ring buffer caps
at `EVENT_CAP = 2000`, so events beyond the newest 2000 are evicted regardless
— all that replay work is wasted.

The user confirmed: only recent history is needed in the Activity Feed and
Diagnostics views. Old sessions with incomplete summaries (no `session_idle`
in the window) are an acceptable trade-off.

This was identified during code review (C1) and confirmed by an Opus
consultation (`trace/1782317957-1c74d35/phase-6-code-review.md`). The
review-fixes plan (`plan/1782317957-1c74d35/review-fixes.md`) ships `try_send`
as a liveness guard first. This Slice 2 is the real fix for the root cause.

## Why not pure client-side?

The existing `GET /events` REST API is forward-only (`after + limit` ascending
by seq). There is no `ORDER BY seq DESC` and no `max_seq` endpoint. Getting
the *last* N events client-side would require paginating forward from 0 — which
defeats the purpose. A minimal server affordance is needed.

## Accepted limitation

Session summaries for sessions whose `session_idle` event falls outside the
`limit_history` window will show:
- `project_dir = ""` and `session_title = None`
- Undercounted `input_tokens`, `output_tokens`, `error_count`

Pick `N = 3000` (EVENT_CAP + 1000 headroom) so active sessions are almost
always fully captured. Document this as intentional in code comments.

## Implementation

### Server — `crates/acuity/src/db.rs`

Add a `max_seq` helper (one query):

```rust
pub async fn max_seq(pool: &SqlitePool) -> sqlx::Result<i64> {
    let row = sqlx::query("SELECT COALESCE(MAX(seq), 0) FROM events")
        .fetch_one(pool)
        .await?;
    Ok(row.get::<i64, _>(0))
}
```

### Server — `crates/acuity/src/main.rs`

Add a query parameter struct and update `sse_handler`:

```rust
#[derive(serde::Deserialize, Default)]
struct SseQuery {
    limit_history: Option<i64>,
}

async fn sse_handler(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<SseQuery>,
) -> Sse<...> {
    let header_cursor = parse_last_event_id(&headers);

    // limit_history only applies on first connect (header cursor == 0).
    // On reconnect the client sends its advanced cursor — use that directly.
    let cursor = if header_cursor == 0 {
        if let Some(limit) = query.limit_history {
            let max = db::max_seq(&state.db).await.unwrap_or(0);
            (max - limit).max(0)
        } else {
            0
        }
    } else {
        header_cursor
    };
    // ... rest of handler unchanged, uses `cursor` instead of direct
    //     parse_last_event_id result
```

### Client — `crates/curator/src/sse.rs`

In `connect_and_stream`, append `?limit_history=3000` only on the initial
connect (`cursor == 0`):

```rust
// Constant — one public definition, referenced from both sse.rs and any tests.
const HISTORY_LIMIT: i64 = 3_000;

async fn connect_and_stream(
    client: &reqwest::Client,
    url: &str,
    cursor: i64,
    tx: &SyncSender<Msg>,
) -> Result<i64> {
    // On first connect, request only the most recent HISTORY_LIMIT events.
    // On reconnect the advanced cursor already scopes the replay correctly.
    let stream_url = if cursor == 0 {
        format!("{url}/events/stream?limit_history={HISTORY_LIMIT}")
    } else {
        format!("{url}/events/stream")
    };
    let response = client
        .get(stream_url)
        .header("Last-Event-ID", cursor.to_string())
        .send()
        .await?;
    // ... rest unchanged
```

No changes to `app.rs`, `msg.rs`, `ui.rs`, or `main.rs` — events still
arrive via the same `Msg::Sse` → `push_event` path.

## Tests to add

- `db.rs`: `max_seq_returns_zero_on_empty_db`, `max_seq_returns_highest_seq`
- `main.rs` integration test: seed DB with M > HISTORY_LIMIT rows, connect
  with `?limit_history=N`, assert stream starts at approximately `M - N`
  (first event seq > M - N - 1)
- `sse.rs`: assert the URL built on `cursor == 0` includes
  `limit_history=3000` and the URL on `cursor > 0` does not

## Effect on C1

Once this slice ships, cold-start backlog no longer flows through the shared
channel. The `try_send` added in the review-fixes plan becomes belt-and-
suspenders — protecting liveness under warm reconnect bursts and live
overload, not initial replay. No removal needed.

## Cue references

- Opus consultation: `trace/1782317957-1c74d35/phase-6-code-review.md`
- Review fixes (ship first): `plan/1782317957-1c74d35/review-fixes.md`
