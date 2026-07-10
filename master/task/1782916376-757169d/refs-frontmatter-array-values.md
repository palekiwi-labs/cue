---
title: Frontmatter array values + mandated refs field
status: open
priority: high
---
# Frontmatter array values + mandated refs field

Enable the `cue` ecosystem to write list-valued frontmatter (notably `refs:`)
so that the upcoming `acumen` graph index has edges to extract. This is the
producer side of roadmap Phase 0 (`master/plan/acumen-roadmap.md`).

## Source

- `master/plan/acumen-roadmap.md` — Phase 0 (Frontmatter foundations)
- `master/spec/acumen/index.md` — `refs:` / `git_branch:` field spec (lines 94-121)

## Design

General, field-agnostic array support via **repeated key** semantics on the
existing `-f KEY=VALUE` flag:

- single occurrence -> scalar (unchanged behavior)
- repeated same key -> YAML Sequence
- zero occurrences -> key absent (no empty-array special-casing)

`parse_frontmatter_field` and the flag declaration stay byte-identical. All
logic lives in `build_frontmatter_bytes`. The consumer normalizes
scalar -> single-element list (acumen's job, out of scope here).

## Scope (3 repos)

1. cue CLI (this repo): `build_frontmatter_bytes` group-by-key + tests.
2. cue-plugins: tools accept `refs` param; skill mandates `refs:`.
3. cue.nvim: `core.add` emits list-valued frontmatter (no empty-array seeding —
   per "no flags -> no frontmatter value"; consumer normalizes absent refs).

Out of scope: cuelib `RawFrontmatter` parsing; `git_branch:` branch-init
script; acumen crate.

## Acceptance Criteria

| # | Criterion (outcome) | Verify by | Evidence |
|---|---|---|---|
| 1 | Repeated `-f key=v` produces a YAML list | `cargo test -p cue` | exit 0 (8b1483c) |
| 2 | Single `-f key=v` stays scalar (regression) | `cargo test -p cue` | exit 0 (8b1483c) |
| 3 | Scalar coercion + colon-protection still work | `cargo test -p cue` | exit 0 (8b1483c) |
| 4 | cue-plugins tools accept `refs` (default `[]`) | `bun run typecheck` | exit 0 / tsc --noEmit (baedf29) |
| 5 | cue skill mandates `refs:` on all artifacts | manual review | |
| 6 | cue.nvim emits refs via `core.add` | `luacheck` + manual | luacheck: exit 0 (8f430b6); nvim QA: |

## Branch

`feat/frontmatter-array-values` (cue). Coordinated branches in cue-plugins
and cue.nvim under the same name.
