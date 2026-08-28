---
status: complete
priority: high
refs:
- .cue/master/task/worktrees-and-dirs-impl.md
- .cue/worktrees-and-dirs-impl/spec/index.md
---
# Manual QA: Worktree Context Isolation

Acceptance criterion 9 of task `worktrees-and-dirs-impl` requires human
attestation of the full end-to-end orchestrator workflow. The integration
tests cover the STORE redirect mechanics with simulated git repos, but the
real `git worktree add` + `cue link` flow with actual cast/container
mounting has only been verified at the unit level. Run through this
checklist before merging.

## Prerequisites

- `.gitignore` in the main project contains `worktrees/`
- The main project's `.cue/` store exists and has a `master/` subdir

## 1. Proxy worktree setup (single agent)

- [x] `git worktree add worktrees/qa-impl -b qa-impl` from the main project
- [x] `cd worktrees/qa-impl`
- [x] `cue link <abs-path-to-main-project>/.cue --task qa-impl`
- [x] Verify `worktrees/qa-impl/.cue/STORE` exists and contains the
      canonicalized absolute path to the main `.cue/`
- [x] Verify `worktrees/qa-impl/.cue/HEAD` contains `qa-impl`
- [x] Verify NO `master/` directory was created inside the proxy `.cue/`
      (it should contain only `STORE` and `HEAD`)

## 2. Context reads correctly in the proxy

- [x] `cue status` from inside the proxy worktree prints `qa-impl` as the
      active task (reads local HEAD)
- [x] `cue status --json` returns `{"context":"qa-impl","global":false,...}`
- [x] Add `.cue/` to the worktree's branch `.gitignore`; confirm `git status`
      no longer shows the proxy `.cue/` as untracked

## 3. Artifact writes land in the shared store

- [x] `cue log add` from inside the proxy worktree (write a test entry)
- [x] Verify the entry appears at `<main-project>/.cue/qa-impl/log.md`
      (the shared store), NOT at `worktrees/qa-impl/.cue/qa-impl/log.md`
- [x] `cue add --type note test.md` from inside the proxy; verify the file
      lands under `<main-project>/.cue/qa-impl/note/`
- [x] From the MAIN worktree, `cue list --all` shows the entries written
      by the proxy agent

## 4. Multi-worktree isolation

- [ ] Set up a SECOND worktree: `git worktree add worktrees/qa-review -b qa-review`
- [x] `cue link <abs-path>/.cue --task qa-review` from the second worktree
- [x] `cue log add` from worktree A (`qa-impl`) and worktree B (`qa-review`)
- [x] Verify A's log entry is at `.cue/qa-impl/log.md` and B's is at
      `.cue/qa-review/log.md` — no cross-contamination
- [x] Verify changing HEAD in one worktree does not affect the other
      (each has an independent local `.cue/HEAD`)

## 5. cue switch inside a proxy worktree

- [x] From inside a proxy worktree, run `cue switch <other-slug>`
- [x] Verify `worktrees/<dir>/.cue/HEAD` updated to the new slug
- [x] Verify the scope directory was created under the SHARED store
      (`<main-project>/.cue/<other-slug>/`), not under the local proxy

## 6. Orchestrator compose step

- [x] From the main worktree, run `cue list --all` and confirm both agents'
      artifacts (logs, notes) are visible in a single view
- [x] Confirm the orchestrator can read agent outputs without entering any
      worktree

## 7. Cast mount boundary (if using cast)

- [x] Confirm the worktree path `<main-project>/worktrees/<branch>` is
      reachable inside the container (nested, not a sibling) so the
      in/out mount paths match for the STORE absolute path

## Notes

- All of the above assume the main `.cue/` store's absolute path is stable
  and identical inside/outside any container mount (operational requirement
  on cast: mount at identical inside/outside path).
- These steps satisfy acceptance criterion 9. Record the result (pass/fail)
  and any deviations in the task card's Evidence field.
