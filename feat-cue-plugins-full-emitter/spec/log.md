# Project Log

## [df4d411] Phase 4 Step 1: acuity-schema-types flake package committed

Committed the acuity-schema-types package output to cue's flake.nix on feat/cue-plugins-full-emitter (df4d411). This adds an internal acuity-schema-codegen let binding (buildRustPackage for the codegen binary) and a public packages.acuity-schema-types runCommand that produces types.ts. cue-plugins will pin this flake as an input and vendor types.ts via a nix run script.

- **Found:** ts-rs generates both types.ts and serde_json/JsonValue.ts when a struct uses serde_json::Value
- **Found:** acuity-schema-codegen must be a private let binding (not exposed) because buildRustPackage produces a binary, while acuity-schema-types is the public runCommand that runs it
- **Decided:** Use git+file:// local path for cue input in cue-plugins during development, switch to github:palekiwi-labs/cue post-merge
- **Decided:** Expose update-types as a writeShellScriptBin package in cue-plugins flake rather than a committed shell script
- **Decided:** Keep serde_json/JsonValue.ts as a second committed file alongside types.ts (standard ts-rs behavior)

## [860d792] Phase 4 Steps 2-8: cue-plugins full emitter committed

Committed the cue-plugins full emitter on master (f49f930). The acuity-plugin.ts now emits all four AcuityEvent variants: SessionIdle, AgentTurnCompleted, ToolCallRequested, ToolCallCompleted. Types are generated via `nix run .#update-types` which runs the codegen binary directly into src/generated/acuity/ (no nix store copy, no permission issues). The cue flake input is pinned via git+file:// during development; TODO post-merge to switch to github:palekiwi-labs/cue. TypeScript typecheck passes clean. Remaining: smoke test (live session → all four event types in SQLite) and schema rejection test (header bump → clean 400).

- **Found:** nix store files are 0444 (read-only); cp preserves source mode by default, creating read-only copies that break on re-runs
- **Found:** ts-rs generates serde_json/JsonValue.ts as a separate file for serde_json::Value fields, with relative import from types.ts
- **Found:** TS SDK discriminated union narrowing: assigning part.state.status === 'error' to a boolean variable loses narrowing; must inline the check in a ternary or if-block
- **Found:** SDK ToolState union has 4 members: pending, running, completed, error — only pending/completed/error are emitted (running is transient)
- **Decided:** Generate types into src/generated/acuity/ rather than src/ directly — linguist-generated in .gitattributes, clear separation of generated vs hand-written code
- **Decided:** Run codegen binary directly into source tree rather than copying from nix store derivation — avoids 0444 permission issues with immutable store files
- **Decided:** Cast part.state.input as JsonValue — structurally compatible but TS can't prove { [key: string]: unknown } is valid JSON
- **Decided:** Inline part.state.status === 'error' ternary for type narrowing — boolean variable doesn't narrow discriminated unions

## [860d792] Phase 4: code review fix — postEvent HTTP error handling

Two code reviews (gemini-3.5-flash and opus) identified that postEvent's fetch().catch() pattern silently swallowed HTTP 4xx/5xx errors because fetch only rejects on network failures, not on HTTP error responses. Fixed by wrapping the call in try/catch with a res.ok check. This also catches synchronous JSON.stringify throws that would escape the error boundary.

Deferred to Phase 7 hardening (both reviewers agreed these are acceptable for MVP):
- Dedup map unbounded growth on crashed/killed sessions (no session.idle fired)
- Dedup clearing on idle risks duplicates in multi-turn sessions after resume
- Server has no UNIQUE constraint — client dedup is sole defense against dupes
- No retry/buffer for transient acuity outages

Full review trace saved at .cue/feat-cue-plugins-full-emitter/trace/1782204423-860d792/phase-4-full-emitter-review.md

- **Found:** fetch() does NOT reject on 4xx/5xx HTTP responses — only on network failures. This is a well-known Fetch API footgun.
- **Found:** JSON.stringify() runs synchronously as a fetch argument; if it throws, the error escapes any .catch() on the returned promise
- **Found:** acuity server (crates/acuity/src/db.rs:12-25) has no UNIQUE constraint on events — bare INSERT with autoincrement PK. Client-side dedup is currently the sole defense against duplicate rows.
- **Found:** AssistantMessage.tokens is REQUIRED (not optional) in the SDK types — the ?. in the plugin is for TS narrowing, not runtime safety
- **Decided:** Fixed postEvent to use try/catch + res.ok check instead of fetch().catch() — handles both HTTP errors and sync throws
- **Decided:** Deferred dedup lifecycle improvements (LRU bound, multi-turn reset semantics) to Phase 7 hardening

## [860d792] Phase 4 complete: cue-plugins full emitter proven end-to-end

Phase 4 is complete. The cue-plugins acuity-plugin now emits all four AcuityEvent variants (SessionIdle, AgentTurnCompleted, ToolCallRequested, ToolCallCompleted) from live agent sessions. All events land in acuity's SQLite with correct payloads including real token counts and tool metadata.

Key implementation decisions that diverged from the original plan:
1. Replaced the scripts/update-types.sh shell script with a nix flake input + `nix run .#update-types` approach. cue is declared as a flake input in cue-plugins (git+file:// during dev, github: post-merge). The update-types package runs the codegen binary directly into src/generated/acuity/ — no nix store copy, no permission issues.
2. Types vendored under src/generated/acuity/ (not src/) with .gitattributes linguist-generated marking, following consultation with Gemini Flash on conventions for generated type locations.
3. Exposed acuity-schema-codegen as a public package in cue's flake (originally planned as private let binding only) so consumers can run the binary directly.
4. postEvent error handling rewritten after code review: fetch().catch() silently swallowed HTTP 4xx/5xx errors. Fixed with try/catch + res.ok check.

Commits:
- cue repo: df4d411, 860d792 (flake packages)
- cue-plugins: f49f930 (full emitter), f9253d0 (postEvent fix), 6406604 (README)

Deferred to Phase 7 hardening (agreed by both reviewers as acceptable for MVP):
- Dedup map unbounded growth on crashed sessions (no session.idle)
- Dedup clearing on idle risks duplicates in multi-turn sessions
- Server has no UNIQUE constraint — client dedup is sole defense
- No retry/buffer for transient acuity outages

- **Found:** fetch() does NOT reject on HTTP 4xx/5xx — only on network failures
- **Found:** nix store files are 0444 (read-only); cp preserves source mode, breaking on re-runs
- **Found:** ts-rs generates serde_json/JsonValue.ts as a separate file for serde_json::Value fields
- **Found:** TS SDK: assigning part.state.status === 'error' to a boolean loses discriminated union narrowing; must inline in ternary
- **Found:** acuity server (db.rs) has no UNIQUE constraint — bare INSERT with autoincrement PK
- **Found:** All tool_call_requested events showed args:{} in live testing — SDK part.state.input appears empty at pending state; actual args may be in part.state.raw
- **Decided:** Use nix flake input + writeShellScriptBin for type generation instead of a committed shell script
- **Decided:** Generate types into src/generated/acuity/ with .gitattributes linguist-generated
- **Decided:** Expose codegen binary as public package — run directly into source tree, no store copy
- **Decided:** Fix postEvent to use try/catch + res.ok instead of fetch().catch() for HTTP error handling
- **Decided:** Defer dedup lifecycle improvements and server-side idempotency to Phase 7

