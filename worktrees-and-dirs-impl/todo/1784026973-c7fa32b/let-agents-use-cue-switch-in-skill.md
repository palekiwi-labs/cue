---
title: Let agents use `cue switch` in skill
priority: normal
status: complete
---

Current "cue" skill prohibits agents from using `cue switch` command but with
the worktrees support, setting the context for woktrees will be a fundamental
need. We need to adapt the skill.

## Resolution

Dropped the prohibition entirely (Option B). The skill's premise that agents
share `.cue/HEAD` is now obsolete under worktree isolation: each proxy
worktree has its own local HEAD, so an agent calling `cue switch` cannot
clobber the user's or sibling agents' context.

Done in `cue-plugins` repo (branch `feat/task-mode-cli`):
`skills/cue/SKILL.md` — removed rule 4 ("Never call `cue switch`"),
renumbered the handoff rule to 4, and replaced the stale "shared HEAD"
premise with: "The active context is pinned by `.cue/HEAD`. Be deliberate
about which context you write to."
