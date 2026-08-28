---
status: complete
refs: spec/index.md
parent: .cue/cue-agent/plan/index.md
---
# Phase 0 — scaffold & spec parsing

Executive plan for phase 0 of the cue-agent master plan
(`.cue/cue-agent/plan/index.md`).

## Scope

- `crates/cue-agent` skeleton (lib + bin) wired into the workspace
- CLI shell: `cue-agent run [FLAGS] [JSON_SPEC]`, three input modes,
  orchestrator flags; usage errors exit 2 (stderr message, empty stdout)
- Spec model: serde structs, defaults, `{file}` interpolation, validation

## Steps

- [x] Workspace wiring: add `crates/cue-agent` member, Cargo.toml
      (lib cue_agent + bin cue-agent), empty lib + minimal clap main
- [x] Input modes: positional JSON string, `--spec-file PATH`, `-` stdin;
      missing input and mode conflicts exit 2
- [x] Orchestrator flags: `--task SLUG` (validated via cuelib slug rules),
      `--concurrency N` (u64, 0 = unbounded), `--timeout SECS` (reject 0)
- [x] Spec model: full field reference, kebab-case serde renames,
      deny_unknown_fields, normalized post-parse model with defaults
      (harness pi, approve false, session {persist:false},
      worktree {mode:cwd}, background false)
- [x] Validation (all exit 2): top-level array required, empty array
      rejected, unknown fields, prompt required, harness != pi,
      background true, duplicate run ids, thinking vocabulary,
      worktree mode/name/base consistency, session.id without persist,
      empty id strings, env key sanity
- [x] `{file}` interpolation for prompt / system-prompt /
      append-system-prompt, resolved relative to the spec file dir
      (cwd for stdin/argv); missing file exits 2 naming the file
- [x] Integration tests via assert_cmd for every exit-2 path and the
      exit-0 happy paths; unit tests for defaults and normalization
- [x] Gate: cargo build, cargo test, clippy, fmt clean; commits +
      log entries

## Notes

- Valid spec parse in phase 0 exits 0 with empty stdout; execution and
  receipts land in phase 1.
- Default run-id minting (run-N) is phase 2; duplicate check only sees
  explicit ids. Phase 2 minting must also dodge explicit run-N ids.
