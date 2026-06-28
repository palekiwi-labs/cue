---
status: closed
title: should notes be root level?
---
I am wondering if the benefits of nesting the artifact under
`<timestamp>-<hash>` directory makes sense for the note
artifacts. The main benefits that I imagined for nesting was:

1. no file name collisions - especially important for reports like `rspec-output.log`
2. anchoring to the codebase state

For the notes, none of these benefits has real value. On the other hands,
nesting the notes prevents us from easily organizing the notes with
subdirectories such as:

```
.cue/master/notes/my-idea-a/index.md
.cue/master/notes/my-idea-a/references.md
.cue/master/notes/my-idea-a/follow-up.md
```

This would make the note artifact lend itself to using it as a replacement
for external note systems that could live inside the cue framework
