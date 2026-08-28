---
status: open
refs:
- .cue/master/task/cue-link-with-easier-api.md
- .cue/master/task/bug-cue-status-with-no-cue-dir.md
- crates/cuelib/src/store.rs
- crates/cue/src/commands/link.rs
- crates/cue/src/cli.rs
- crates/cue/src/commands/status.rs
- crates/cue/src/main.rs
---
# Executive Plan: `cue link --at` + strict store resolution

## Foreword

This plan implements the active task `cue-link-with-easier-api` and absorbs the
closed bug `bug-cue-status-with-no-cue-dir` (folded in because both touch
`resolve_store`).

**Decisions agreed with the user (informed by an opus consultation):**

1. **Flag name `--at`.** Not `--dir` (collides with the global `global = true`
   `-C/--dir` at `crates/cue/src/cli.rs:19`; a second `--dir` trips clap's
   duplicate-long-name debug assertion). Not `--to` (fights the positional,
   which already reads "link **to** /abs/store"). `--at` = "create the proxy
   **at** this location".
2. **Orthogonal source/destination model.** `store_path` (positional) = which
   store, default = discovered from cwd. `--at <PATH>` = where the proxy goes,
   default = cwd. The two are NOT mutually exclusive; `cue link /abs/store --at
   ./wt/x` is a valid, useful invocation.
3. **Store discovery is git-root-anchored** (`git_root(cwd).join(config.dir_name)`,
   the existing idiom), NOT a new ancestor walk. When linking, write
   `resolved.store_dir` (following any proxy redirect) into the new `STORE`
   file — never the raw `cue_dir` — to avoid creating a chain that
   `validate_store_target` (`crates/cuelib/src/store.rs:91-96`) later rejects.
4. **Bug fix at the source.** Make `resolve_store` strict (missing `.cue` is a
   loud error by default), delete the four duplicated guards, and attach a
   dynamic remedy hint at the CLI layer (`cue link` if `.git` is a file = linked
   worktree, else `cue init`).

**Sequencing:** Slice 1 (bug) first because it is foundational and the feature
depends on the same code; Slice 2 (feature) builds on top.

**Conventions:** TDD red-green. Every step ends with `cargo test` where noted.
One commit per green checkpoint (load the `git-commit` skill before committing).
Do NOT commit `.cue/` artifacts.

---

## Slice 1 — Strict `resolve_store` + dynamic remedy (bug fix)

- [ ] **1.1 Red: unit test for strict `resolve_store`.**
  In `crates/cuelib/src/store.rs` test module, add a test that calls
  `resolve_store(<non-existent .cue path>)` and asserts it errors with a message
  indicating the store is missing (e.g. contains "no cue store"). Confirm it
  FAILS (current code returns `Ok` passthrough at `store.rs:34-39`).

- [ ] **1.2 Green: make `resolve_store` strict.**
  At the top of `resolve_store` (`crates/cuelib/src/store.rs:31`), before the
  `STORE` check, bail with a factual message if `cue_dir` does not exist (e.g.
  `bail!("no cue store at {}", cue_dir.display())`). Update the doc comment
  (`store.rs:21-30`) which currently documents passthrough as unconditional.
  Verify 1.1 now passes. Note: all existing `store.rs` unit tests
  `create_dir_all` the `.cue` first, so they keep passing — confirm no regressions.

- [ ] **1.3 Find tests asserting the old error phrasing.**
  Grep `crates/cue/tests/` for the literal `directory does not exist. Run \`cue init\` first.`
  Known: `crates/cue/tests/list.rs:1333`. Catalogue every hit; these will need
  updating after the guards are removed (1.5) since the message will change.

- [ ] **1.4 Introduce a wrapper helper for the dynamic remedy.**
  Add a helper in the `cue` crate (e.g. `crate::commands::resolve_store(cwd) ->
  Result<cuelib::store::ResolvedStore>`) that: calls `git::get_git_root`,
  loads `Config`, builds `cue_dir`, calls `cuelib::store::resolve_store`, and on
  error attaches `.context()` with the dynamic remedy. Remedy logic: if
  `git_root.join(".git")` is a **file** (linked-worktree marker), hint
  `run 'cue link --at .' from the main repo, or 'cue link <store-path>' here`;
  else hint `run 'cue init'`. This is the single vehicle that applies the remedy
  consistently and also de-duplicates the git-root+config+cue_dir boilerplate
  repeated across commands. Write unit/integration tests covering both remedy
  branches (mock a `.git` file in a tempdir to trigger the worktree branch).

- [ ] **1.5 Route commands through the helper; delete duplicated guards.**
  Replace the per-command `resolve_store` + manual `.cue`-existence checks with
  calls to the new helper in: `add/mod.rs`, `list/mod.rs`, `log/mod.rs`,
  `commands/switch.rs`, `commands/status.rs`, and all `commands/context.rs`
  subcommands. Delete the now-redundant guards at `add/mod.rs:36`,
  `list/mod.rs:145`, `log/mod.rs:51`, `commands/switch.rs:52`. CRITICAL: do NOT
  route `commands/link.rs` target handling through this helper — `link`'s
  *target* legitimately does not exist yet (`link.rs:16-22` must stay). `link`
  should only call the helper for the *source* discovery in Slice 2.

- [ ] **1.6 Update affected tests.**
  Update every test catalogued in 1.3 to assert the new factual library message
  plus the remedy context (or assert the relevant substring). Add an integration
  test reproducing the original bug: `cue status` in a tempdir with no `.cue`
  exits non-zero with a clear error (no more silent "master (global)"), and a
  second test where a `.git` file triggers the `cue link` remedy hint.

- [ ] **1.7 Verify Slice 1.** `cargo test --workspace`. Commit checkpoint.

---

## Slice 2 — `cue link --at <PATH>` (feature)

- [ ] **2.1 Red: integration tests for `--at`.**
  In `crates/cue/tests/link.rs`, add failing tests:
  (a) `cue link --at <target>` resolves the store from cwd and creates a proxy
  at `<target>/.cue` with a `STORE` pointing at the cwd store; assert `STORE`
  content equals the canonical cwd store path.
  (b) Linking from inside an already-linked worktree writes
  `resolved.store_dir` (the real store), NOT the proxy path — assert no `STORE`
  chain forms and `validate_store_target` accepts the result.
  (c) Orthogonal mode: `cue link <abs_store> --at <target>` links an arbitrary
  store to an arbitrary target without `-C`.
  (d) `--at <non-existent dir>` errors clearly (target must exist; prevents typo
  silently creating e.g. `worktrees/my-featrue/.cue`).
  (e) Degenerate self-link guard: when resolved source store canonical path
  equals the target's `.cue` canonical path, error.

- [ ] **2.2 Add the `--at` argument.**
  In `crates/cue/src/cli.rs` (`Link` variant, ~line 134), add:
  `#[arg(long = "at", value_name = "PATH")] at: Option<std::path::PathBuf>`
  with a doc comment stating it is resolved relative to cwd/`-C` and that the
  target directory must already exist. Make `store_path` optional
  (`store_path: Option<std::path::PathBuf>`); keep `arg_required_else_help =
  true` so `cue link` with nothing prints help (this handles the neither-arg
  case).

- [ ] **2.3 Refactor `commands/link.rs::handle`.**
  New signature accepts `store_path: Option<PathBuf>` and `at: Option<PathBuf>`.
  Logic:
  1. Resolve **source store**: if `store_path` given, validate via
     `store::validate_store_target` and canonicalize (existing path). Else
     discover via the Slice 1 helper (`crate::commands::resolve_store(cwd)`) and
     use `resolved.store_dir`.
  2. Resolve **target dir**: if `at` given, require it to exist and be a
     directory (error otherwise); canonicalize its *parent* (the `.cue` itself
     does not exist yet, so canonicalizing the target `.cue` would fail). Default
     target = cwd.
  3. Self-link guard: compare canonical source store vs `target.join(".cue")`
     canonical; error if equal.
  4. Reuse the existing target check (`proxy_cue.exists()` at `link.rs:16-22`),
     create proxy dir, write `STORE` with the canonical **source store** path,
     handle `--task` as today.

- [ ] **2.4 Wire dispatch in `main.rs`.**
  Update the `Commands::Link` arm (`crates/cue/src/main.rs:121-123`) to pass the
  new `at` field through to `handle`.

- [ ] **2.5 Verify Slice 2.** `cargo test --workspace`. Confirm tests from 2.1
  pass and no regressions in existing `link.rs` tests. Commit checkpoint.

---

## Closeout

- [ ] **3.1 Update help/docs.** Verify `cue link --help` reads clearly; update
  any doc/reference mentioning `cue link` if present (grep the repo).
- [ ] **3.2 Log milestones.** `cue-log` after each slice commit and on
  completion (decisions recorded, dead ends if any).
- [ ] **3.3 Verify acceptance criteria.** Re-read
  `.cue/master/task/cue-link-with-easier-api.md` acceptance criteria; fill
  Evidence for automated criteria, flag any human-attested criteria as blocking
  if not yet signed off.