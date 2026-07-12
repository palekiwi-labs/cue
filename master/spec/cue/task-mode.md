# Cue task mode

---

## Context

`cue` currently is tightly coupled to git branches: it names its directories in
`.cue/` after branches and by default writes files and logs into the directory
derived from current branch. This workflow has a number of disadvantages.

## Proposal

Introduce a new "task mode" that can be enabled by a new config setting:

```json
{
  "mode": "task"
}
```

The default value for "mode" will be "branch" so that we can support both modes,
at least during an initial evaluation period.

## Pillars of the task mode

- `task` as the central piece of context organization
- `task` artifacts continue to live on inside `.cue/master/task/`
- active task is selected with `cue switch <task-id>`
- active task can be queried with `cue status`
- the context of a `task` will live in `.cue/<task-id>/`
- context directory can be auto-created when running `cue switch <task-id>`
- `.cue/master/` remains as the global context
- global active task is saved in `.cue/HEAD` inspired by the the git convention
- `--task <task-id>` flag that allows selection of context similarly to
- new frontmatter on `task` artifacts:
  - `branch: []`: allows auto-selection of active task based on checked out
  branch (auto-switching can be done with `cue switch --branch`, user can use it
  in their git hooks, e.g. post-checkout)
  - `id`: identifies the task
- a new nvim plugin picker for selection of active task

## Open questions

- what `<task-id>` is in commands: `id` in the frontmatter or the task filepath?
- how will the task id be generated? a new flag `--id` passed to `cue add` that
generates some kind of id (UUID?) in a frontmatter field?
- should we support both `<task-id>` and `<task-filepath>` as args to `cue
switch`?
- what is the contents of `.cue/HEAD`: just `task-id` or also the title and
fillepath?
- parallel workflows and tracking of active task for git worktrees
