# External surfaces audit: cue link / STORE removal and $CUE_TASK

Delegated scan (explore agent) of the four surfaces affected by the
worktree-store + $CUE_TASK design. Verdicts per surface, citations
included. This artifact is the build-planning input for documentation
and test rollouts.

## cue.nvim (/home/pl/code/palekiwi-labs/cue.nvim)

- No `cue link`, `STORE`, or independent store-path resolution logic.
  The plugin shells out to the `cue` CLI throughout.
- link/STORE removal: UNAFFECTED.
- Stale-doc spots to refresh when $CUE_TASK lands:
  - lua/cue/core.lua:427 — comment claims context is "resolved from
    .cue/HEAD via cue status" (true but incomplete once env rung exists).
  - lua/cue/picker.lua:492 — comment "No explicit scope: picker follows
    HEAD" (will follow the full precedence chain).
  - lua/cue/picker.lua:36, lua/cue/core.lua:681 — `--task` flag usage;
    behavior unchanged.
- $CUE_TASK: UNAFFECTED behaviorally (consumes `cue status --json`,
  which will resolve correctly); refresh comments only.

## cue-plugins (/home/pl/.config/opencode/plugin/palekiwi-labs/cue-plugins)

- No `cue link`, `STORE`, or independent store resolution.
- link/STORE removal: UNAFFECTED.
- $CUE_TASK: behavior UNAFFECTED (docs mention `.cue/HEAD` in help
  text only; e.g. src/opencode/cue-add.ts:29,46). Refresh help text to
  mention the env rung when it ships. NOTE: these plugins are the
  natural place to DOCUMENT that agents may set $CUE_TASK themselves
  for child-process scoping.

## cue skill (/home/pl/.agents/skills/cue)

- No `cue link` / `STORE` references. UNAFFECTED by removal.
- $CUE_TASK: DEGRADE (stale docs) — must be updated at build time:
  - SKILL.md:73 — "Active Context (.cue/HEAD)" description needs the
    full precedence chain (--task > $CUE_TASK > .cue/HEAD > master).
  - SKILL.md:234 — "DON'T run cue switch or mutate .cue/HEAD" rule
    stays; agents should set $CUE_TASK instead (document this).
  - reference/cli.md:8 — `cue status` description (resolved from
    .cue/HEAD) needs the chain.
  - reference/cli.md:33 — `--task <SLUG>` description needs the chain.
- Per user decision: the skill must also document the store location
  rule (always <git-root>/.cue, shared across worktrees) so agents do
  not look in the wrong location.

## cue repo (/home/pl/code/palekiwi-labs/cue, non-store surfaces)

- BREAK (expected, in-scope implementation work):
  - crates/cue/src/commands/link.rs — delete with the command.
  - crates/cue/tests/link.rs:33 — `.cue/STORE` assertions; delete.
  - crates/cue/tests/switch.rs proxy-mode tests (STORE setup) —
    rewrite or delete.
  - crates/cue/tests/proxy_reads.rs — proxy-store reads; delete.
  - crates/cuelib/src/store.rs:31-66 — resolve_store rewrite site.
  - crates/cuelib/src/head.rs:30-32 — resolve_scope env-rung site.
- DEGRADE (docs):
  - README.md:30 — diagram mentions ".cue/ (store)"; refresh to state
    the git-root store rule.
  - Historical specs/logs under .cue referencing link/STORE are
    versioned memory, not living docs; leave as history.

## Rollout checklist (for build planning)

1. Remove link command, STORE following, and their tests (BREAK items).
2. Implement git-root store resolution + $CUE_TASK rung.
3. Update cue skill (SKILL.md, reference/cli.md): precedence chain +
   store location rule.
4. Update cue.nvim comments (core.lua:427, picker.lua:492).
5. Update cue-plugins help text; document $CUE_TASK for child agents.
6. Refresh README store description.
