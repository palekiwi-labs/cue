---
title: Implement worktree context isolation
status: complete
priority: critical
---
# Implement worktree context isolation

Implement the `STORE` file redirect mechanism and the `cue link` command as
specified in spec/index.md. This enables multiple agents running in separate
git worktrees to share one `.cue/` artifact store while each maintaining its
own isolated active context via a local `HEAD` file.

## Source

- spec/index.md — full specification for this feature
- Parent task: `worktrees-and-dirs` in the `ai` coordination workspace

## Acceptance Criteria

| # | Criterion | Verify by | Evidence |
|---|-----------|-----------|----------|
| 1 | `ResolvedStore` struct and resolution helper exist in `cuelib` | code review | confirmed e70eece |
| 2 | All command call sites refactored to use `{head_dir, store_dir}` | code review | confirmed e70eece |
| 3 | `cue link <store-path> [--task <slug>]` command exists and works | `cue link --help` + integration test | 10 integration tests pass |
| 4 | STORE redirect: artifact I/O goes to target, HEAD stays local | integration test | proxy_reads.rs, switch_in_proxy |
| 5 | Nested STORE errors loudly | integration test | chained_store_target_errors in store.rs |
| 6 | Invalid/missing STORE target errors loudly | integration test | link integration tests |
| 7 | `cue switch` in a proxy worktree writes HEAD locally, creates scope dir in target | integration test | switch_in_proxy_writes_head_locally_scope_dir_in_store |
| 8 | All existing tests pass | `cargo test` | all pass e70eece |
| 9 | Manual verification: full orchestrator workflow works end-to-end | human attestation | passed — trace/1784609144-e70eece/Manual QA.md |
