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

