# Plan: `mem init`

## Dependencies

```toml
[dependencies]
clap    = { version = "4", features = ["derive"] }
figment = { version = "0.10", features = ["json", "env"] }
anyhow  = "1"

[dev-dependencies]
assert_cmd        = "2"
predicates        = "3"
pretty_assertions = "1"
tempfile          = "3"
```

## Module Structure

```
src/
  main.rs          # Entry: parse CLI via clap, dispatch to command handlers
  cli.rs           # clap types: Cli struct, Commands enum
  config.rs        # figment-based layered config (Config struct + load fn)
  git.rs           # Procedural git shell-out helpers
  commands/
    mod.rs
    init.rs        # mem init logic (orchestrates git.rs + config.rs)

tests/
  init.rs          # Integration tests for `mem init`
  helpers.rs       # Shared test helpers: setup_git_repo, setup_remote
```

## Config Loading (figment)

`Config` struct deriving `Deserialize`:

```rust
pub struct Config {
    pub branch_name: String,  // default: "mem"
    pub dir_name: String,     // default: ".mem"
}
```

Layered sources (lowest to highest priority):

1. Defaults (hardcoded `impl Default`)
2. `~/.config/mem/mem.json`
3. `./mem.json` (project config, loaded relative to CWD)
4. Env vars: `MEM_BRANCH_NAME`, `MEM_DIR_NAME`

## Git Shell-out Strategy (`src/git.rs`)

All git operations shell out to the system `git` binary via a single core helper.
To handle non-UTF-8 paths safely, it is generic over `AsRef<OsStr>`:

```rust
pub fn run_git<I, S>(args: I, cwd: &Path) -> anyhow::Result<String>
where
    I: IntoIterator<Item = S>,
    S: AsRef<std::ffi::OsStr>,
```

Functions needed for `init` (enforcing strict ref names like `refs/heads/` to avoid collisions):

| Function                                    | Git command                              |
|---------------------------------------------|------------------------------------------|
| `get_git_root(cwd)`                         | `git rev-parse --show-toplevel`          |
| `list_worktrees(cwd)` -> `Vec<PathBuf>`     | `git worktree list --porcelain`          |
| `branch_exists_local(root, name)`           | `git rev-parse --verify refs/heads/<name>` |
| `branch_exists_on_remote(root, remote, name)` | `git ls-remote --heads <remote> refs/heads/<name>` |
| `add_worktree(root, path, branch)`          | `git worktree add <path> <branch>`       |
| `add_worktree_orphan(root, path, branch)`   | `git worktree add --orphan <branch> <path>` |
| `fetch_branch(root, remote, branch)`        | `git fetch <remote> +refs/heads/<branch>:refs/remotes/<remote>/<branch>` |
| `git_add(cwd, files)`                       | `git add <files...>`                     |
| `git_commit(cwd, msg)`                      | `git commit -m <msg>`                    |

## `mem init` Logic (`src/commands/init.rs`)

1. Verify git repo: `run_git(["rev-parse", "--git-dir"], cwd)` — error if fails
2. `get_git_root(cwd)` — resolve project root
3. Load config (ensuring `HOME` is handled safely; skip global if missing)
4. `mem_path = root / config.dir_name`
5. If `mem_path.exists()` — print "Already initialized", exit 0
6. If `list_worktrees` contains EXACT `mem_path` — bail with error (worktree exists, dir missing)
7. `ensure_worktree(root, mem_path, &config)`:
   - `branch_is_checked_out?` — bail (already in use)
   - `branch_exists_local?`  — `add_worktree(root, mem_path, branch)`
   - `branch_exists_on_remote?` — `fetch_branch` then `add_worktree` (relying on git's auto-setup for upstream)
   - Neither — `add_worktree_orphan`, write `.gitignore` + `.rgignore`, `git_add`, `git_commit`
8. Print success message

`.gitignore` contents:
```
*/tmp/
*/ref/
```

`.rgignore` contents:
```
!*/tmp/
!*/ref/
```

## Testing Approach

### Environment variable injection

Env vars are injected per spawned process via `assert_cmd`'s `.env()` — the Rust
equivalent of `VAR=value cmd`. This does NOT call `std::env::set_var`, so it is
safe to use without `unsafe` and has no parallel test execution issues.

```rust
Command::cargo_bin("mem")?
    .current_dir(temp.path())
    .env("MEM_BRANCH_NAME", "test-mem")
    .env("MEM_DIR_NAME", ".test-mem")
    .arg("init")
    .assert()
    .success();
```

### Shared helpers (`tests/helpers.rs`)

```rust
/// Initialises a git repo with a user config and an initial commit (HEAD exists)
fn setup_git_repo(dir: &Path)

/// Creates a bare repo and wires it as `origin` for an existing local repo
fn setup_remote(local: &Path, remote: &Path)

/// Pushes a named branch from a local repo to its origin
fn push_branch(local: &Path, branch: &str)
```

Each test owns a `tempfile::TempDir` (auto-deleted on drop).

### Test scenarios (`tests/init.rs`)

| # | Scenario                          | Setup                                          | Expected outcome                                              |
|---|-----------------------------------|------------------------------------------------|---------------------------------------------------------------|
| 1 | Fresh repo, no mem branch         | Plain git repo                                 | Worktree created; orphan branch; `.gitignore` + `.rgignore` committed |
| 2 | Not a git repo                    | Bare temp dir (no `git init`)                  | Non-zero exit, stderr contains error                         |
| 3 | Already initialised               | Run `init` twice                               | Exit 0, stdout contains "already initialized"                |
| 4 | Local mem branch exists           | Create `test-mem` branch before running        | Worktree attached; no new orphan created                     |
| 5 | Remote mem branch exists          | Bare remote + `test-mem` branch pushed; local has none | Branch fetched; worktree attached; UPSTREAM tracked |
| 6 | Worktree exists, dir missing      | Manually register worktree, remove dir         | Non-zero exit, stderr contains error                         |
