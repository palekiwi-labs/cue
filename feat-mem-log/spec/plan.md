# Feature Specification: `mem log`

## Overview
The `mem log` feature introduces two commands to manage a project's evolutionary history (`log.md`) natively via the `mem` CLI. It supports structured updates via `mem log add` (designed for both human and AI use) and fast context retrieval via `mem log list`.

The log acts as a durable, appending record of discoveries, decisions, and dead-ends, conceptually tied to the codebase's state at specific points in time.

## `mem log add`

Appends a new entry to the target branch's `spec/log.md`. If the file does not exist, it is auto-created with a `# Project Log` header.

### Entry Format
```markdown
## [abc1234-dirty] Title text here

Optional prose body paragraph sits directly below the header.

- **Found:** First observation
- **Found:** Second observation
- **Decided:** Decision made
- **Open:** Question remaining
```
- **Header Hash**: Uses the short HEAD hash of the *project branch* (not the `mem` orphan branch). If the working tree has uncommitted changes, the `-dirty` suffix is appended.
- **Ordering**: Entries are appended to the end of the file (oldest at top, newest at bottom).
- **Separation**: Entries are delineated purely by the `##` header. No horizontal rules (`---`) are used.
- **Rendering**: Missing fields are omitted entirely. Structured fields use markdown bullet points (`- **Label:** Value`).

### CLI Flags
- `--title <text>`: Required (max length 120 chars).
- `--body <text>`: Optional single value.
- `--found <text>`: Optional, repeatable.
- `--decided <text>`: Optional, repeatable.
- `--open <text>`: Optional, repeatable.
- `--file <path>`: Optional, single. Takes a path to a JSON file.

### Validation Rules
- `--title` is the only required field. An entry with just a title is valid.
- `--file` is mutually exclusive with all other content flags (`--title`, `--body`, `--found`, etc.).
- String values must be trimmed; whitespace-only strings should be rejected or ignored.
- The command only mutates the file; it does **not** auto-commit to the orphan branch.

### JSON Schema (for `--file`)
Designed for AI agents to bypass shell escaping issues.
```json
{
  "title": "string (required)",
  "body": "string (optional)",
  "found": ["string (optional)"],
  "decided": ["string (optional)"],
  "open": ["string (optional)"]
}
```

## `mem log list`

Retrieves the log entries for human or AI consumption.

### Behavior
- Prints the raw markdown content of `log.md` to `stdout`.
- If `log.md` does not exist for the target branch, exits gracefully with code `0` and prints nothing (to prevent pipeline/agent noise).

### CLI Flags
- `--branch <branch-name>`: Targets a specific branch's log instead of the current project branch.

### Future Enhancements (Deferred)
- `--json (-j)` flag for `mem log list`: Parses the markdown AST and outputs a structured JSON array of entries. Valuable for editor plugins (Neovim/Telescope) or CLI tools (`jq`, `fzf`), but excluded from the current scope.