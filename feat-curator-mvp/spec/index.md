# curator MVP — Branch Spec

## Implements

`task/1781965432-d2f3251/curator-artifact-kanban.md`

## Scope

Build the `curator` TUI (Phase 2 of the cue ecosystem roadmap).
Scope is deliberately narrow — CWD-only project, no multi-project registry,
no `acuity` involvement.

## Constraints

- **CWD only**: read `.cue/master/task/` (and plan/, todo/) from the current
  working directory. `ProjectStore` is not consulted in this phase.
- **Read-only**: no mutations to `.cue/` artifacts.
- **No acuity**: the live activity half is Phase 6.
- **`cue` CLI tests must not regress**: `cargo test -p cue` must stay green.

## Prerequisites

- `feat/curator-mvp` branch checked out.
- `cuelib` extended with a typed artifact reader (Stream 1 below).

## Deliverables

1. `cuelib`: `extract_frontmatter_yaml` + `collect_files` migrated from
   the `cue` crate into `cuelib`; a thin typed `ArtifactMeta` reader added.
2. `curator`: minimal Ratatui TUI — three-column kanban
   (Open | In Progress | Complete) populated with tasks from CWD's
   `.cue/master/task/`.
3. All existing `cue` CLI tests green.
