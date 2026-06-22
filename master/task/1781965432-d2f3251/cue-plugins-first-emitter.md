---
title: "cue-plugins: full emitter (one harness)"
status: open
priority: normal
---
# cue-plugins: full emitter (one harness)

Extend the opencode plugin in `cue-plugins` to emit all three lifecycle event
types (session.idle, tool-call-requested, tool-call-completed) using the
updated `types.ts` generated from the full event model.

One harness only. The second harness adds breadth, not new knowledge, and is
deferred to Phase 7 hardening.

## Source

- spec: `.cue/master/spec/cue-monorepo/index.md`
- roadmap: `.cue/master/trace/1781942441-cef325f/cue-ecosystem-roadmap.md` (Phase 4)
- ref: `.ref/opencode-notifications-server-hook.ts`
- repo: `~/.config/opencode/plugin/palekiwi-labs/cue-plugins/`

## Acceptance Criteria

| #  | Criterion                                                                      | Verify by                                         | Evidence |
| -- | ------------------------------------------------------------------------------ | ------------------------------------------------- | -------- |
| 1  | A live agent session produces all three event types in `acuity`'s SQLite       | run agent session, inspect SQLite                 |          |
| 2  | A deliberate schema version bump on the Rust side causes clean plugin rejection | bump schema version, observe 400 from acuity      |          |
