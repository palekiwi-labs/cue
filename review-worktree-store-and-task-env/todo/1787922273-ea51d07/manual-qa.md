---
status: open
priority: high
---
# Manual QA before merge

Run these checks from a disposable Git repository with the branch build of `cue` available.

## Main worktree initialization

- [x] Run `cue init` in a fresh Git repository and confirm `.cue/` is created.
- [x] Run `cue init` again and confirm it succeeds without damaging existing artifacts.
- [x] Run `cue status` and confirm it reports `master`, `default` provenance, and the main worktree store path.

## Interactive scope selection

- [x] Run `cue switch manual-qa` and confirm the local `.cue/HEAD` contains `manual-qa`.
- [x] Run `cue status` and confirm it reports `manual-qa` with `head` provenance.
- [x] Add a root note and confirm it is written under `.cue/manual-qa/note/`.
- [x] Run `cue switch master` and confirm status returns to the global context.

## Session scope and precedence

- [x] Set `CUE_TASK=session-qa`, run `cue status`, and confirm `session-qa` with `env` provenance.
- [x] With `CUE_TASK=session-qa`, run an add command with `--task flag-qa` and confirm the artifact is written under `.cue/flag-qa/`.
- [x] Confirm `cue status --task flag-qa` reports `flag` provenance.
- [x] Set `CUE_TASK` to an empty value and confirm scope falls back to `.cue/HEAD`.
- [x] Set `CUE_TASK=../../escape` and confirm scoped commands fail with an invalid task slug error and create nothing outside `.cue/`.
- [x] Manually write `../../escape` to `.cue/HEAD`, unset `CUE_TASK`, and confirm `cue status` fails with an invalid task slug error.

## Linked worktree behavior

- [x] Create a linked Git worktree and run `cue status` inside it.
- [x] Confirm status reports the main worktree’s `.cue/` as the shared store.
- [x] Run `cue switch linked-qa` inside the linked worktree and confirm only its local `.cue/HEAD` changes.
- [x] Confirm the main worktree’s `.cue/HEAD` remains unchanged.
- [x] Add a note and log entry from the linked worktree and confirm both are stored under the main worktree’s `.cue/linked-qa/` directory.
- [x] Confirm add and log output paths are store-relative rather than absolute.
- [x] Run `cue init` in the linked worktree and confirm it recognizes the existing main-worktree store rather than creating an independent store.

## Context commands

- [x] Run `cue context init --task manual-qa` and confirm it creates the expected scoped `context.json`.
- [x] Run `cue context show --task manual-qa` and confirm stdout remains valid JSON.
- [x] Run `cue context profiles --task manual-qa` and confirm profile names are listed.
- [x] Run `cue context path --task manual-qa` and confirm it prints the expected absolute path.
- [x] Run `cue context render --task manual-qa` and confirm stdout remains an artifact stream without status metadata mixed into it.

## Legacy removal and final smoke test

- [x] Run `cue link --help` and confirm `link` is no longer a recognized command.
- [x] Confirm normal `add`, `list`, `log add`, `log list`, `status`, and `context render` workflows still succeed.
- [x] Confirm no unexpected `.cue/STORE` files were created.
- [x] Record any failures or surprising output before deciding whether to merge.
