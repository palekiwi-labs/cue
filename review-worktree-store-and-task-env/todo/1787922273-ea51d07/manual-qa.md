---
status: open
priority: high
---
# Manual QA before merge

Run these checks from a disposable Git repository with the branch build of `cue` available.

## Main worktree initialization

- [ ] Run `cue init` in a fresh Git repository and confirm `.cue/` is created.
- [ ] Run `cue init` again and confirm it succeeds without damaging existing artifacts.
- [ ] Run `cue status` and confirm it reports `master`, `default` provenance, and the main worktree store path.

## Interactive scope selection

- [ ] Run `cue switch manual-qa` and confirm the local `.cue/HEAD` contains `manual-qa`.
- [ ] Run `cue status` and confirm it reports `manual-qa` with `head` provenance.
- [ ] Add a root note and confirm it is written under `.cue/manual-qa/note/`.
- [ ] Run `cue switch master` and confirm status returns to the global context.

## Session scope and precedence

- [ ] Set `CUE_TASK=session-qa`, run `cue status`, and confirm `session-qa` with `env` provenance.
- [ ] With `CUE_TASK=session-qa`, run an add command with `--task flag-qa` and confirm the artifact is written under `.cue/flag-qa/`.
- [ ] Confirm `cue status --task flag-qa` reports `flag` provenance.
- [ ] Set `CUE_TASK` to an empty value and confirm scope falls back to `.cue/HEAD`.
- [ ] Set `CUE_TASK=../../escape` and confirm scoped commands fail with an invalid task slug error and create nothing outside `.cue/`.
- [ ] Manually write `../../escape` to `.cue/HEAD`, unset `CUE_TASK`, and confirm `cue status` fails with an invalid task slug error.

## Linked worktree behavior

- [ ] Create a linked Git worktree and run `cue status` inside it.
- [ ] Confirm status reports the main worktree’s `.cue/` as the shared store.
- [ ] Run `cue switch linked-qa` inside the linked worktree and confirm only its local `.cue/HEAD` changes.
- [ ] Confirm the main worktree’s `.cue/HEAD` remains unchanged.
- [ ] Add a note and log entry from the linked worktree and confirm both are stored under the main worktree’s `.cue/linked-qa/` directory.
- [ ] Confirm add and log output paths are store-relative rather than absolute.
- [ ] Run `cue init` in the linked worktree and confirm it recognizes the existing main-worktree store rather than creating an independent store.

## Context commands

- [ ] Run `cue context init --task manual-qa` and confirm it creates the expected scoped `context.json`.
- [ ] Run `cue context show --task manual-qa` and confirm stdout remains valid JSON.
- [ ] Run `cue context profiles --task manual-qa` and confirm profile names are listed.
- [ ] Run `cue context path --task manual-qa` and confirm it prints the expected absolute path.
- [ ] Run `cue context render --task manual-qa` and confirm stdout remains an artifact stream without status metadata mixed into it.

## Legacy removal and final smoke test

- [ ] Run `cue link --help` and confirm `link` is no longer a recognized command.
- [ ] Confirm normal `add`, `list`, `log add`, `log list`, `status`, and `context render` workflows still succeed.
- [ ] Confirm no unexpected `.cue/STORE` files were created.
- [ ] Record any failures or surprising output before deciding whether to merge.
