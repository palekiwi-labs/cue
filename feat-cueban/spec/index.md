# Curtain

`curtain` is a TUI kanban dashboard for `cue` artifacts, built as a separate
crate in the `cue` workspace.

## Purpose

The `cue` framework stores artifacts (specs, plans, todos, logs) as plain files
in a git worktree. `curtain` makes those artifacts actionable by presenting them
as a kanban board that spans one or many projects simultaneously.

The initial focus is `todo` artifacts, because these map most naturally to a
kanban layout. Support for other artifact types (e.g. `plan`) is not ruled out
and the architecture should not preclude it.

## Workspace structure

`curtain` lives alongside `cue` and `cuelib` in a Cargo workspace:

```
cue/           (workspace root)
  cue/         (CLI binary)
  cuelib/      (shared library)
  curtain/     (TUI binary)
```

`cuelib` contains all shared logic: config loading, git utilities, artifact
discovery, frontmatter parsing, and the project registry. Both `cue` and
`curtain` depend on `cuelib`; they do not depend on each other.

`acuity` is a future planned workspace member that will act as an observability
hub, feeding live notifications to `curtain`.

## Artifact types and status vocabulary

The canonical set of artifact types supported out of the box:

```
spec  plan  trace  doc  todo  bin  tmp  ref
```

Default ignored types (not listed or committed by default): `tmp`, `bin`.

The canonical todo status values, in kanban column order:

```
open  →  in-progress  →  complete
```

Additional statuses (`closed`, `archived`) are valid in frontmatter but are
silently hidden in the kanban view — they do not appear in any column.

## Project registry

`cue` needs a way to discover todos across multiple projects. The registry is a
JSON file stored at:

```
~/.local/share/cue/projects.json
```

Overridable via the `CUE_DATA_DIR` environment variable.

### Format

Keys are project identifiers derived from the git remote:

- `github:org/repo` for GitHub projects (HTTPS or SSH remote)
- `local:<dir-name>` as a fallback when no remote is configured

Values are **arrays of filesystem paths**, because the same repository may be
checked out in multiple locations or used with git worktrees:

```json
{
  "github:org/repo": ["/path/to/main", "/path/to/worktree-a"]
}
```

### Registration via `cue init`

`cue init` is idempotent. After setting up the cue worktree it registers the
current project in the store. Running `cue init` a second time on the same path
is a no-op for both the worktree and the registry entry.

### `cue project` subcommand

Project registration can also be managed explicitly:

- `cue project add [path]` — registers a path (defaults to cwd). Idempotent.
- `cue project remove [path]` — removes a single path entry (defaults to cwd).
  If it was the last path for that key, the key is also removed.
- `cue project remove --key <key>` — removes all paths for a key.
- `cue project list` — lists all registered projects and their paths.

## `curtain` — kanban board

### Data model

Each card on the board represents one `todo` artifact and carries:

- **title** — from frontmatter; falls back to filename if absent
- **status** — from frontmatter; determines which column the card appears in
- **priority** — optional integer from frontmatter
- **project key** — e.g. `github:org/repo`
- **branch** — the cue branch the artifact belongs to (e.g. `main`, `feat-x`)
- **path** — absolute path to the file on disk

### Board layout

Three fixed columns:

```
 open          in-progress       complete
──────────    ──────────────    ──────────
 Card          Card              Card
 Card                            Card
```

Each card displays two lines:

```
┌──────────────────────────────┐
│ Fix login bug                │
│ github:org/repo  @feat-auth  │
└──────────────────────────────┘
```

The project key and branch are always visible on the card so the user can
distinguish the origin of each item when viewing across multiple projects.

### Project filter

Rather than a binary toggle between current and global, `curtain` provides a
**cyclic project filter**. Pressing `Tab` cycles through:

```
All projects → project-key-A → project-key-B → … → All projects
```

The active filter is shown in the status bar.

### Key bindings

| Key | Action |
|-----|--------|
| `h` / `Left` | Previous column |
| `l` / `Right` | Next column |
| `k` / `Up` | Move up within column |
| `j` / `Down` | Move down within column |
| `Tab` | Cycle project filter |
| `e` | Open selected artifact in `$EDITOR` |
| `q` / `Esc` | Quit |

`curtain` is not an editor. The `e` binding hands off to the user's `$EDITOR`
(falling back to `vi`) so that artifact bodies can be updated without leaving
the workflow.

### CLI

```
curtain [--all | --path <path>] [--type <type>]
```

- Default: loads the project rooted at the current working directory.
- `--all`: loads all projects registered in the project store.
- `--path <path>`: loads a specific project directory.
- `--type <type>`: artifact type to display (default: `todo`).

## Implementation approach

Development follows TDD: each unit of work starts with failing tests and is
driven to green before moving on. The slices in order are:

0. Convert to Cargo workspace (no new tests; all existing tests must stay green)
1. `cuelib`: status constants and canonical artifact types
2. `cuelib`: extract `config`, `git`, `artifact` modules from `cue`
3. `cuelib`: project registry (`ProjectStore`, `derive_project_key`)
4. `cue`: `cue project add/remove/list` subcommands
5. `cue`: `cue init` registers the project in the store
6. `curtain`: data loading (`load_todos`, `group_by_kanban_status`)
7. `curtain`: `App` state logic (filter cycling, column visibility)
8. `curtain`: TUI rendering with ratatui
