# Project Log

## [173152e-dirty] Spec rewritten for cueban feature branch

Rewrote .cue/feat-cueban/spec/index.md to reflect the full design discussion. Previous spec was a rough initial idea; new spec is authoritative and covers all settled decisions.

- **Decided:** Cargo workspace with three crates: cue, cue-lib, cueban
- **Decided:** cue-lib is the shared library — config, git, artifact discovery, project registry
- **Decided:** Canonical artifact types: spec plan trace doc todo bin tmp ref (8 types)
- **Decided:** Default ignored types: tmp bin
- **Decided:** Canonical todo statuses: open in-progress complete closed archived; only first three shown in kanban
- **Decided:** Project registry at ~/.local/share/cue/projects.json, key maps to Vec<PathBuf> to support multiple checkouts and worktrees
- **Decided:** Project keys: github:org/repo for GitHub remotes, local:<dir-name> fallback
- **Decided:** CUE_DATA_DIR env var controls store path for test isolation
- **Decided:** cue init registers project in store (idempotent)
- **Decided:** cue project add/remove/remove --key/list subcommands
- **Decided:** archived/closed todos silently hidden in cueban (no clutter)
- **Decided:** cue project remove by path (cwd default); --key removes all paths for a key
- **Decided:** BTreeMap for project store (no extra dep, alphabetical order fine)
- **Decided:** cueban cyclic project filter via Tab: All -> key-A -> key-B -> All
- **Decided:** Card shows title on line 1, project-key and branch on line 2
- **Decided:** cueban --type flag kept as forward-compat hook (default: todo)
- **Decided:** TDD throughout: 8 implementation slices, each starts with failing tests

