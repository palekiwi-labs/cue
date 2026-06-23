---
title: "acuity: full event model + SQLite persistence"
status: complete
priority: normal
branch: ""
---
# acuity: full event model + SQLite persistence

Extend `acuity-schema` with a 4-event harness-agnostic discriminated union
(`SessionIdle`, `AgentTurnCompleted`, `ToolCallRequested`, `ToolCallCompleted`),
add SQLite persistence via `sqlx` to `acuity`'s POST ingest handler, and refactor
Gotify notification to be fully optional and fire-and-forget.

Builds on the stateless MVP ingest path from Phase 1 — the POST endpoint and
header validation are already proven; this phase widens the schema, adds
durable storage, and decouples notification from ingest.

## Design decisions recorded

- 4-event model (not 3): `AgentTurnCompleted` added for activity-feed sub-rows
  and turn-level token tracking. `AgentTurnStarted` explicitly dropped — no
  curator view reads a start row; liveness belongs to the Phase 5 SSE stream.
- Harness-agnostic vocabulary. opencode/pi differences normalised in TypeScript
  (cue-plugins), not in the Rust schema.
- `SCHEMA_VERSION` stays at `1` — pre-alpha, no deployed users.
- `payload` column = raw request bytes (faithful copy). No `deny_unknown_fields`.
- `received_at` = server-side ISO-8601 UTC (`...Z`, seconds precision). Client
  timestamps, if needed, go inside `payload` only.
- `seq` INTEGER PRIMARY KEY AUTOINCREMENT is the future SSE resume cursor.
- Gotify: presence-based opt-in (`ACUITY_GOTIFY_TOKEN` optional). Persist-first,
  always-200, fire-and-forget notification via `tokio::spawn`. In-flight
  notifications dropped on crash — accepted behaviour.
- Idempotency/dedup deferred to Phase 7 hardening.

## Source

- spec: `.cue/master/spec/acuity/index.md`
- roadmap: `.cue/master/plan/index.md` (Phase 3)
- design session: `.cue/master/plan/1782118406-65dd2bc/phase-3.md`

## Acceptance Criteria

| #  | Criterion                                                                          | Verify by                                               | Evidence                                           |
| -- | ---------------------------------------------------------------------------------- | ------------------------------------------------------- | -------------------------------------------------- |
| 1  | `curl` POSTs of all four event types return 200                                    | curl H1-H4 in the executive plan                        | attested 2026-06-23 (H1-H4 curl; PR #24 merged)    |
| 2  | Events land in SQLite with correct `seq`, `event_type`, `session_id`, `received_at` | `sqlite3` inspect after POSTs (H2)                     | attested 2026-06-23 (H2 sqlite3 inspect)           |
| 3  | `turn_id` is NULL for `session_idle`, populated for the other three types          | `sqlite3` inspect (H3)                                  | attested 2026-06-23 (H3 sqlite3 inspect)           |
| 4  | A wrong `X-Acuity-Schema` value returns 400                                        | curl with `X-Acuity-Schema: 2` (H5)                     | attested 2026-06-23 (H5 curl)                      |
| 5  | An unknown event `type` value returns 422                                           | curl with `"type":"nope"` (H6)                          | attested 2026-06-23 (H6 curl)                      |
| 6  | Server starts without `ACUITY_GOTIFY_TOKEN` and persists events                    | start without env var, verify log + POST (H7)           | attested 2026-06-23 (H7 startup + POST)            |
| 7  | `cargo test` (workspace) green, including persistence and Gotify-disabled tests    | `cargo test --workspace`                                | 138 workspace tests green; 20 acuity; clippy clean |
| 8  | `types.ts` regenerates with the 4-event discriminated union                        | `cargo run -p acuity-schema --bin codegen` (H8)         | attested 2026-06-23 (H8 codegen)                   |
