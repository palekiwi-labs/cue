---
status: open
---
# Phase 4 Review — cue-plugins Full Emitter

Reviewer: opus (read-only). Branch: feat/cue-plugins-full-emitter.
Sources verified against committed code + @opencode-ai/sdk v1.17.9 types
(dist/gen/types.gen.d.ts) + acuity server ingest/db.

## Findings

| # | Issue | Severity | Location |
|---|---|---|---|
| 1 | Server has no unique constraint; client dedup is sole defense | MEDIUM (arch) | crates/acuity/src/db.rs:12-25,60-72 |
| 2 | JSON.stringify/fetch sync throw escapes `.catch` | MEDIUM | acuity-plugin.ts:32-41 |
| 3 | Dedup unbounded if session never idles (crash/kill) | MEDIUM | acuity-plugin.ts:75 |
| 4 | Clearing ALL dedup on idle -> dup risk multi-turn/resume | MEDIUM | acuity-plugin.ts:75,85 |
| 5 | dedup.delete after await -> idle-clear race window | MINOR-MEDIUM | acuity-plugin.ts:64,75 |
| 6 | No retry/buffer; transient outage loses events | MEDIUM (ok for MVP) | acuity-plugin.ts:28-42 |
| 7 | git+file:// local-path flake input | MINOR (deferred) | cue-plugins/flake.nix:9-13 |
| 8 | rm -rf not guarded by CWD sentinel | MINOR | cue-plugins/flake.nix:43 |
| 9 | Stale committed dist/types.ts (only SessionIdle) | MINOR | crates/acuity-schema/dist/types.ts |
| 10 | Misleading "tokens optional" comment | MINOR | acuity-plugin.ts:90 |

Not bugs: `else if` pending/completed (discrete events, correct);
`args as JsonValue` cast (sound); vendored generated types + .gitattributes
(correct choice).

## Verified SDK facts
- AssistantMessage.tokens is REQUIRED; .input/.output required number
  (types.gen.d.ts:117-119). No AssistantMessage subtypes.
- time.completed? optional number (types.gen.d.ts:104) -> guard correct.
- ToolState 4 members; running transient/skipped (types.gen.d.ts:262).
- ToolStateError.error required string (types.gen.d.ts:253) -> safe.
- EventMessagePartUpdated.part is full Part union with delta? streaming
  field (types.gen.d.ts:354-358) -> dedup necessary.

## Top recommendations
1. Fix #2 (try/catch around stringify+fetch) — defeats error isolation goal.
2. Decouple dedup memory-bound from correctness reset (#3/#4/#5):
   never reset within a live session; bound via LRU / session.deleted;
   move delete before the await.
3. Add server-side idempotency (#1) as durable backstop:
   UNIQUE(event_type,session_id,turn_id,tool_call_id) + INSERT OR IGNORE.
4. Nix integration is well-architected; remaining items deferred/minor.
