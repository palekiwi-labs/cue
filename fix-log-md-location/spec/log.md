# Project Log

## [e054afb] Log file moved from spec/ to branch root; skill doc updated

Addressed task master/task/1783061955-235ceeb/move-log-file-to-top-level.md. `cue log` now writes to `.cue/<branch>/log.md` at the branch root instead of `.cue/<branch>/spec/log.md`. Updated write (log/mod.rs:add_entry) and read (commands/log.rs list) paths plus all test expectations. Also fixed three stale test-literal failures (test_default_artifact_types, test_default_ignored_types, default_ignored_types) whose expected order predated commit 01f8f254's alphabetical sort of CANONICAL_TYPES/DEFAULT_IGNORED_TYPES. Finally updated the external cue SKILL.md (cue-plugins repo) root-artifacts list and spec/ hygiene note to reflect the new location. Existing log.md files in historical branches are intentionally left in place.

- **Found:** Sandbox LLD linker intermittently crashes with 'Resource temporarily unavailable' on thread spawn; worked around with RUSTFLAGS="-C link-arg=-Wl,--threads=1" --jobs 1
- **Found:** Three cuelib ordering tests were already failing before this task because test literals were never updated when 01f8f254 sorted the canonical type constants
- **Decided:** Log file lives at `<branch>/log.md` (branch root), not under a type directory, and is managed by `cue log` rather than `cue add --root`
- **Decided:** Did NOT migrate existing `.cue/*/spec/log.md` files per user instruction; historical branches left untouched
- **Decided:** Made two atomic commits in the cue repo (test-literal fix, then the log-path move) plus one docs commit in cue-plugins
- **Decided:** Left pre-existing repo-wide cargo fmt drift (curator/acuity/cue import sorting from newer rustfmt) untouched as out of scope
- **Open:** Repo-wide cargo fmt drift (new rustfmt import-sorting rules) affects curator, acuity, cue/cuelib imports -- not addressed here; may warrant a dedicated `style:` cleanup commit

## [e054afb] Updated cue.nvim open_log() to read log from branch root

Extended the log-path migration to the two consuming projects. The nvim plugin had a single reference at lua/cue/core.lua:118 in open_log() (the :CueLog command), now reading from .cue/<branch>/log.md. The external cue SKILL.md was already updated in a prior step. Verified via nix develop: luacheck passes 0 warnings/errors on core.lua. The stylua --check drift is a pre-existing project-wide issue (file uses 2-space indent but no .stylua.toml exists, so stylua defaults to tabs) — left untouched as out of scope; my line matches the file's existing 2-space style.

- **Decided:** nvim repo change was a single line in open_log(); committed as fix(core): open log from branch root, not spec/ (7e897eb) on master
- **Decided:** Did NOT reformat lua/cue/core.lua with stylua to avoid converting the whole file from 2-space to tabs against the project's actual convention; the missing .stylua.toml is a pre-existing config gap

