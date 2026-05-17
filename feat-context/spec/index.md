# Feature: `mem context`

## Overview

`mem context` introduces a **context object** — a branch-specific configuration file
that allows precise, reproducible composition of AI agent context at the start of a
session. Instead of manually instructing the agent to locate and read particular
artifacts, the user defines one or more named *profiles* in a `context.json` file.
`mem context render` expands a profile into a structured stream that can be piped
directly into an AI agent.

## Problem

When starting a new agent session:
- The user must manually instruct the agent to read specific files (spec, plan, log)
- With many artifacts per branch, it's easy to include too much or too little context
- Multi-branch PR workflows (e.g. a feature branch built on a DB migration branch)
  have no way to compose context from multiple branches without manual coordination
- The agent may read additional artifacts (e.g. trace reports) not relevant to the
  current session focus

## Core Concepts

### Context Profile

A named collection of artifacts, an optional git diff, and optional references to
profiles on other branches. Profiles are defined in `context.json` and identified by
name. The `default` profile is used when no name is specified.

### Context File Location

`.mem/<sanitized-branch-name>/context.json`

Stored in the `mem` worktree alongside all other branch artifacts.

### Profile Resolution

`include` references are resolved recursively (DFS), prepended to the current
profile's artifacts. This ordering reflects LLM recency bias: base context appears
early in the stream, current working context appears later, and the diff appears last.

## Commands

| Command                             | Description                                                     |
|-------------------------------------|-----------------------------------------------------------------|
| `mem context init [--force]`        | Create `context.json`, auto-populated from existing spec/ files |
| `mem context show`                  | Print raw `context.json`                                        |
| `mem context profiles`              | List available profile names                                    |
| `mem context render [-p <profile>]` | Expand and stream context to stdout                             |

`render` defaults to the `default` profile when `-p` is omitted.

## `context.json` Schema

```json
{
  "default": {
    "artifacts": [
      "./spec/index.md",
      "./spec/plan.md",
      "./spec/log.md",
      "@1234-new-feature-db-migration:spec/plan.md"
    ],
    "diff": "main...HEAD",
    "include": [
      "@1234-new-feature-db-migration",
      "@some-other-branch:brief"
    ]
  },
  "brief": {
    "artifacts": [
      "./spec/index.md",
      "./spec/plan.md"
    ]
  }
}
```

### Fields

- **`artifacts`**: Ordered list of artifact paths. Two notations supported (see below).
- **`diff`**: Raw string passed verbatim to `git diff`. Omit for no diff.
  Examples: `"HEAD"`, `"main...HEAD"`, `"--staged"`, `"origin/main"`.
- **`include`**: Ordered list of profiles from other branches to prepend.
  Resolved recursively with cycle detection.

## Path Notation

| Syntax                            | Resolves to                                          |
|-----------------------------------|------------------------------------------------------|
| `./spec/index.md`                 | `.mem/<current-branch>/spec/index.md`                |
| `@branch-name:spec/plan.md`       | `.mem/branch-name/spec/plan.md`                      |
| `@branch-name` (in `include`)     | `default` profile of `.mem/branch-name/context.json` |
| `@branch-name:brief` (in `include`) | `brief` profile of `.mem/branch-name/context.json` |

Branch names in the sigil follow the same sanitisation as the rest of `mem`
(slashes and backslashes replaced with dashes).

## Include Resolution Algorithm

Resolution is depth-first. For a profile P on branch B:

1. If `(B, P)` is already in the visited set → error: cycle detected
2. Add `(B, P)` to the visited set
3. For each entry in `P.include`:
   - Parse `@target-branch` (implies `:default`) or `@target-branch:target-profile`
   - Recursively resolve `(target-branch, target-profile)` → produces ordered artifact list
   - Append those paths to the accumulator (deepest dependency resolved first)
4. Append `P.artifacts` to the accumulator
5. Deduplicate the full list: first occurrence of each path wins

**Output ordering** (LLM recency bias aware):
1. Artifacts from included profiles (base/upstream context — appears early)
2. Current profile's own artifacts (current working context)
3. `<diff>` block last (most immediate; benefits from recency bias)

## `mem context render` Output Format

Artifacts are wrapped in XML tags. The `path` attribute is relative to the git
repository root.

```xml
<artifact path=".mem/main/spec/index.md">
# mem: Agent Memory System
...file content...
</artifact>

<artifact path=".mem/1234-new-feature-db-migration/spec/plan.md">
...file content...
</artifact>

<diff args="main...HEAD">
diff --git a/src/main.rs b/src/main.rs
...
</diff>
```

- **stdout**: Artifact content only — pipe-clean for `mem context render | ai-tool`
- **stderr**: Warnings for missing artifacts, cycle errors, git diff failures

### Missing Artifacts

If a referenced file does not exist (e.g. included branch deleted post-merge), it is
**silently skipped**. A warning is emitted to stderr. No content appears in stdout for
that artifact.

## Security Model

- `./...` paths: reject any `..` path components (same rule as `mem add`)
- `@branch:path` paths: branch component must match a sanitized name (no raw slashes,
  no `..`); path component independently rejects `..` components
- No shell expansion or command substitution in any path or diff value

## `mem.json` Additions

Two new optional fields in the project/global config:

```json
{
  "diff_exclude_paths": ["Cargo.lock", "*.lock"],
  "base_branch_cmd": "gh pr view --json baseRefName --jq '.baseRefName'"
}
```

- **`diff_exclude_paths`**: Glob patterns excluded from all git diff output produced
  by `mem context render`. Applied project-wide, not per-profile.
- **`base_branch_cmd`**: Shell command returning the base branch name. Reserved for
  future tooling. Does not affect `context.json` — `diff` values are always explicit
  git ref strings.

## Out of Scope for v1

- **Token counting** (`mem context render --tokens`): Deferred pending tokenizer crate
  evaluation (`tiktoken-rs`). Will report per-artifact and total counts to stderr.
- **Glob patterns** in artifact paths (e.g. `"./spec/*.md"`)
- **`exclude` array** for overriding artifacts from included profiles
- **Per-profile diff path filtering** (currently project-wide only via `mem.json`)
- **Line ranges** for large artifacts (e.g. `@branch:doc/huge.md#L100-200`)
- **CLI add/remove helpers** for context.json (direct JSON editing for v1; TUI planned)
