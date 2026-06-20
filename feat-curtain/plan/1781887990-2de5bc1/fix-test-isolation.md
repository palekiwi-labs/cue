---
status: complete
---
# Fix: ProjectStore Empty-File Crash + Test Isolation Leak

## Foreword

This plan addresses two related bugs surfaced by `.cue/feat-curtain/tmp/1781887990-2de5bc1/test.log`:

**Bug A — Library robustness (`cuelib/src/project.rs:87`):**
`ProjectStore::load()` handles a missing `projects.json` (returns empty store) but crashes
with `EOF while parsing a value` when the file exists but is empty. The fix is a one-line
guard before `serde_json::from_str`.

**Bug B — Test isolation leak (all test files):**
No shared test helper sets `CUE_DATA_DIR`, so any test that runs `cue init` falls through
to the developer's real `~/.local/share/cue/projects.json`. This affects both `helpers::cue_cmd()`
(~167 call sites) and `TestEnv::command()` (~46 call sites in `context_*.rs`). The fix is to
make `TestEnv` the single authoritative isolation abstraction, add `data_dir` to it, and
remove `helpers::cue_cmd()` and the two copies of `cue_cmd_with_data_dir`.

Architectural decision (Opus): Option B — extend `TestEnv`, delete `cue_cmd()`. The
thread-name heuristic (Option A) was rejected because it: bypasses `tempfile` RAII cleanup,
leaks dirs in `/tmp` across runs, and relies on an undocumented libtest implementation detail
that breaks under `cargo nextest`.

References:
- `cuelib/src/project.rs` — library fix target
- `cue/tests/helpers.rs` — `TestEnv` struct (lines 6-43), `cue_cmd()` (lines 50-56)
- `cue/tests/init.rs` — local `cue_cmd_with_data_dir` (lines 7-11)
- `cue/tests/project.rs` — duplicate `cue_cmd_with_data_dir` (lines 6-10)
- `cue/tests/add.rs` — 19 tests that call `cue init` without `CUE_DATA_DIR`
- `cue/tests/context_init.rs`, `context_path.rs`, `context_render.rs`, `context_show.rs`
  — 46 `TestEnv` call sites that also leak (missing `CUE_DATA_DIR` in `command()`)

---

## Commit 1 — Fix library + add unit test (TDD red→green)

### Step 1 — Write failing unit test in `cuelib/src/project.rs`

Add after `load_returns_empty_when_file_absent` (line 273), inside the `#[cfg(test)]` block:

```rust
#[test]
fn load_returns_empty_when_file_is_empty() {
    let dir = TempDir::new().unwrap();
    temp_env::with_var("CUE_DATA_DIR", Some(data_dir_path(&dir)), || {
        let path = store_path();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).unwrap();
        }
        std::fs::write(&path, "").unwrap();
        let store = ProjectStore::load().unwrap();
        assert!(store.entries().is_empty());
    });
}
```

Run `cargo test -p cuelib load_returns_empty_when_file_is_empty` — expect RED.

### Step 2 — Add empty-file guard in `ProjectStore::load()`

In `cuelib/src/project.rs`, after line 86 (the `read_to_string` call), insert:

```rust
if data.trim().is_empty() {
    return Ok(Self::default());
}
```

Final `load()` body (lines 80-90 after edit):

```rust
pub fn load() -> Result<Self> {
    let path = store_path();
    if !path.exists() {
        return Ok(Self::default());
    }
    let data = std::fs::read_to_string(&path)
        .with_context(|| format!("Failed to read {}", path.display()))?;
    if data.trim().is_empty() {
        return Ok(Self::default());
    }
    let store = serde_json::from_str(&data)
        .with_context(|| format!("Failed to parse {}", path.display()))?;
    Ok(store)
}
```

Run `cargo test -p cuelib load_returns_empty_when_file_is_empty` — expect GREEN.
Run `cargo test -p cuelib` — all existing tests must stay green.

### Step 3 — Commit

Message: `fix(cuelib): treat empty projects.json as empty store`

---

## Commit 2 — Extend `TestEnv`, fix `context_*.rs` leak (standalone shippable fix)

### Step 4 — Extend `TestEnv` in `cue/tests/helpers.rs`

Replace the `TestEnv` struct and its `impl` block (lines 6-43) with:

```rust
#[allow(dead_code)]
pub struct TestEnv {
    pub temp_dir: TempDir,
    pub config_dir: PathBuf,
    pub data_dir: PathBuf,
}

impl Default for TestEnv {
    fn default() -> Self {
        Self::new()
    }
}

impl TestEnv {
    #[allow(dead_code)]
    pub fn new() -> Self {
        let temp_dir = tempfile::tempdir().expect("Failed to create temp dir");
        let config_dir = temp_dir.path().join("config");
        let data_dir = temp_dir.path().join("data");
        std::fs::create_dir_all(&config_dir).expect("Failed to create config dir");
        std::fs::create_dir_all(&data_dir).expect("Failed to create data dir");
        Self { temp_dir, config_dir, data_dir }
    }

    /// Returns a fully isolated `cue` command. All state directories
    /// (config, data store) are scoped to this `TestEnv`'s `TempDir`
    /// and are cleaned up on drop. This is the only permitted way to
    /// spawn the `cue` binary in tests.
    #[allow(dead_code)]
    pub fn command(&self) -> assert_cmd::Command {
        let mut cmd = assert_cmd::Command::cargo_bin("cue").expect("Failed to find cue binary");
        cmd.env("CUE_CONFIG_DIR", &self.config_dir)
           .env("CUE_DATA_DIR", &self.data_dir)
           .env_remove("CUE_ARTIFACT_TYPES")
           .env_remove("CUE_IGNORED_TYPES")
           .current_dir(self.temp_dir.path());
        cmd
    }

    #[allow(dead_code)]
    pub fn root(&self) -> &Path {
        self.temp_dir.path()
    }
}
```

Key changes vs current:
- Added `data_dir: PathBuf` field (subdir `data/` of `temp_dir`)
- `command()` now sets `CUE_DATA_DIR` and removes `CUE_ARTIFACT_TYPES`/`CUE_IGNORED_TYPES`
  (aligning with what `cue_cmd()` already does for those two vars)
- `current_dir` is now set in `command()` (it wasn't before — `helpers.rs:35`)

Do NOT remove `cue_cmd()` in this commit. It is still referenced by `add.rs`, `list.rs`, etc.

### Step 5 — Run `context_*.rs` tests

```
cargo test --test context_init
cargo test --test context_path
cargo test --test context_render
cargo test --test context_show
```

All should pass. These now correctly isolate `CUE_DATA_DIR` via the extended `TestEnv`.

### Step 6 — Commit

Message: `fix(tests): add data_dir isolation to TestEnv`

---

## Commit 3 — Migrate `cue_cmd()` call sites, delete dead helpers

### Step 7 — Migrate `cue/tests/add.rs`

All tests in `add.rs` follow the same pattern:

```rust
// BEFORE
let temp = TempDir::new()?;
helpers::setup_git_repo(temp.path());
let mut cmd = helpers::cue_cmd();
cmd.current_dir(temp.path())
   .env("CUE_BRANCH_NAME", "test-mem")
   .env("CUE_DIR_NAME", ".test-mem")
   .arg("init");
cmd.assert().success();

// AFTER
let env = helpers::TestEnv::new();
helpers::setup_git_repo(env.root());
env.command()
   .env("CUE_BRANCH_NAME", "test-mem")
   .env("CUE_DIR_NAME", ".test-mem")
   .arg("init")
   .assert()
   .success();
```

The `TempDir` local var is replaced by `TestEnv`. Subsequent commands in the same test
replace `helpers::cue_cmd().current_dir(temp.path())` with `env.command()`. Any place
that references `temp.path()` becomes `env.root()`.

Remove `use tempfile::TempDir;` from `add.rs` if no longer needed after migration
(verify no test creates its own `TempDir` for other purposes).

### Step 8 — Migrate `cue/tests/list.rs`

Same mechanical transformation as Step 7.

### Step 9 — Migrate `cue/tests/log.rs`

Same mechanical transformation.

### Step 10 — Migrate `cue/tests/config_show.rs`

Same mechanical transformation (only 1 call site).

### Step 11 — Migrate `cue/tests/init.rs`

- Replace `helpers::cue_cmd()` call sites (lines 18, 51, 73, 89, 97, 124, 229) with `TestEnv`
  pattern.
- Delete the local `cue_cmd_with_data_dir` helper (lines 7-11) — now redundant because
  `TestEnv::command()` always sets `CUE_DATA_DIR`. Tests that previously used it
  (`test_init_registers_project_in_store`, `test_init_twice_does_not_duplicate_entry`) now
  use `env.data_dir` to locate `projects.json`:
  ```rust
  // BEFORE
  let store_path = data.path().join("projects.json");
  // AFTER
  let store_path = env.data_dir.join("projects.json");
  ```
- `test_init_remote_branch_exists` (line 200) needs two working trees: let `TestEnv` own the
  primary (`temp_local`) and create one extra `TempDir` for the bare remote (`temp_remote`).

### Step 12 — Migrate `cue/tests/project.rs`

- Delete the local `cue_cmd_with_data_dir` (lines 6-10).
- Replace all 14 call sites with `TestEnv`. Tests that previously inspected
  `data.path().join("projects.json")` now use `env.data_dir.join("projects.json")`.
- Remove `use tempfile::TempDir;` if no longer needed.

### Step 13 — Delete `helpers::cue_cmd()`

Remove lines 45-56 from `cue/tests/helpers.rs`. Remove `use tempfile::TempDir;` from
`helpers.rs` if no longer needed (check: `TempDir` is still used in `TestEnv::new()`
via `tempfile::tempdir()`, so the import stays).

### Step 14 — Full test run

```
cargo test
```

All 158+ tests must pass. Clippy must be clean:

```
cargo clippy --workspace -- -D warnings
```

### Step 15 — Commit

Message: `refactor(tests): unify on TestEnv, remove cue_cmd()`

---

## Verification checklist

- [ ] `cargo test -p cuelib load_returns_empty_when_file_is_empty` passes (Commit 1)
- [ ] `cargo test -p cuelib` all green (Commit 1)
- [ ] `cargo test --test context_init` (and context_path, context_render, context_show) green (Commit 2)
- [ ] `cargo test` full suite green (Commit 3)
- [ ] `cargo clippy --workspace -- -D warnings` clean (Commit 3)
- [ ] `grep -r 'cue_cmd()' cue/tests/` returns only the definition in helpers.rs — then after Commit 3: no results
- [ ] `grep -r 'cue_cmd_with_data_dir' cue/tests/` returns no results after Commit 3
- [ ] `grep -r 'CUE_DATA_DIR' cue/tests/helpers.rs` shows it is set in `TestEnv::command()`
