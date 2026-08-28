# Project Log

## [e798914] cuelib TaskStatus::Inbox implemented; push blocked by sandbox creds

Implemented the cuelib half of this card (point 1 + curator
explicit handling) as commit eac5be3 on branch
feat/formalize-inbox-status, worktree
worktrees/feat-formalize-inbox-status. TDD: RED tests first
(task_status_inbox_round_trip, inbox not kanban-visible, inbox
task-only), then the enum variant.

Scope kept minimal per operator request (own PR):
- crates/cuelib/src/artifact.rs: TaskStatus::Inbox (first variant,
  doc-commented as intake state), FromStr/as_str/is_kanban_visible
  (false) updated; tests: round-trip, visibility, task-only
  (TodoStatus/NoteStatus still reject inbox).
- crates/curator/src/app.rs: classify_tasks match gained explicit
  Some(TaskStatus::Inbox) exclusion arm (compiler forced it);
  new test closed_and_inbox_tasks_are_excluded_from_kanban pins
  the behavior that was previously accidental (unparseable ->
  None -> excluded); doc comments updated.

Validation: cargo test --workspace all green (cuelib 73, curator
128), clippy clean, rustfmt adds zero new violations in touched
files (repo-wide pre-existing drift confirmed at HEAD and left
out of scope).

NOT pushed: this cast sandbox has no ssh agent and the fine-grained
GH_TOKEN is denied Contents-write (git push 403, blobs API 403).
Branch awaits host-side push + PR creation.

Remaining on this card after this lands: point 2 CLI default
(cue add --type task -> status inbox), point 5 cue README/CLI
help mention, nvim triage/promote action. Plugin-side defaults
and skill docs already shipped from the palekiwi workspace
(cue-plugins 07d8df9/949104e/acefb4c, cue.nvim cad669e).

- **Decided:** Inbox placed as first TaskStatus variant with doc comment; kanban-invisible
- **Decided:** Curator exclusion made explicit rather than relying on unparseable->None
- **Decided:** Kept CLI default and docs rollout out of this PR (card points 2/5)
- **Open:** Push branch and open PR from a host-side session (no push creds in sandbox)
- **Open:** cue add task default to inbox (card point 2)
- **Open:** cue repo README/CLI help status list (card point 5)

