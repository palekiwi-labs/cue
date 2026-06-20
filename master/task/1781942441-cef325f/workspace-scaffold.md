---
title: Workspace & contract scaffolding
status: open
priority: normal
---
# Workspace & contract scaffolding

Add four new crate skeletons (`acuity-schema`, `acuity-api`, `acuity`,
`curator`) to the Cargo workspace, wired into the dependency graph but empty.
Stand up `ts-rs` codegen in `acuity-schema`. Bootstrap the `cue-plugins` repo
and establish the vendoring workflow for the generated `types.ts`.

## Source

- spec: `.cue/master/spec/cue-monorepo/index.md`
- roadmap: `.cue/master/trace/1781942441-cef325f/cue-ecosystem-roadmap.md` (Phase 0)

## Acceptance Criteria

| #  | Criterion                                                        | Verify by                              | Evidence |
| -- | ---------------------------------------------------------------- | -------------------------------------- | -------- |
| 1  | `cargo build` succeeds across all six workspace crates           | `cargo build --workspace`              |          |
| 2  | Codegen command exists and produces a `types.ts` from a Rust struct | run codegen command, inspect output |          |
| 3  | `cue-plugins` repo is initialised and contains the vendored `types.ts` | inspect repo                     |          |
