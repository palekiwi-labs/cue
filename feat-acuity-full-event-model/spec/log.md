# Project Log

## [65dd2bc-dirty] Phase 3 design session complete — artifacts created

Extended three-round design session (with two Opus consultations) to define the full Phase 3 scope for acuity: 4-event model, SQLite persistence, and Gotify refactor. Task and executive plan artifacts created.

- **Found:** AcuityEvent enum needs its own #[ts(export_to = types.ts)] or it does not appear in the generated file
- **Found:** acuity-schema requires serde_json dep and ts-rs serde-json-impl feature for serde_json::Value args field
- **Found:** impl AcuityEvent accessor methods (event_type, session_id, turn_id) must exactly match serde rename_all = snake_case tags — unit test enforces this
- **Found:** SqliteConnectOptions::create_if_missing(true) required — pool connect fails on missing DB file without it
- **Found:** In-memory SQLite test pools need max_connections(1) — multiple connections each get an isolated :memory: database
- **Found:** tokio::spawn requires static captures; reqwest::Client is internally Arc so cloning shares the connection pool
- **Found:** Gotify 502 error in Phase 1 handler could cause plugin retries leading to double-persist — eliminated by persist-first design
- **Found:** Default body limit of 16KiB should be bumped to 64KiB to accommodate tool args and error_text fields
- **Decided:** 4-event harness-agnostic discriminated union: SessionIdle (existing), AgentTurnCompleted, ToolCallRequested, ToolCallCompleted
- **Decided:** AgentTurnStarted explicitly dropped — no curator view reads a start row; liveness belongs to the Phase 5 SSE stream
- **Decided:** SCHEMA_VERSION stays at 1 — pre-alpha, no deployed users, breaking wire change accepted without bump
- **Decided:** Gotify refactored to presence-based opt-in: ACUITY_GOTIFY_TOKEN optional, persist-first, always-200, fire-and-forget via tokio::spawn
- **Decided:** payload column = raw request bytes (faithful copy), no deny_unknown_fields
- **Decided:** received_at = server-side ISO-8601 UTC, seconds precision, Z suffix. Client timestamps go inside payload only
- **Decided:** seq INTEGER PRIMARY KEY AUTOINCREMENT is the future SSE resume cursor for Phase 5 Last-Event-ID
- **Decided:** Session-to-project in activity feed: feed row only shown after first SessionIdle for that session_id. No denormalization, no curator state
- **Decided:** Crashing sessions (no SessionIdle) are invisible to the activity feed but fully visible in the diagnostics view
- **Decided:** Idempotency/dedup deferred to Phase 7 hardening
- **Decided:** sqlx (async, runtime-tokio-rustls, sqlite) chosen over rusqlite
- **Decided:** chrono added for received_at generation — was missing from initial Cargo planning
- **Decided:** turn_id added to ToolCallCompleted (Opus recommendation) — links completions to turns for diagnostics grouping
- **Open:** Token tracking (input_tokens, output_tokens on AgentTurnCompleted) depends on Phase 4 plugin research into AgentMessage structures in pi and opencode — fields are Option<u32> in Phase 3 schema, populated in Phase 4

## [cc4cb0f-dirty] Stage A complete — acuity-schema 4-event union

Implemented all of Stage A from phase-3.md. Commit: cc4cb0f.

- **Found:** AcuityEvent needs #[ts(export_to = 'types.ts')] on the enum itself — confirmed working
- **Found:** ts-rs exports each variant struct to its own .ts file and imports them into types.ts
- **Found:** serde_json::Value in ToolCallRequested.args generates a JsonValue import from a serde_json/ subdirectory, not inline 'any' — still TS-compatible
- **Found:** All 13 unit tests pass: round-trip serde, event_type() discriminant matching, turn_id() accessor, session_id() accessor
- **Decided:** Kept SCHEMA_VERSION = 1 as per plan — no bump needed
- **Decided:** Added Clone + PartialEq derives to all structs to support test equality assertions

## [5dcfb97] Stage A review response implemented — 18 tests green

Addressed all accepted findings from the Stage A code review. Commit: 5dcfb97.

- **Found:** unknown_fields_are_ignored_on_deserialization test confirms forward-compat by design — passes cleanly
- **Found:** Raw-wire deserialization tests pass: serde internally-tagged enum + serde_json::Value works correctly for all 4 variants from literal JSON strings
- **Decided:** ToolCallCompleted output field omitted by design — raw payload is the retrieval path; doc comment added
- **Decided:** serde_json direct dep accepted and documented at crate level
- **Decided:** deny_unknown_fields intentionally absent — documented on AcuityEvent
- **Decided:** n3 (fixture IDs) and n4 (split session_id test) deferred as low value

## [b6665b5] Phase 3 Stages B-G complete — SQLite persistence + optional Gotify

Commit b6665b5. Implemented all of Stages B through G from phase-3.md in a single session. All 20 acuity tests pass; full workspace (138 tests) green; clippy -D warnings clean.

- **Found:** sqlx Row trait must be imported as `use sqlx::Row as _` when used only in #[cfg(test)] blocks — unused import in production path causes clippy -D warnings failure
- **Found:** tokio::spawn fire-and-forget Gotify notification races with wiremock server shutdown in tests — fixed with a 50ms sleep before mock verification drop
- **Found:** Clippy collapsible_if lint fires on nested `if let ... { if let ... { } }` — collapsed to `if let ... && let ... { }` using Rust 2024 let-chains
- **Found:** LSP showed persistent stale diagnostics from old tests.rs — cargo test and clippy are the ground truth; LSP errors were phantom
- **Decided:** Row trait import scoped to #[cfg(test)] mod to avoid unused-import warning in production build
- **Decided:** 50ms sleep in valid_session_idle_forwards_to_gotify test is acceptable — wiremock has no built-in await-spawn API and the window is deterministic
- **Decided:** Collapsed nested if-let per clippy suggestion using Rust 2024 let-chains syntax (edition 2024 in Cargo.toml)
- **Open:** Stage H manual curl acceptance tests remain — server start, H1-H7 curl checks against live process with ACUITY_DATA_DIR=/tmp/acuity-phase3

## [241ed82-dirty] Fix TS codegen: add #[ts(export_to)] to inner event structs

Commit 241ed82. Added #[ts(export_to = \"types.ts\"] to SessionIdle, AgentTurnCompleted, ToolCallRequested, ToolCallCompleted. All four structs now inline into types.ts instead of scattering to separate files. serde_json/JsonValue.ts side-file remains unavoidable.

- **Found:** ts-rs 12 export_all() writes each unannotated dependency type to its own .ts file by default
- **Found:** Adding #[ts(export_to = types.ts)] on inner structs consolidates all type definitions into a single file
- **Found:** serde_json::Value cannot be redirected to types.ts without a newtype wrapper — accepted
- **Decided:** Accept the serde_json/JsonValue.ts side-file as unavoidable
- **Decided:** Do not introduce a newtype wrapper for Value at this stage

## [352b80d-dirty] Type-safe received_at, fix from_utf8_lossy, add received_at DB test

Commit 352b80d. insert_event now accepts DateTime&lt;Utc&gt; instead of &str. from_utf8_lossy replaced with from_utf8().expect() since JSON parse already guarantees UTF-8. fetch_row in tests now returns received_at; insert_session_idle asserts ISO-8601 Z format.

- **Decided:** Accept DateTime&lt;Utc&gt; at insert_event boundary — formatting is an implementation detail of the function
- **Decided:** Use from_utf8().expect() not from_utf8_lossy — lossy path is unreachable after successful JSON parse

## [fb24715-dirty] Review findings implemented — 4 commits, all 138 tests green

Applied all confirmed review findings from the Flash/Sonnet/Opus review cycle. Four commits: 241ed82 (ts export_to), 352b80d (typed received_at + from_utf8 fix + received_at test), e020af3 (notify_gotify log), fb24715 (flaky test poll loop). Workspace at 138 tests green, clippy -D warnings clean.

- **Found:** Finding 3 (sqlx multi-statement drop) was REFUTED by Opus — sqlx 0.8 SQLite driver iterates all statements via prepare_next tail loop
- **Found:** Poll loop on mock_server.received_requests() is a clean no-handler-change fix for fire-and-forget test synchronization
- **Decided:** Do not migrate to sqlx::raw_sql — functionally correct as-is, deprecated path is a style nit only
- **Decided:** Accept serde_json/JsonValue.ts side-file — cannot redirect without newtype wrapper

