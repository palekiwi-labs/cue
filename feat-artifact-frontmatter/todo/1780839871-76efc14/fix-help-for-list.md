---
status: done
---

current output:

```
List artifacts for a branch

Usage: mem list [OPTIONS]

Options:
      --branch <BRANCH>     List files for a specific branch instead of current
  -a, --all                 List files for all branches
  -t, --type <MEM_TYPE>     Filter by artifact type
  -i, --include-gitignored  Include ignored artifact types (e.g. tmp)
  -j, --json                Output as JSON
      --frontmatter         Parse and include YAML frontmatter in output (implies --json)
      --filter <EXPR>       Filter by frontmatter field (repeatable, ANDed). Syntax: KEY[OP]VALUE where OP is =, !=, or ~= (substring). Dot notation for nested keys: meta.status=done Examples: --filter "status!=done"  --filter "priority=high"
  -h, --help                Print help
```

PLAN reference:
/home/pl/code/palekiwi-labs/mem/.mem/feat-artifact-frontmatter/plan/1780571840-a37c462/frontmatter-filtering.md L56-L57
