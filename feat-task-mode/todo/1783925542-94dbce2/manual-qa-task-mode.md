---
status: open
priority: normal
refs:
- crates/cue/src/commands/switch.rs
- crates/cue/src/commands/status.rs
- .cue/feat-task-mode/plan/1783877957-c260da5/phase-2-head-scope.md
---
# Manual QA: task mode subcommands

Smoke-test the new HEAD-driven scope, `cue switch`, `cue status`, and `--task`
flag introduced in `feat/task-mode`.

Build first:

```
cargo build
alias cue=./target/debug/cue
```

---

## cue status (no HEAD file)

```
cue status
```

Expected: `active context: master (global)`

### Comments

command confirmed to work

the command displays: "active context: master (global)" which means we cannot
use the output programmatically to retrieve current context

Consider adding a `--json` flag for structured output


---

## cue switch to a task slug

```
cue switch my-task
```

Expected: `switched to task: my-task`

Verify `.cue/HEAD` contains `my-task` and `.cue/my-task/` directory was created.

### Comments

command confirmed to work
- with both slug and path (relative or absolute)
- with `master`

Same issue about structured output as above.

---

## cue status after switch

```
cue status
```

Expected:
```
active task: my-task
  context: .cue/my-task/
```

(No title/status lines since there is no task card for `my-task`.)

### Comments

works

---

## cue switch with a filepath

```
cue switch .cue/master/task/some-card.md
```

Expected: `switched to task: some-card`

### Comments

works

---

## cue switch to master (return to global context)

```
cue switch master
```

Expected: `switched to global context`

Then `cue status` should print `active context: master (global)`.

### Comments

works

---

## --task flag overrides scope without touching HEAD

```
cue switch my-task          # set HEAD to my-task
cue add --type spec --filename test.md --task other-task <<< "hello"
```

Expected: artifact written to `.cue/other-task/spec/...`, not `.cue/my-task/`.
`.cue/HEAD` must still contain `my-task` afterwards.

### Comments

works

---

## cue log add respects HEAD scope

```
cue switch my-task
cue log add --title "Test entry"
```

Expected output path: `.cue/my-task/log.md`

### Comments

works

---

## cue log list reads from HEAD scope

```
cue log list
```

Expected: prints contents of `.cue/my-task/log.md` including `# Project Log`
and `Test entry`.

### Comments

works

---

## cue log add --task overrides scope

```
cue switch master
cue log add --title "Other entry" --task my-task
```

Expected output path: `.cue/my-task/log.md`
`.cue/HEAD` must still contain `master` afterwards.

---

## cue switch --branch (with a matching task card)

Requires a task card at `.cue/master/task/<slug>.md` with a `branch:` list
containing the current git branch name.

```
# Example frontmatter in .cue/master/task/feat-task-mode.md:
# branch:
#   - feat/task-mode

cue switch --branch
```

Expected: `switched to task: feat-task-mode`

If no card matches the current branch:

Expected: `no task matched branch: <branch>` then `switched to global context`

### Comments

It works when the frontmatter `branch` contains a single branch (string value).
It does not work when `branch` is an array of branches

---

## Extra findings

```bash
$ cue -- log list --task test-task1

error: unexpected argument '--task' found

Usage: cue log list [OPTIONS]
```

we need to be able to list log entries from other task contexts by specifying
`--task`
