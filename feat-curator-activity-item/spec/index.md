# Branch: feat/curator-activity-item

## Purpose

Build a rich activity feed in the `curator` TUI that surfaces per-session
turns, tool calls, and lineage metadata (parent session, agent, model, title)
captured from the opencode plugin pipeline through the `acuity` SQLite event
store.

The pipeline: **opencode-plugin** (TypeScript) captures events from the
opencode runtime and POSTs them to the **acuity** server (Rust/axum), which
persists them to SQLite and serves them to the **curator** TUI (Rust/ratatui)
via SSE.

## Current state (as of d590d61)

### Shipped and live-verified

| Slice | Scope | Commits |
|-------|-------|---------|
| 5 | `ActivityItem` enum + `build_activity_items` pure function | ae09f03, 0c2ff37 |
| 5 review fixes | `or_insert_with` fold-check hoist, edge-case tests | 3976d5c, 0c2ff37 |
| 6a collection | `SessionUpdated` schema variant + curator `SessionSummary` ingest + plugin handlers | 7f4bac5, 1acf52f, 8555572 (cue-plugins) |
| 6a logging | Acuity dual-layer tracing (stderr + optional file), per-variant INFO fields, DEBUG raw-vs-parsed | 369a0d1 |
| 6a hardening | Plugin dedup (`lastSessionSig`), model capture from `AssistantMessage`, curator model ingest | f3b11f7, db90d51, 3aeb835 (cue-plugins); f5c1695, d590d61 (cue) |

### What works end-to-end

- Lineage (`parent_id`) captures correctly for sub-agent sessions
- Agent identifier captures correctly (plan, build, explore, etc.)
- Title arrives early via `session_updated` (before `session_idle`)
- Model captures correctly on every `agent_turn_completed` (including
  subagents — resolved per-turn from `AssistantMessage.providerID/modelID`)
- `session_updated` dedup collapsed from 5+ identical rows to 1-2 per session

## What's next

### Slice 6b — Rendering (stage B)

Plan: `plan/1782659497-0c2ff37/slice6b-rendering.md` (status: open, intentionally light)

The activity feed currently renders with an 8-char session-id truncation
(`ui.rs:214`) that causes collisions (opencode IDs share a per-run prefix).
Slice 6b kills the truncation, surfaces title + agent + lineage in headers,
and adds dev debug columns. It retains the flat reverse-chrono layout — the
nested tree is stage C.

### Stage C — Nested tree (future)

`build_activity_items` uses a `(session_id, project_dir)` header key that is
known-degenerate (`project_dir` is workspace-constant). Stage C reworks this
with `parent_id`-based nesting to fold child sessions under parent turns.

## Deferred work

| Item | Priority | Trigger |
|------|----------|---------|
| `normalize-events-sessions-table` | low | Adding a second harness ("pi"), or wanting persistent session summaries |
| `fix-displayed-timezone-of-received-at` | high | Display shows UTC instead of host timezone |
| Cast cleanup in plugin `SessionUpdated` handler | low | SDK adds `agent?`/`model?` to `Session` type |

## Two repos involved

- **cue repo:** `/home/pl/code/palekiwi-labs/cue` (branch `feat/curator-activity-item`)
- **cue-plugins repo:** `/home/pl/.config/opencode/plugin/palekiwi-labs/cue-plugins` (branch `master`)

The cue-plugins repo typechecks via `nix develop -c bash -c "bun run typecheck"`.

## Key references

- Cumulative history: `spec/log.md`
- Model-capture investigation: `doc/1782664752-0b0aa5b/opencode-model-capture.md`
- Slice 6b plan (next): `plan/1782659497-0c2ff37/slice6b-rendering.md`
- Sessions-table normalization todo: `todo/1782659497-0c2ff37/normalize-events-sessions-table.md`
