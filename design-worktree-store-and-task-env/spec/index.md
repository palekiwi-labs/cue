---
status: complete
refs:
- .cue/master/spec/cue/task-mode.md
- .cue/master/task/cue-agent.md
---
# First-class git worktree support: root store resolution and $CUE_TASK

## Context

`cue` stores its memory on an orphan branch inside the very repo it serves,
materialized as a git worktree of that branch at `.cue/` in the git root.
Git forbids checking out the same branch in two worktrees, so the store is
structurally singular per repository: every worktree of a repo necessarily
shares one store. The current machinery does not exploit this. Each
worktree must run `cue link`, which writes a `.cue/STORE` redirect file
that `resolve_store` follows. The link workflow is unused and clunky; the
indirection is unnecessary.

Separately, `.cue/HEAD` (the active-scope pointer) is owned by the human,
but agents need a reliable, process-scoped way to pin their task context —
especially child agent processes spawned by the upcoming `cue-agent`
binary, which launches harness processes with an environment overlay.
`task-mode.md` deferred exactly this problem ("Deferred: Git worktrees");
this spec delivers it.

## Store resolution

One rule: **the store is always the `.cue/` directory in the git root.**

- "Git root" is the main worktree: the first entry of
  `git worktree list --porcelain`, normalized through
  `git rev-parse --show-toplevel`. In the main checkout this is the
  current directory's toplevel, so plain repos are unchanged.
- If `<git-root>/.cue/` is not a valid store (no `master/`), fail loudly
  with a hint to run `cue init` in the main repo.
- **`cue link` and `.cue/STORE` are removed entirely.** No escape hatch,
  no cross-store sharing for non-worktree directories (hard-linked
  clones). One repo, one store, no exceptions. At prototyping stage this
  is final; a future revival is not blocked by anything.
- Stray content in a worktree's local `.cue/` (other than `HEAD`) is
  ignored. Store resolution never consults the local directory.
- `ResolvedStore` keeps both fields: `head_dir` stays the local `.cue/`
  (see scope resolution), `store_dir` points at the git root's `.cue/`.
- Config (`dir_name`, etc.) is loaded from the git root, the store owner —
  not from the current worktree.

Mechanism notes (preserved from the superseded design): use
`list_worktrees(root)[0]` normalized by `get_git_root(&entry0)`, NOT
`git rev-parse --git-common-dir` (cwd-relative, unreliable for submodules
and `--separate-git-dir`). The normalization is idempotent for normal
repos, resolves a submodule to its own toplevel (correct: no
inheritance), and fails loudly for a bare main.

## Active scope resolution

Precedence, in order:

1. `--task <slug>` flag — per-invocation override (unchanged).
2. `$CUE_TASK` environment variable — agent/process-scoped override.
   Empty or unset means "not set".
3. Local `.cue/HEAD` — the human's pointer for this checkout.
4. `"master"` — default.

Rules:

- **No inheritance.** A worktree never inherits HEAD from the git root:
  the root has a possibly unrelated branch checked out. Local
  `.cue/HEAD` present (and non-empty) wins for that worktree; otherwise
  `master`. This is the existing `resolve_scope` behavior — the spec
  formalizes it and forbids adding the inheritance rung.
- `$CUE_TASK` content gets **no special validation** — identical
  treatment to `.cue/HEAD` content today: malformed values surface
  through whatever handling commands already apply. (The strict path
  validation that `.cue/STORE` had dies with `STORE`.)
- `cue status` and `cue context` must respect the full precedence chain
  and report provenance (flag / env / head / default).
- `cue switch` **warns** when `$CUE_TASK` is set (switch writes the
  human's HEAD; under an active `$CUE_TASK` the write is likely
  ineffective for the current process). It proceeds; no refusal.

### Worktree materialization

- `cue switch <slug>` in a fresh worktree (no local `.cue/`) must work:
  the guard moves from "local head_dir exists" to "resolved store
  exists", and `write_head`'s `create_dir_all` creates the local
  `.cue/HEAD` as a side effect.
- A fresh worktree gets its HEAD populated by the human via
  `cue switch` — including the no-argument restore from the
  `branch.<name>.cue-task` git-config association.
- `cue init` run inside a worktree whose git root already has a store
  prints the store location and exits successfully; it never creates a
  local store.

### Consumers

`cue-agent` will set `$CUE_TASK` in the environment of spawned agent
processes (alongside its env overlay), guaranteeing children write to
the correct context even when they forget an explicit `--task`.

## Affected components

- `cuelib / store.rs` — remove `STORE` following; add git-root
  resolution (`list_worktrees[0]` + `get_git_root` normalization) and
  the loud `cue init` error; new `store::open(root, config)` chokepoint.
- `cue CLI` — delete the `link` command and its tests
  (`crates/cue/src/commands/link.rs`, `crates/cue/tests/link.rs`,
  `proxy_reads.rs`, the STORE setup in `switch.rs` tests); migrate the
  ~15 `get_git_root + join + resolve_store` call sites to
  `store::open`; fix the hardcoded-`.cue` bug by deletion.
- `cuelib / head.rs` — scope resolution gains the `$CUE_TASK` rung
  between flag and HEAD.
- `cue / commands/switch.rs` — guard relaxation; `$CUE_TASK` warning.
- `cue / commands/status.rs`, `context.rs` — precedence chain +
  provenance output (see "Observability").
- External surfaces (docs only; audit found no code dependencies on
  `link`/`STORE` outside this repo — full report in
  `trace/1787568301-855ff6a/external-surfaces-audit.md`):
  - cue skill (`~/.agents/skills/cue`): SKILL.md and
    `reference/cli.md` must document the precedence chain and the
    store-location rule (agents must not look for the store in a
    worktree's local `.cue/`).
  - cue.nvim: refresh comments (`core.lua`, `picker.lua`) that claim
    scope is resolved solely from `.cue/HEAD`.
  - cue-plugins: refresh `--task` help text; document that agents may
    set `$CUE_TASK` for child-process scoping.
  - README: store description states the git-root rule.

## Non-goals

- Cross-store sharing or store redirection of any kind.
- Isolated per-worktree stores.
- Branch-derived scope (stays rejected; `switch` branch association is
  write-path only).
- A cue daemon / file locking. Parallel-write coherency remains an
  accepted pre-existing constraint.
- Validation machinery for `$CUE_TASK` beyond existing command handling.

## Reference

- Delivers the deferral recorded in `spec/cue/task-mode.md`
  ("Deferred: Git worktrees").
- Supersedes and deletes `spec/cue/worktree-auto-store-resolution.md`
  (never implemented).

## Observability

Scoped commands, including `cue status` and `cue context`, respect the
full precedence chain. `cue status` reports provenance plus the resolved
store path so agents never look in the wrong location. Machine-oriented
`cue context show` and `cue context render` output remains unmodified:

- Human output: concise provenance labels `(flag)` / `(env)` /
  `(head)` / `(default)`; the resolved store path is printed.
- `--json` output: structured `provenance` field
  (`flag|env|head|default`) and a `store` path field.
- The `cue switch` `$CUE_TASK` warning goes to **stderr only**, never
  `--json` output (JSON stays parseable and machine-clean).

## Resolved questions

(Formerly open; all decided.)

- Provenance wording: `(env)`-style concise labels in human output;
  structured values in JSON. Store path printed in both.
- `cue switch` warning: stderr only.
- External audit: complete — no external code depends on `cue link` or
  `STORE`; only doc/comment refreshes needed (see Affected components
  and the trace report).
