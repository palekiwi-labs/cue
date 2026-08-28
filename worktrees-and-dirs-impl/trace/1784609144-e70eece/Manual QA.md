---
refs:
- .cue/worktrees-and-dirs-impl/todo/1784209310-72b98a4/manual-qa-worktree-isolation.md
---

## Error when running `cue link` twice

```nu
󰲒 cargo run -p cue -- link /home/pl/code/palekiwi-labs/cue/.cue --task qa-impl
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.09s
     Running `target/debug/cue link /home/pl/code/palekiwi-labs/cue/.cue --task qa-impl`
Error: .cue/ already exists in /home/pl/code/palekiwi-labs/cue/worktrees/qa-impl: remove it first to re-link
```

## all the QA steps pass
