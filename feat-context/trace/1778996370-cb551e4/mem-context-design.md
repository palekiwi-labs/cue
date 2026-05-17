# mem context — Feature Design Plan

## Summary

A new `mem context` subcommand that allows precise composition of AI agent
context from existing artifacts, supporting cross-branch references, git diffs,
and named profiles.

## Commands

```
mem context init [--force]           # create .mem/<branch>/context.json
mem context show                     # pretty-print the raw JSON
mem context cat [--name <profile>]   # expand and concatenate to stdout
```

## context.json Schema

```json
{
  "default": {
    "artifacts": [
      "./spec/index.md",
      "./spec/plan.md",
      "./spec/log.md",
      "@1234-new-feature-db-migration:spec/plan.md"
    ],
    "diff": "head",
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

## Path Notation

- `./spec/index.md` — current branch, relative to `.mem/<branch>/`
- `@branch-name:spec/plan.md` — explicit cross-branch artifact reference
- No `..` traversal allowed anywhere (validated same as `add.rs`)

## `include` Semantics

- `@branch` — includes that branch's `default` profile
- `@branch:profile` — includes a specific named profile
- Resolution: DFS, recursive, with cycle detection on `(branch, profile)` pairs
- Deduplication: first occurrence wins
- Ordering: included artifacts prepended (parents before children), then current profile's artifacts

## `diff` Field

String enum (omit for none):
- `"head"` — `git diff HEAD` (staged + unstaged)
- `"staged"` — `git diff --cached`
- `"branch"` — full branch diff vs. detected or specified base

## `mem context cat` Output Format

XML-style tags for AI parseability:

```xml
<artifact path=".mem/main/spec/index.md">
...file content...
</artifact>

<artifact path=".mem/1234-new-feature-db-migration/spec/plan.md">
...file content...
</artifact>

<diff mode="head">
...git diff output...
</diff>
```

## `mem context init` Behaviour

- Scans `.mem/<branch>/spec/` for existing files
- Auto-populates `default` profile with `./spec/<filename>` paths (sorted)
- Errors if `context.json` already exists (unless `--force`)

## Implementation Architecture

New module: `src/commands/context/{mod,init,show,cat}.rs`

Core types:
```rust
#[derive(Deserialize, Serialize, Default)]
pub struct ContextProfile {
    #[serde(default)]
    pub artifacts: Vec<String>,
    pub diff: Option<String>,
    #[serde(default)]
    pub include: Vec<String>,
}

pub type ContextConfig = HashMap<String, ContextProfile>;
```

CLI additions in `cli.rs`:
- `Commands::Context { command: ContextCommands }`
- `ContextCommands`: `Init { force: bool }`, `Show`, `Cat { name: Option<String> }`

## Design Decisions

| Question | Decision |
|---|---|
| Cross-branch path notation | Sigil syntax `@branch:path` |
| `include` semantics | Named profile (`@branch` implies `:default`), recursive + cycle detection |
| `cat` output format | XML-style `<artifact>` tags |
| `diff` field type | String enum, not bool |
| `init` defaults | Auto-populate from existing `spec/` files |
| Glob patterns | No — exact paths only for v1 |
| Edit subcommands | No — direct JSON editing for v1 |
