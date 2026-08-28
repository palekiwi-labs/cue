---
title: Formalize inbox status; default new tasks to it
status: open
priority: normal
tag: 0.2.0
kind: build
---

# Problem

`status: inbox` is used informally across stores (cast, cue, palekiwi)
for captured-but-untriaged tasks, but it is outside the documented
enum (`open|in-progress|complete|closed`). Meanwhile every newly
created task defaults to `open`, which puts fresh captures directly
into the active/in-progress views of the nvim task picker and clutters
orientation.

Semantics being formalized (GTD-style two-stage funnel): creation is
capture (inbox, cheap, non-committal); commitment is an explicit
promotion to open. Inbox items are invisible in active views until
triaged.

# Proposal

1. Accept `inbox` as a formal task status in cuelib validation and all
   documentation.
2. Default task creation to `status: inbox`:
   - `cue` CLI `add` (task type),
   - cue-plugins harness tools (`cue-task`, `cue-add` for tasks).
   Promotion to open is an explicit act during triage.
3. Active-view filters everywhere treat inbox as not-active
   (`open|in-progress` = active; `inbox` = awaiting triage).
4. cue.nvim: task picker active view excludes inbox; add an inbox
   triage view or action (promote to open / close) — exact UX to be
   settled in implementation.
5. Docs rollout: cue skill enum, CLI help, README.

# Scope hints

- cuelib: status enum/validation site.
- crates/cue: `add` command defaults.
- cue-plugins: tool default status.
- cue.nvim: picker filtering + triage action.
- No migration needed: existing informal inbox cards become valid
  as-is.

# Progress (2026-08-26)

- Point 1 DONE (local commit eac5be3, branch
  `feat/formalize-inbox-status`, worktree
  `worktrees/feat-formalize-inbox-status`): `TaskStatus::Inbox`
  in cuelib (kanban-invisible, task-only), curator exclusion made
  explicit + tested. Awaiting host-side push (sandbox lacks push
  credentials) and PR.
- Point 2 (cue-plugins tool default) and point 5 (skill docs) DONE
  from the palekiwi workspace: cue-plugins `cue-task` defaults to
  inbox (07d8df9), init templates ask for status (949104e), cue
  skill documents inbox (acefb4c), cue.nvim slug-flow defaults to
  inbox (cad669e).
- Remaining: `cue add` CLI task default (point 2, CLI half), cue
  README/CLI help (point 5), nvim promote/triage action (point 4).

# Release candidacy

Candidate for 0.2.0 (pending operator decision); interacts with the
release only trivially (enum + defaults), but the cue.nvim triage UX
may land independently of the tag.
