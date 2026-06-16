---
status: complete
priority: 1
---
# GitHub: rename repository to `cue`

Rename the remote repository from `palekiwi-labs/mem` to `palekiwi-labs/cue`.

## Steps

1. Run the rename via `gh`:

```bash
gh repo rename cue --repo palekiwi-labs/mem
```

2. Update the local remote URL to match:

```bash
git remote set-url origin git@github.com:palekiwi-labs/cue
```

3. Verify:

```bash
git remote -v
```

Expected output:
```
origin  git@github.com:palekiwi-labs/cue (fetch)
origin  git@github.com:palekiwi-labs/cue (push)
```

## Notes

- GitHub will redirect the old URL automatically, but the local remote should be updated explicitly.
- Do this after all source code changes are merged to avoid confusion.
