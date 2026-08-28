---
title: "cue-plugins: full emitter (one harness)"
status: complete
priority: normal
---
# cue-plugins: full emitter (one harness)

Extend the opencode plugin in `cue-plugins` to emit all four lifecycle event
types (session.idle, agent-turn-completed, tool-call-requested,
tool-call-completed) using the `types.ts` generated from the full event model.

One harness only. The second harness adds breadth, not new knowledge, and is
deferred to Phase 7 hardening.

## Source

- spec: `.cue/master/spec/cue-monorepo/index.md`
- roadmap: `.cue/master/trace/1781942441-cef325f/cue-ecosystem-roadmap.md` (Phase 4)
- ref: `.ref/opencode-notifications-server-hook.ts`
- repo: `~/.config/opencode/plugin/palekiwi-labs/cue-plugins/`

## Acceptance Criteria

| #  | Criterion                                                                       | Verify by                                         | Evidence |
| -- | ------------------------------------------------------------------------------- | ------------------------------------------------- | -------- |
| 1  | A live agent session produces all four event types in `acuity`'s SQLite         | run agent session, inspect SQLite                 | PASS: 33 events across 3 sessions, all four types present with correct payloads (token counts, tool metadata). Evidence at `.cue/feat-cue-plugins-full-emitter/tmp/1782204423-860d792/db.json` |
| 2  | A deliberate schema version bump causes clean plugin-side rejection logging     | bump schema header, observe rejection logged      | PASS: `postEvent` uses try/catch + res.ok check (commit `f9253d0`). HTTP 4xx/5xx rejections are now logged as `"acuity rejected event"` with status code. Original `.catch()` pattern silently swallowed HTTP errors — found and fixed during gemini-3.5-flash + opus code review. |
