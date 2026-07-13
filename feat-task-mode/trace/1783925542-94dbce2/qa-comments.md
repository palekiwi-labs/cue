---
status: open
refs:
  - .cue/feat-task-mode/todo/1783925542-94dbce2/manual-qa-task-mode.md
---

# `cue status` 

command confirmed to work

the command displays: "active context: master (global)" which means we cannot
use the output programmatically to retrieve current context

Consider adding a `--json` flag for structured output

# `cue switch` 

command confirmed to work
- with both slug and path (relative or absolute)
- with `master`

Same issue about structured output as above.
