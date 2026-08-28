# Project Log

## [1dbd25b] Plan drafted: link --at + strict resolve_store; bug folded in

Created executive plan at .cue/cue-link-with-easier-api/plan/1784699509-1dbd25b/link-at-and-strict-store.md covering both slices. Closed bug-cue-status-with-no-cue-dir.md (folded in). Decisions above came out of an opus consultation whose load-bearing claims (global --dir collision, 4 duplicated guards, list.rs:1333 literal assertion) were all verified against the code before committing to the plan.

- **Found:** `.cue`-existence guard is duplicated in 4 handlers (add/mod.rs:36, list/mod.rs:145, log/mod.rs:51, switch.rs:52) and MISSING from status.rs and all 5 context subcommands — whole class of silent-success bugs.
- **Found:** crates/cue/tests/list.rs:1333 asserts the literal old error string; will need updating.
- **Found:** `link` never calls resolve_store — it validates its target itself (link.rs:8, link.rs:16-22), so the strict-store change must not be routed through link's target handling.
- **Decided:** Flag name is `--at` (not `--dir`: collides with global `-C/--dir` which is `global = true` at cli.rs:19 — a second `--dir` trips clap's duplicate-name debug assertion; not `--to`: fights the positional 'link to /abs/store').
- **Decided:** Source/destination args are orthogonal, NOT mutually exclusive. `store_path` = which store (default: discovered from cwd), `--at` = where the proxy goes (default: cwd). `cue link /abs/store --at ./wt/x` is valid.
- **Decided:** Store discovery stays git-root-anchored (no new ancestor walk). When linking, write `resolved.store_dir` into STORE — never the raw cue_dir — to avoid proxy chains that validate_store_target (store.rs:91-96) rejects.
- **Decided:** Status bug fixed at the source: make `resolve_store` strict (missing `.cue` = loud error), delete the 4 duplicated guards (add/mod.rs:36, list/mod.rs:145, log/mod.rs:51, switch.rs:52), attach a dynamic remedy hint at the CLI layer via a wrapper helper (cue link if `.git` is a file = linked worktree, else cue init).
- **Decided:** Sequencing: Slice 1 (bug) first — foundational; Slice 2 (`cue link --at`) builds on top.
- **Open:** Acceptance criteria table on cue-link-with-easier-api.md is minimal; will revisit during closeout (step 3.3) — may need to add explicit criteria for the --at behavior and the bug fix.

