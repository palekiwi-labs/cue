---
priority: normal
status: closed
title: Bug | Cue Status With No Cue Dir
refs:
  - .cue/master/task/cue-link-with-easier-api.md
---

`cue status` run in a worktree without `.cue` returns:

```
active context: master (global)
```

which then results in:

```bash
󰲒 cue add --type trace test.md ""
Error: .cue directory does not exist. Run `cue init` first.
```

## Closed

Folded into `cue-link-with-easier-api`. The fix is captured as Slice 1
(strict `resolve_store` + dynamic remedy hint) of that task's executive plan
`.cue/cue-link-with-easier-api/plan/<timestamp>-<hash>/link-at-and-strict-store.md`.
The fix is foundational to the `cue link --at` feature (both touch
`resolve_store`), so the two ship together.

