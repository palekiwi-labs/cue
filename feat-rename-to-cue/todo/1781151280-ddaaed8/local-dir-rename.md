---
status: complete
priority: 2
---
# Local: rename checkout directory to `cue`

Rename the local project directory from `mem` to `cue` to match the new project name.

## Steps

```bash
mv /home/pl/code/palekiwi-labs/mem /home/pl/code/palekiwi-labs/cue
```

## Notes

- This is cosmetic only — no source code depends on the directory name.
- Do this last, after the GitHub rename (Slice 7) and all code changes are done.
- Any shell bookmarks, aliases, or editor workspace files pointing to the old path will need updating.
