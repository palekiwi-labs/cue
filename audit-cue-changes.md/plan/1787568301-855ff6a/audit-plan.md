---
status: in-progress
---
# Audit Plan: cue Feature Changes

## Phase 1: Scouting & Surface Discovery
- [ ] List contents of all target surfaces to understand structure.
- [ ] Identify search patterns for each category (A-F).

## Phase 2: Surface 1 - cue.nvim
- [ ] Scan for `cue link`, `STORE`, `HEAD`.
- [ ] Analyze store resolution logic in Lua.
- [ ] Check env var handling ($CUE_TASK, etc.).

## Phase 3: Surface 2 - cue-plugins
- [ ] Scan for `cue link`, `STORE`, `HEAD`.
- [ ] Analyze store resolution logic in TS/JS.
- [ ] Check env var handling.

## Phase 4: Surface 3 - cue skill
- [ ] Scan SKILL.md and reference/ files for all categories.

## Phase 5: Surface 4 - cue repo (docs/non-store)
- [ ] Scan README, docs/, AGENTS.md.
- [ ] Scan crates' doc comments (using grep on .rs files but excluding code logic if possible, focusing on comments).

## Phase 6: Synthesis & Reporting
- [ ] Categorize findings (BREAK, DEGRADE, UNAFFECTED).
- [ ] Generate the final structured report.
