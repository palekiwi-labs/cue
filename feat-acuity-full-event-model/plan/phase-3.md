---
status: open
---
# Phase 3 — Full event model + SQLite persistence

## Foreword

This plan covers Phase 3 of the acuity roadmap. It implements:

1. A 4-event harness-agnostic discriminated union in `acuity-schema`
2. SQLite persistence in `acuity` via `sqlx`
3. A Gotify refactor: optional, presence-based, fire-and-forget

**Task:** `.cue/master/task/1781965432-d2f3251/acuity-full-event-model.md`
**Base branch:** `master` (working directly on master, no feature branch)

### Prerequisites

- `cargo test --workspace` passes (11 tests in `acuity`, green baseline)
- `acuity-schema` has one event type: `SessionIdle`
- `acuity` is stateless: `POST /events` validates header, deserialises
  `SessionIdle`, forwards to Gotify (required at startup)

### Wire format change

Body changes from a bare `SessionIdle` object to a tagged union:

```
{ "type": "session_idle",          ... }
{ "type": "agent_turn_completed",  ... }
{ "type": "tool_call_requested",   ... }
{ "type": "tool_call_completed",   ... }
```

`SCHEMA_VERSION` stays at `1`. A Phase 1 plugin will 422 until updated in
Phase 4 — this is expected and accepted.

### Key implementation notes for a fresh agent

- `AcuityEvent` enum needs `#[ts(export_to = "types.ts")]` on the enum itself
  (not only on variants) or it will not appear in the generated file.
- `serde_json::Value` requires `serde_json` dep in `acuity-schema` AND
  `ts-rs` feature `serde-json-impl`.
- `impl AcuityEvent` with `event_type()`, `session_id()`, `turn_id()` must
  return strings that exactly match the serde-generated `"type"` discriminants.
  A unit test enforces this.
- `payload` = raw request `Bytes` cast to `String` (faithful copy). Do NOT
  re-serialise the deserialized enum — that drops unknown fields.
- SQLite pool requires `SqliteConnectOptions::create_if_missing(true)`.
  Without it, `connect()` fails if the DB file does not yet exist.
- In-memory SQLite test pools must use `max_connections(1)`. With multiple
  connections each gets its own isolated `:memory:` database.
- `tokio::spawn` requires `'static` captures. Clone `http`, `gotify_url`,
  and `token` before spawning. `reqwest::Client` is internally `Arc` — cloning
  shares the connection pool.

---

## Stage A — `acuity-schema` crate (wire contract)

- [x] **A1.** `crates/acuity-schema/Cargo.toml`: add `serde_json = "1"`;
      change ts-rs entry to
      `ts-rs = { version = "12", features = ["serde-compat", "serde-json-impl"] }`.

- [x] **A2.** `crates/acuity-schema/src/lib.rs`: add the 3 new structs
      (`AgentTurnCompleted`, `ToolCallRequested`, `ToolCallCompleted`) and the
      `AcuityEvent` enum with `#[serde(tag = "type", rename_all = "snake_case")]`.
      Add `#[ts(export_to = "types.ts")]` to the `AcuityEvent` enum itself.
      Keep `SCHEMA_VERSION = 1`. Keep `SessionIdle` unchanged.

- [x] **A3.** Add `impl AcuityEvent` in `lib.rs` with three methods:
      - `event_type() -> &'static str` — returns the snake_case tag string
      - `session_id() -> &str` — returns the `session_id` field from the active variant
      - `turn_id() -> Option<&str>` — `None` for `SessionIdle`, `Some(&turn_id)` for the rest

- [x] **A4.** Add `#[cfg(test)]` mod in `acuity-schema/src/lib.rs`. For each
      of the 4 variants assert:
      - round-trip serde (serialise → deserialise → eq)
      - `event_type()` equals the `type` field produced by `serde_json::to_value`
      - `turn_id()` is `None` for `SessionIdle`, `Some(...)` for the rest

- [x] **A5.** `crates/acuity-schema/src/bin/codegen.rs`: replace
      `SessionIdle::export_all(&cfg)` with `AcuityEvent::export_all(&cfg)`.

- [x] **A6.** Run `cargo test -p acuity-schema` — green.

- [x] **A7.** Run `cargo run -p acuity-schema --bin codegen -- /tmp/acuity-ts`
      and inspect `cat /tmp/acuity-ts/types.ts`. Verify:
      - `AcuityEvent` is present as a TypeScript discriminated union
      - All 4 variant types are exported
      - `args` field is typed `any` (from `serde_json::Value`)
      - Optional fields are `... | null`

---

## Stage B — `acuity` crate dependencies

- [ ] **B1.** `crates/acuity/Cargo.toml`: add
      ```toml
      sqlx   = { version = "0.8", default-features = false,
                 features = ["runtime-tokio-rustls", "sqlite", "macros", "migrate"] }
      chrono = { version = "0.4", default-features = false,
                 features = ["clock", "std"] }
      ```

- [ ] **B2.** `cargo build -p acuity` — confirms clean compile before any
      source changes.

---

## Stage C — DB module (new), TDD

- [ ] **C1.** Create `crates/acuity/src/db.rs`. Define:
      - `SCHEMA_SQL: &str` — the inline `CREATE TABLE IF NOT EXISTS events ...`
        and two `CREATE INDEX IF NOT EXISTS` statements
      - `pub async fn connect(path: &std::path::Path) -> anyhow::Result<sqlx::SqlitePool>`
        using `SqliteConnectOptions::new().filename(path).create_if_missing(true)`,
        `create_dir_all` on the parent dir, then execute `SCHEMA_SQL`
      - `pub async fn insert_event(pool: &sqlx::SqlitePool, event: &AcuityEvent, received_at: &str, payload: &str) -> sqlx::Result<i64>`
        using the `impl AcuityEvent` accessors; returns `last_insert_rowid()`
      - `#[cfg(test)] pub async fn memory_pool() -> sqlx::SqlitePool`
        using `:memory:` with `max_connections(1)` and running `SCHEMA_SQL`

      SQLite schema (inline in `SCHEMA_SQL`):
      ```sql
      CREATE TABLE IF NOT EXISTS events (
          seq         INTEGER PRIMARY KEY AUTOINCREMENT,
          received_at TEXT    NOT NULL,
          event_type  TEXT    NOT NULL,
          session_id  TEXT    NOT NULL,
          turn_id     TEXT,
          payload     TEXT    NOT NULL
      );
      CREATE INDEX IF NOT EXISTS idx_events_session
          ON events(session_id);
      CREATE INDEX IF NOT EXISTS idx_events_turn
          ON events(turn_id) WHERE turn_id IS NOT NULL;
      ```

- [ ] **C2.** Tests in `db.rs` (`#[cfg(test)]` mod):
      - Insert one `SessionIdle` → assert `event_type = "session_idle"`,
        `turn_id IS NULL`, `session_id` correct, `payload` byte-equal
      - Insert one `AgentTurnCompleted` → assert `turn_id` populated
      - Insert one `ToolCallRequested` → assert `event_type`, `turn_id`, `payload`
      - Insert one `ToolCallCompleted` → assert `is_error` preserved in payload

- [ ] **C3.** `cargo test -p acuity db::` — green.

---

## Stage D — Config / DB path resolution

- [ ] **D1.** Add `fn resolve_db_path() -> std::path::PathBuf` in `main.rs`
      (not in `Config`): if `ACUITY_DATA_DIR` env var is set, use
      `{that}/acuity/events.db`; else `dirs::data_dir().unwrap_or_else(|| ...)/acuity/events.db`.

- [ ] **D2.** No changes to `config.rs` or its tests required.

---

## Stage E — AppState + tests-first (RED)

- [ ] **E1.** `crates/acuity/src/main.rs`: update `AppState`:
      ```rust
      struct AppState {
          config: config::Config,
          gotify_token: Option<String>,   // was String
          http: reqwest::Client,
          db: sqlx::SqlitePool,           // new
      }
      ```

- [ ] **E2.** Update `crates/acuity/src/tests.rs` **before** touching the
      handler:
      - All `AppState` constructions: add `db: db::memory_pool().await`,
        change `gotify_token` to `Some("test-token".into())`
      - Change **"Gotify error → 502"** assertion: expect `200` (persisted
        despite Gotify failure), assert the row exists in the pool
      - Add new tests:
        - `gotify_disabled_session_idle_returns_200`: `gotify_token: None`,
          POST `session_idle`, assert 200 and row exists, assert wiremock
          received 0 calls
        - `non_idle_event_does_not_notify`: POST `agent_turn_completed`,
          assert 200 and row exists, assert wiremock received 0 calls
        - `valid_event_persists_row`: POST any valid event, query pool,
          assert 1 row with correct `event_type` and `session_id`
        - `db_failure_returns_500`: simulate DB error (close pool or use a
          narrow unit test on `db::insert_event`)
      - Update malformed-body test body to an invalid `AcuityEvent`
        (missing/unknown `type` field) — still expects 422

- [ ] **E3.** `cargo test -p acuity` — expect RED (handler not yet updated).
      Confirm failures are the expected ones (handler still uses old types).

---

## Stage F — Handler rewrite (GREEN)

- [ ] **F1.** Rewrite `handle_event` in `main.rs` to the new control flow:
      1. Validate `X-Acuity-Schema == SCHEMA_VERSION` → 400 (unchanged logic)
      2. `serde_json::from_slice::<AcuityEvent>(&body)` → 422 on error
      3. `let received_at = chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true)`
      4. `let payload = String::from_utf8_lossy(&body).into_owned()` (raw faithful copy)
      5. `db::insert_event(&state.db, &event, &received_at, &payload).await`
         → on `Err`: log + `return StatusCode::INTERNAL_SERVER_ERROR`
      6. If `matches!(&event, AcuityEvent::SessionIdle(_))` AND
         `state.gotify_token.is_some()`: extract `title`/`message` from the
         `SessionIdle` variant, clone `http`/`gotify_url`/`token`, then
         `tokio::spawn(async move { notify_gotify(...).await; })`
      7. `return StatusCode::OK`

- [ ] **F2.** Extract `async fn notify_gotify(http, url, token, title, message)`
      from the old Gotify block. Failure logs an error; never returns a status.
      Keep `fn basename` unchanged.

- [ ] **F3.** `cargo test -p acuity` — GREEN.

- [ ] **F4.** `cargo clippy -p acuity -- -D warnings` — clean.

---

## Stage G — `main()` wiring

- [ ] **G1.** Remove the hard-required `ACUITY_GOTIFY_TOKEN` block. Replace with:
      ```rust
      let gotify_token = std::env::var("ACUITY_GOTIFY_TOKEN").ok();
      match &gotify_token {
          Some(_) => info!("Gotify notifications enabled"),
          None    => info!("Gotify token not set, notifications disabled"),
      }
      // Warn if token is set but URL is still the default
      if gotify_token.is_some() && cfg.gotify_url == Config::default().gotify_url {
          tracing::warn!("ACUITY_GOTIFY_TOKEN is set but gotify_url is still the default; \
                          notifications will likely fail");
      }
      ```

- [ ] **G2.** Resolve DB path, open pool, log the resolved path:
      ```rust
      let db_path = resolve_db_path();
      info!("opening database at {}", db_path.display());
      let db = db::connect(&db_path).await?;
      ```

- [ ] **G3.** Add `db` and updated `gotify_token` to `AppState`. Add `mod db;`.

- [ ] **G4.** Bump body limit: `DefaultBodyLimit::max(64 * 1024)`.

- [ ] **G5.** `cargo test --workspace` — all green.

- [ ] **G6.** `cargo clippy --workspace -- -D warnings` — clean.

---

## Stage H — Acceptance (`curl`)

Start the server with no Gotify token:
```bash
ACUITY_DATA_DIR=/tmp/acuity-phase3 cargo run -p acuity
```
Verify startup log says "Gotify token not set, notifications disabled".

- [ ] **H1.** SessionIdle → 200:
  ```bash
  curl -s -o /dev/null -w "%{http_code}\n" \
    -X POST localhost:33222/events \
    -H 'X-Acuity-Schema: 1' -H 'Content-Type: application/json' \
    -d '{"type":"session_idle","session_id":"s1","project_dir":"/home/pl/code/acme","session_title":"hack"}'
  # expect: 200
  ```

- [ ] **H2.** Inspect SQLite:
  ```bash
  sqlite3 /tmp/acuity-phase3/acuity/events.db \
    "SELECT seq, event_type, session_id, turn_id, received_at FROM events;"
  # expect: 1|session_idle|s1||2026-...Z   (turn_id empty)
  ```

- [ ] **H3.** AgentTurnCompleted (turn_id populated):
  ```bash
  curl -s -o /dev/null -w "%{http_code}\n" \
    -X POST localhost:33222/events \
    -H 'X-Acuity-Schema: 1' -H 'Content-Type: application/json' \
    -d '{"type":"agent_turn_completed","session_id":"s1","turn_id":"t1","input_tokens":120,"output_tokens":340}'
  # expect: 200 ; row has turn_id=t1
  ```

- [ ] **H4a.** ToolCallRequested with JSON args:
  ```bash
  curl -s -o /dev/null -w "%{http_code}\n" \
    -X POST localhost:33222/events \
    -H 'X-Acuity-Schema: 1' -H 'Content-Type: application/json' \
    -d '{"type":"tool_call_requested","session_id":"s1","turn_id":"t1","tool_call_id":"c1","tool_name":"read","args":{"path":"/x","limit":50}}'
  # expect: 200
  ```

- [ ] **H4b.** ToolCallCompleted with error:
  ```bash
  curl -s -o /dev/null -w "%{http_code}\n" \
    -X POST localhost:33222/events \
    -H 'X-Acuity-Schema: 1' -H 'Content-Type: application/json' \
    -d '{"type":"tool_call_completed","session_id":"s1","turn_id":"t1","tool_call_id":"c1","tool_name":"bash","is_error":true,"error_text":"command not found: fd"}'
  # expect: 200
  ```

- [ ] **H5.** Bad schema header → 400:
  ```bash
  curl -s -o /dev/null -w "%{http_code}\n" \
    -X POST localhost:33222/events \
    -H 'X-Acuity-Schema: 2' -H 'Content-Type: application/json' -d '{}'
  # expect: 400
  ```

- [ ] **H6.** Unknown event type → 422:
  ```bash
  curl -s -o /dev/null -w "%{http_code}\n" \
    -X POST localhost:33222/events \
    -H 'X-Acuity-Schema: 1' -H 'Content-Type: application/json' \
    -d '{"type":"nope","session_id":"x"}'
  # expect: 422
  ```

- [ ] **H7.** No-token server starts and persists (covered above — verify log
      line and that H1-H4 all wrote rows).

- [x] **H8.** Codegen artifact:
  ```bash
  cargo run -p acuity-schema --bin codegen -- /tmp/acuity-ts
  cat /tmp/acuity-ts/types.ts
  # expect: AcuityEvent discriminated union + all 4 variant types + args: any
  ```

---

## Done criteria

- [ ] `cargo test --workspace` green (new persistence + Gotify-disabled tests included)
- [ ] H1-H8 pass manually
- [ ] `cargo clippy --workspace -- -D warnings` clean
- [x] `types.ts` regenerates with the 4-event union
- [ ] Server starts without `ACUITY_GOTIFY_TOKEN` (logs "disabled") and persists events
