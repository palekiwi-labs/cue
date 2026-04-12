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

### Learn and practice Rust

The second purpose is for me to refresh, expand and practice my Rust skills.
While the implementation of this project is important, the process and experience
of building it together should serve the development of my skills as a Rust
programmer.

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

### `mem add` 

Arguments and flags:
- `filepath`: required, can include '/' to create subdirectories
- `content`: optional, defaults to stdin
- `--type`: "spec" | "trace" | "tmp" | "ref", optional, defauts to: "spec"

Creates files in `<dir-name>/<current-branch-name>/<type>/` according to the following rules:

- type "spec" or "ref":
  `<dir-name>/<current-branch-name>/<type>/<filepath>`

- type "trace" or "tmp":
  `<dir-name>/<current-branch-name>/<type>/<commit-timestamp>-<commit-hash>/<filepath>` where:
    <commit-timestamp> and <commit-hash> refer to current commit on the project branch, not `mem` orphan branch

### `mem list`

Prints a list of files in `<dir-name>/<current-branch-name>/` relative to project root

Flags:
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
- gix
- anyhow
