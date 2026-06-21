---
title: "acuity stateless MVP: session.idle -> Gotify"
status: in-progress
priority: normal
branch: "feat/acuity-mvp"
---
# acuity stateless MVP: session.idle -> Gotify

Implement the `SessionIdle` event type in `acuity-schema` with `serde` +
`ts-rs` derives. Generate `types.ts` and commit it into `cue-plugins`. Build
the `acuity` binary with a single POST endpoint that accepts a `session.idle`
event, validates the `X-Acuity-Schema` version header, and forwards a
notification to a configured Gotify instance. Write the opencode plugin in
`cue-plugins` using the vendored type.

Scope is intentionally narrow: no SQLite, no second event type, no SSE or
query surface. This phase retires the cross-repo schema contract risk and
replaces the hand-rolled notifications server in production.

## Source

- spec: `.cue/master/spec/acuity/index.md`
- spec: `.cue/master/spec/cue-monorepo/index.md`
- roadmap: `.cue/master/trace/1781942441-cef325f/cue-ecosystem-roadmap.md` (Phase 1)
- ref: `.ref/opencode-notifications-server-hook.ts`
- ref: `.ref/notifications-server/`

## Acceptance Criteria

| #  | Criterion                                                                           | Verify by                                         | Evidence |
| -- | ----------------------------------------------------------------------------------- | ------------------------------------------------- | -------- |
| 1  | A real `session.idle` POST from the opencode plugin triggers a Gotify notification  | run live agent session, observe Gotify            |          |
| 2  | The plugin imports the type from the vendored `types.ts`                            | inspect plugin source                             |          |
| 3  | The POST carries `X-Acuity-Schema: N` and acuity accepts it                        | inspect plugin + acuity logs                      |          |
| 4  | A deliberately wrong schema version header is cleanly rejected by acuity            | send bad-version POST with curl, observe 400      |          |
| 5  | Hand-rolled notifications server is decommissioned                                  | human attestation                                 |          |
