# Research: `--branch` support for `cue log`

## Research question

Task `.cue/master/task/1782236275-5be2d77/log-with-branch.md` asks to add the
`--branch` flag to the `cue log` command (which `cue add` and `cue list` already
support), enabling listing logs for a branch and writing logs to a branch
(especially the master log).

This trace scopes the work, confirms the implementation pattern, and estimates
effort. All findings are verified against the current source tree.

## Headline finding: the task premise is partially incorrect

`cue log` is split into two subcommands (`Add` and `List`). One of them
already supports `--branch`:

- `cue log list` **ALREADY** supports `--branch` — `crates/cue/src/cli.rs:172-176`.
- `cue log add` does **NOT** support `--branch` — `crates/cue/src/cli.rs:151-170`.
  Its handler and storage function hardcode the current git branch.

**The real scope of this task is: add `--branch` to `cue log add` (and thread
it through to storage).** `log list` needs no change.

---

## Sourced findings

### 1. CLI architecture

- Subcommand enum: `Commands` at `crates/cue/src/cli.rs:26-27` (clap derive).
- Dispatch: `main` at `crates/cue/src/main.rs:32-111`.
- `log` has its own sub-enum: `LogCommands` at `crates/cue/src/cli.rs:148-177`.

### 2. The pattern to follow: `add --branch`

CLI field (note the `-b` short flag):

```rust
// crates/cue/src/cli.rs:53-55
        /// Save artifact to a specific branch instead of current
        #[arg(short = 'b', long)]
        pub branch: Option<String>,
```

Handler resolution (override else current branch, then sanitize):

```rust
// crates/cue/src/add/mod.rs:47-57
    // 3. Get branch
    let branch = if let Some(b) = branch_name {
        b
    } else {
        git::get_current_branch(root)
            .context("Could not determine current branch. Have you made your first commit yet?")?
    };
    let branch_dir = branch.replace(['/', '\\'], "-");

    // 4. Resolve destination directory
    let type_dir = cue_path.join(&branch_dir).join(&cue_type);
```

### 3. `list --branch` (already implemented, both top-level `list` and `log list`)

Top-level `list` CLI field (long only, no short):

```rust
// crates/cue/src/cli.rs:62-65
    List {
        /// List files for a specific branch instead of current
        #[arg(long, conflicts_with = "all")]
        branch: Option<String>,
```

`log list` CLI field — already present:

```rust
// crates/cue/src/cli.rs:171-176
    /// List log entries
    List {
        /// List log for a specific branch instead of current
        #[arg(long)]
        branch: Option<String>,
    },
```

`log list` handler branch resolution — `crates/cue/src/commands/log.rs:51-59`:

```rust
        LogCommands::List { branch } => {
            let branch_name = if let Some(b) = branch {
                b
            } else {
                git::get_current_branch(&root).context(
                    "Could not determine current branch. Have you made your first commit yet?",
                )?
            };
            let branch_dir = branch_name.replace(['/', '\\'], "-");
```

### 4. `log add` — current state (the gap to close)

`LogCommands::Add` has **no `branch` field** — `crates/cue/src/cli.rs:151-170`:

```rust
    Add {
        /// Entry title (required unless --file is used)
        #[arg(long)]
        title: Option<String>,
        /// Entry body text
        #[arg(long)]
        body: Option<String>,
        /// Findings (can be repeated)
        #[arg(long)]
        found: Vec<String>,
        /// Decisions (can be repeated)
        #[arg(long)]
        decided: Vec<String>,
        /// Open questions (can be repeated)
        #[arg(long)]
        open: Vec<String>,
        /// Read entry data from a JSON file
        #[arg(long, conflicts_with_all = &["title", "body", "found", "decided", "open"])]
        file: Option<String>,
    },
```

The handler destructures all fields but **no branch** — `crates/cue/src/commands/log.rs:20-46`:

```rust
        LogCommands::Add {
            title,
            body,
            found,
            decided,
            open,
            file,
        } => {
            // ... build `entry` ...
            let log_file_path = log::add_entry(&root, &config, LogAddOptions { entry })?;
```

`LogAddOptions` carries no branch — `crates/cue/src/log/mod.rs:22-24`:

```rust
pub struct LogAddOptions {
    pub entry: LogEntry,
}
```

`add_entry` hardcodes the current branch — `crates/cue/src/log/mod.rs:52-56`:

```rust
    let branch = git::get_current_branch(root)
        .context("Could not determine current branch. Have you made your first commit yet?")?;
    let branch_dir = branch.replace(['/', '\\'], "-");

    let log_file_path = cue_path.join(&branch_dir).join("spec").join("log.md");
```

### 5. Shared infrastructure (helpers exist but are partly underused)

- Git branch resolution: `cuelib::git::get_current_branch` — `crates/cuelib/src/git.rs:121-123`.
- Branch-name sanitizer: `cuelib::git::sanitize_branch_name` — `crates/cuelib/src/git.rs:141-143`.
  Note: the `log`, `add`, and `list` modules currently inline `.replace(['/', '\\'], "-")`
  rather than calling this helper, so there is minor DRY duplication across modules.

```rust
// crates/cuelib/src/git.rs:141-143
pub fn sanitize_branch_name(branch: &str) -> String {
    branch.replace(['/', '\\'], "-")
}
```

### 6. `master` special-casing

There is **no** CLI-level special-casing for `master`. Branches are uniform
directories under `.cue/<branch>/`. Writing to the master log therefore works
for free once `--branch master` is wired into `log add` — no extra logic
needed. (The protocol rule that `task` artifacts live on master is enforced by
the `cue-task` tooling layer passing `--branch master`, not by the `add`
storage path code.)

### 7. Test coverage & patterns

Tests live in `crates/cue/tests/`.

- `add --branch` integration test: `crates/cue/tests/add.rs:666-692`
  (`test_add_with_explicit_branch`) — runs `add --root --branch feature/other`
  and asserts the path `.test-mem/feature-other/spec/other.md`.
- `log add` integration test: `crates/cue/tests/log.rs:7-36`
  (`test_log_add_basic`) — runs `log add --title "Test Title"` and asserts
  output `.test-mem/main/spec/log.md` (uses `main` as the test branch).

Runner: standard `cargo test` (assertions via the `assert_cmd`/`predicates`
crates).

---

## Scope & effort estimate

**Scope is small and mechanical.** Three source files plus a test, following an
existing, duplicated pattern verbatim.

Required edits:

1. `crates/cue/src/cli.rs` — add a `branch` field to `LogCommands::Add`
   (alongside `title`/`body`/...). Use `#[arg(short = 'b', long)]` to match
   `add`, or `#[arg(long)]` to match `log list`. ~3 lines.
2. `crates/cue/src/commands/log.rs` — destructure the new `branch` field and
   pass it into `LogAddOptions`. ~2 lines.
3. `crates/cue/src/log/mod.rs` — add `branch: Option<String>` to
   `LogAddOptions`; in `add_entry`, resolve `opts.branch` else
   `git::get_current_branch(root)` (mirroring `add/mod.rs:47-53`). ~6 lines.
4. `crates/cue/tests/log.rs` — add a test mirroring `test_add_with_explicit_branch`
   that runs `log add --branch <x>` and asserts the path lands in
   `.test-mem/<x>/spec/log.md`. ~25 lines.

**Estimate: ~10-15 lines of production code + ~25 lines of test. Effort is on
the order of a single short session (well under an hour), dominated by writing
the integration test.** Risk is low: no storage-format change, no behavior
change for the default (no-`--branch`) path.

### Optional (out of scope, noted only)

- Consolidate the repeated `if let Some(b) = branch { b } else { git... }` +
  `.replace(...)` into one helper (a `resolve_branch_dir`-style function) and
  adopt `sanitize_branch_name` across `add`/`list`/`log`. This is a refactor,
  not required by the task, and would touch working code unnecessarily.

### Minor decision for the plan

Short flag for `log add --branch`: `add` uses `-b`; `log list`/top-level `list`
use long-only. Recommend `-b` for `log add` to match the sibling write command
`add`, since both are *write* operations.

## Confidence

High. All cited snippets were read directly from the current tree and
cross-checked against the subagent trace. The only soft spot is the `cue-task`
tooling layer (asserted to pass `--branch master` externally); it was not
located in the Rust source in this trace, but it is not on the critical path
for this task.
