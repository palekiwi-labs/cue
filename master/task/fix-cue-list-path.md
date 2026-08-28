---
status: complete
priority: critical
--- 
Following the merge of `2c235a7` PR, `cue list` displays paths that can no
longer by followed because they are now relative to `.cue`.

Currently the path printed by `cue list` is: `master/task/fix-cue-list-path.md`
which means it cannot be reliably followed and processed by programs.

The most reliable solution would be to print absolute paths:
`/home/pl/code/palekiwi-labs/cue/.cue/master/task/fix-cue-list-path.md`
