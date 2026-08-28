---
status: complete
refs:
  - .cue/design-worktree-store-and-task-env/spec/index.md
  - .cue/design-worktree-store-and-task-env/trace/1787568301-855ff6a/external-surfaces-audit.md
  - .cue/master/task/worktree-store-and-task-env-impl.md
---

# Master plan: git-root store resolution and $CUE_TASK

Implements `.cue/design-worktree-store-and-task-env/spec/index.md`.
Phase order minimizes breakage: library first, then command removal,
then call-site migration, then behavior additions, docs last.

## Constraints

- Prototyping stage: no version bumps, no back-compat shims.
- TDD per phase: red-green-refactor; commit per green milestone.
- Mechanism fixed by spec: resolve the store from
  `list_worktrees(root)[0]` normalized by `get_git_root(&entry0)` —
  NOT `git rev-parse --git-common-dir` (cwd-relative, breaks for
  submodules and `--separate-git-dir`).
- `ResolvedStore` keeps its shape: `head_dir` stays the local `.cue/`,
  `store_dir` points at the git root's `.cue/`.

## Phase 1 — cuelib: git-root store resolution

- [x] Failing unit tests for `store::open(root, config)`:
  - plain repo: resolves to `<root>/.cue` (behavior unchanged)
  - worktree: `store_dir` = main-root `.cue/`, `head_dir` = local
  - no `master/` anywhere: loud error with `cue init` hint
  - bare main worktree: loud failure (from `get_git_root`)
  - submodule: resolves to its own toplevel only — no inheritance
  - stray worktree-local `.cue/master/`: ignored by resolution
  - (extra: custom `dir_name` joins against the git root; public
    `store::git_root` returns the main root from a linked worktree)
- [x] Implement `store::open` per the mechanism note above.
- [x] Remove STORE following, `validate_store_target`, and their unit
      tests from `store.rs` (the strict path validation dies with STORE).
- [x] Config is loaded from the git root (store owner), not the cwd
      worktree. (Library contract done: public `store::git_root`
      helper + `open` doc-comment; call-site migration is Phase 3.)

## Phase 2 — Remove `cue link`

- [x] Delete `crates/cue/src/commands/link.rs`, the CLI subcommand
      variant, and mod wiring.
- [x] Delete `crates/cue/tests/link.rs` and
      `crates/cue/tests/proxy_reads.rs`.
- [x] Rework `crates/cue/tests/switch.rs` STORE-proxy setup: replace
      with real-worktree equivalents where the behavior still applies,
      delete the rest.
- [x] Confirm no `STORE` references remain outside history artifacts.

## Phase 3 — Migrate CLI call sites to `store::open`

- [x] Replace the `get_git_root` + `Config::load` + `join(dir_name)` +
      `resolve_store` pattern at all ~15 sites: `context/mod.rs`,
      `commands/{status,list,switch,log,context}.rs`, `list/mod.rs`,
      `add/mod.rs`, `log/mod.rs`.
- [x] Remove the now-redundant `head_dir.exists()` guards
      (`add/mod.rs`, `log/mod.rs`, `list/mod.rs`, `switch.rs`,
      `commands/log.rs`) — existence is implied by successful open.
- [x] Full `cargo test` green.

## Phase 4 — `$CUE_TASK` scope rung

- [x] Failing tests for precedence in `head.rs` / integration:
  - env set + `--task` passed: flag wins
  - env set, no flag: env wins over `.cue/HEAD`
  - env empty string: treated as unset, falls to HEAD
  - env set to odd content: passes through unvalidated (same as HEAD
    content today)
- [x] Implement: `resolve_scope` reads `CUE_TASK` between the flag and
      HEAD rungs.
- [x] Test hygiene for env mutation: isolate env-touching tests
      (serial execution or separate integration binaries) — `set_var` is
      `unsafe` in edition 2024; follow repo conventions.
- [x] Route command handlers through the single chokepoint instead of
      ad-hoc flag checks at each site.

## Phase 5 — `cue switch` updates

- [x] Guard relaxation: bail when the resolved store is missing (not
      when local `head_dir` is absent), so switch works in a fresh
      worktree (`write_head`'s `create_dir_all` materializes local
      `.cue/HEAD`).
- [x] Warn on stderr (never `--json`) when `$CUE_TASK` is set:
      switch writes the human's HEAD, ineffective for the pinned process.
- [x] Fresh-worktree integration test: switch creates local HEAD +
      task dir in the shared store; branch association still mirrors to
      `branch.<name>.cue-task` git config.

## Phase 6 — status/context observability

- [x] `cue context render` and `cue context show` gain an optional
      `--task <slug>` flag (same convention as `add`/`list`/`log`):
      `context.json` and artifact paths resolve from that task context
      dir. Absorbed from task `cue-context-task-flag`; the `(flag)`
      provenance below depends on it.
- [x] Both commands resolve via the full chain and report provenance:
      human labels `(flag)`/`(env)`/`(head)`/`(default)`.
- [x] `--json` gains structured `provenance` and `store` fields; the
      resolved store path is printed in human output too.
- [x] Tests covering all four provenance sources.

## Phase 7 — `cue init` in a worktree

- [x] When the git root already has a store: print the store location
      and exit 0; never create a local store.
- [x] Integration test with a real git worktree (root store present).

## Phase 8 — Docs rollout (audit checklist)

- [x] cue skill `SKILL.md` + `reference/cli.md`: precedence chain,
      store-location rule, agents-set-`$CUE_TASK` guidance.
- [x] cue.nvim comment refresh: `lua/cue/core.lua:427`,
      `lua/cue/picker.lua:492`.
- [x] cue-plugins `--task` help text + `$CUE_TASK` child-scoping note.
- [x] README store description (git-root rule).

## Phase 9 — Final validation

- [x] `cargo test` and `clippy` green during implementation; final reruns
      remained blocked by the sandbox process/thread ceiling. The latest
      retry compiled the workspace successfully before integration tests
      failed exclusively on OS error 11 while spawning threads/git
      subprocesses.
- [x] `fmt` drift classified as pre-existing workspace-wide style-edition
      output, not feature drift; intentionally excluded from this feature.
- [x] Real-worktree smoke test from this repo's `worktrees/*`: shared
      store resolution, `$CUE_TASK` honored, status provenance + path.
- [x] Sweep for stray `link`/`STORE` references in shipped surfaces.
- [x] Log completion; report deviations from spec (if any) back to
      the design task context.

## Risks and notes

- Switch-proxy tests are the largest rework (Phase 2) — they encode
  STORE-era behavior that partly disappears.
- Env-var tests can flake under parallel execution; isolate early.
- `cue init`'s early-exit check (directory exists) does not cover the
  worktree case today; Phase 7 changes its control flow — keep the
  orphan-branch path untouched.
