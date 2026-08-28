---
title: Normalize markdown filenames in cue add
status: open
priority: high
refs:
- .cue/design-worktree-store-and-task-env/log.md
- .cue/master/spec/cue/task-mode.md
- .cue/master/task/worktree-store-and-task-env-impl.md
kind: build
---
# Problem Statement

Task cards (and other markdown artifacts) are intermittently created
without the `.md` extension, breaking the documented
`.cue/master/task/<slug>.md` contract (task-mode spec, SKILL.md) and
creating two spellings of the same card path that rot `refs:` links.

**Impact is worse than cosmetic — the card silently disappears from
the primary surfaces:**

- `read_artifacts` hard-filters to `.md`
  (`crates/cuelib/src/artifact.rs:252-254`, deliberate and tested at
  `artifact.rs:561`); its production caller is the curator kanban
  (`crates/curator/src/app.rs:724`). An extensionless task card is
  invisible on the board.
- cue.nvim gates listings on `.md` too (`lua/cue/core.lua:627`,
  `name:match("%.md$")`).
- `cue list` still shows it (via `collect_files`, no filter), so the
  artifact exists in one view and vanishes in two others — silent
  data loss on the human-facing surfaces, invisible to agents.

Root cause is a chain of optional conventions with no enforcement at
any layer (investigated in design task
`design-worktree-store-and-task-env`, log entry "Root-caused
extensionless task card defect"):

- Agents habitually pass slug-like filenames without extension; tool
  schemas only hint at `auth-login.md` in a description.
- Plugin tools forward `filename` verbatim:
  `src/opencode/cue-task.ts:41` (opencode),
  `worktrees/feat-pi-extension/src/pi/tools/cue-task.ts:87` (pi).
- `cue add` writes verbatim: `crates/cue/src/add/mod.rs:86`;
  `validate_filename` (`add/mod.rs:186-202`) only blocks traversal.
- `collect_files` (`crates/cuelib/src/artifact.rs:150-165`) lists all
  files regardless of extension, so tooling never surfaces the defect.

# Fix

Normalize in `cue add` — the single CLI chokepoint covering opencode
tools, pi tools, and raw CLI users:

- Declare the markdown type set ONCE in `cuelib/src/artifact.rs`,
  beside `CANONICAL_TYPES`:

  ```rust
  /// Artifact types whose payload is a markdown document (frontmatter
  /// + markdown body). Written with a `.md` extension and surfaced by
  /// the board/listing readers.
  pub const MARKDOWN_TYPES: &[&str] = &["doc", "note", "plan", "ref",
      "spec", "task", "todo"];
  ```

- `add` normalizes against it: if the filename has NO extension and
  the artifact type is in `MARKDOWN_TYPES`, append `.md`.
- Filenames with any extension pass through untouched (clipboard
  images `.png`/`.jpg`, traces `.log`, extensionless `bin` payloads).
- `trace`, `tmp`, `bin` are exempt by construction (not in the set).
- Rationale: the read side already enforces the markdown invariant
  (`read_artifacts` filter); this closes the write-side gap of the
  same invariant instead of inventing a new rule. Over time
  `read_artifacts`' ad-hoc `.md` check derives from the same concept.

Note: the type->markdown mapping exists nowhere as data today —
`artifact_types` config (`crates/cuelib/src/config.rs:27`) is a flat
string list; the invariant lives only implicitly in reader code. The
const makes it explicit.

Optional hardening (fail-fast, not required for the fix): add a
`\.md$` pattern check on the `filename` schemas of the two plugin
tools so callers get immediate feedback.

# Acceptance criteria

- Unit/integration tests in `crates/cue/tests/add.rs`: extensionless
  markdown-type filename gets `.md`; extensioned filenames untouched;
  exempt types untouched (extensionless `bin` stays extensionless).
- `cue-task` tool call with `filename: "foo"` produces
  `.cue/master/task/foo.md`.
- `cargo test` green; clippy and fmt clean.
- Prototyping stage: no version bumps, no back-compat shims.

# Open question

Scope of `MARKDOWN_TYPES`:

- **Full set** (`doc note plan ref spec task todo`) — matches the
  documented convention everywhere (SKILL.md, specs promise
  `<type>/<name>.md` paths for all of these). Lean: this one; nothing
  is lost by codifying the promise.
- **Minimum** (`task` only) — exactly the set today's machine gates
  enforce (curator board reads `task`; nvim gate is type-agnostic
  path listing).

Decide before implementation; both fit the same code shape.

# Working rules

- Small scope: single-purpose branch `fix/add-filename-normalization`
  (worktree `worktrees/fix-add-filename-normalization`, base `master`).
- TDD: failing tests first, then the normalization.
- Log milestones to this task context.
