# mem

---

## Project purpose

### Rewrite in Rust

The first purpose of this project is a complete Rust rewrite of one of my existing
experimental applications called `mem` which I wrote in `nushell`.
We are currently using `mem` to manage our context as files.
The original `mem` is available in your environment.

<important>

**Important** We do not intend to rewrite it 1:1 with the original `mem`.
The original can serve as general context to understand the purpos of the
project but we want to focus specifically on the features on the features
outlined in this document. This document will also be iteratively and
progressively modified and expanded.

</important>

## Features and API

We plan to implement the following:

### `mem init`

Initializes the project.

Checks whether:
- current project is a git repo?
- <dir-name> (default `.mem`) already exists and is a subtree with <branch-name> (default `mem`)?
  Exits if already initialized or if directory conflicts.
- <branch-name> is already present on remote? Pulls from remote if exits.

`init` should result in the following state:
- <dir-name> is created as a subdirectory of current project
- an orphan branch named <branch-name> is created
- a git worktree is checked out with <branch-name> to <dir-name>
- `.gitignore` and `.rgignore` are created in <branch-name> with default contents

Contents of `.gitignore`:
```
*/tmp/
*/ref/
```

Contents of `.rgignore`:
```
!*/tmp/
!*/ref/
```

### `mem add <category> <filepath> [flags] [content]` 

Arguments and flags:
- `filepath`: required, can include '/' to automaticaly create subdirs
- `category`: required, "spec" | "trace" | "tmp" | "bin" | "ref" | "doc"
- `content`: optional, defaults to stdin
- flags:
  - `--branch <branch-name>` use the provided branch name

Creates files in `<dir-name>/<current-branch-name>/<category>/` according to the following rules:

- category "spec" or "ref":
  `<dir-name>/<current-branch-name>/<category>/<filepath>`

- category "trace" or "tmp":
  `<dir-name>/<current-branch-name>/<category>/<commit-timestamp>-<commit-hash>/<filepath>` where:
    <commit-timestamp> and <commit-hash> refer to current commit on the project branch, not `mem` orphan branch

### `mem list`

Prints a list of files in `<dir-name>/<current-branch-name>/` relative to project root.
By default, excludedes files from gitignored categories.

Flags:
- `--branch <branch-name>` list files for following branch
- `--all(-a)` list files for all branches
- `--type(-t) <category>` filter by category
- `--include-gitignored(-i)`: prints in the following example format:
- `--json(-j)`: prints in the following example format:
```json
[
  {
    "path": ".mem/master/spec/index.md",
    "name": "index.md",
    "branch": "master",
    "category": "spec",
    "hash": null,
    "commit_hash": "ebe70e4",
    "commit_timestamp": 1775965227
  }
]
```

### `mem log`

Operations on `<dir-name>/<current-branch-name>/spec/log.md`.
This file is a central log for progress made, failed approaches tried, issues discovered, etc.

### `mem log add`

Flags:
- `--title <text>`
- `--body <text>`
- `--found <text>`
- `--decided <text>`
- `--open <text>`
- `--file <path-to-file>` uses a json file as in input

Adds a log entry. Similarly to `mem add`, we need to account for two kinds of users: humans and AI.
The log will be primarily maintained by the AI agent so it is important that agents can effectively
and reliably create content for their entries.

### `mem log list`

Prints log entries

Flags:
- `--branch <branch-name>` list log entries for a given branch

### Configuration management

Layer configurtion system with overrides:

- default config (branch name: `mem`, dir name: `.mem`)
- global config in (`~/.mem/mem.json`)
- project config in (`./mem.json`)
- env vars (`MEM_BRANCH_NAME`, `MEM_DIR_NAME`)

## Tech stack

This application will be written in Rust and distributed via a Nix flake.

We plan to use the following crates:
- clap
- figment (config management)
- anyhow

Suggested test deps:
- assert_cmd
- predicates
- pretty_assertions
- tempfile

## Handling `git` commands

Although git operations can be handled with specialized Rust crates,
for this project we are going to keep it simple and rely on system
installed `git` and simply shell out run the commands.
