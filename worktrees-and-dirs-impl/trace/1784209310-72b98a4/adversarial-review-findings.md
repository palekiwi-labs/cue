# Adversarial Review Findings: feat/worktrees-and-dirs-impl

Synthesized from two independent reviewers (diff-reviewer-opus + consultant-gemini-flash).
Cross-referenced and deduplicated. Ordered by severity.

Branch: `feat/worktrees-and-dirs-impl` vs `master` (merge base `bf3b026`)
Diff: `.cue/worktrees-and-dirs-impl/tmp/1784209310-72b98a4/branch.diff`

## Critical

### C1. Relative path in STORE resolves against process CWD, not the proxy dir

`crates/cuelib/src/store.rs:38-43`

`resolve_store` reads the STORE contents, builds a `PathBuf`, runs `validate_store_target`
(which does `.exists()` / `.join("master").is_dir()` relative to process CWD), then
canonicalizes. Nothing enforces that STORE contains an absolute path.

Both reviewers flagged this as the top issue but DIVERGE on the fix:

- Reviewer A: spec mandates absolute paths -> reject non-absolute loudly.
- Reviewer B: absolute paths break container portability (host path != in-container
  mount path) -> allow relative paths but resolve them relative to `head_dir` (the
  proxy `.cue/` directory), not process CWD.

This is a design fork to resolve: it determines whether `cue link` writes absolute or
relative paths and whether hand-edited relative paths are supported.

## High

### H1. Missing STORE-chaining detection (spec mandates it)

`crates/cuelib/src/store.rs:28-49`

Spec (`index.md:62-63`) and plan require a loud error when the target itself contains
a STORE. Code performs no such check. A chain W1->W2->W3 silently resolves to W2.
Plan defers to Phase 4; spec treats as hard. Discrepancy to reconcile. Cheap fix:
`target.join("STORE").exists()` check in validate_store_target.

### H2. Regression: context path-traversal guard now blocks non-.cue profile refs

`crates/cue/src/context/mod.rs:172,191`

Refactor changed traversal guard anchor from `canonical_git_root` to
`canonical_store`. Side effect: any profile that legitimately references source files,
READMEs, or scripts inside the repo but outside `.cue/` is now silently blocked with
"Path traversal blocked". Guard should accept a path inside `canonical_store` OR
inside local `canonical_git_root`. Logged as "fixed" but over-narrowed.

### H3. cue list / cue context render emit absolute host paths under STORE redirect

`crates/cue/src/commands/list.rs:33`, `list/mod.rs:259-261`, `context.rs:64-67`

Every `strip_prefix(&root)` falls back to printing full absolute store path when
artifact lives in `store_dir` (outside worktree). Normal mode for worktree isolation,
so triggers immediately in proxy worktrees; leaks host paths into agent-facing output.
No integration test covers list/render inside a proxy, so CI passes while broken.

### H4. cue link ignores custom dir_name config

`crates/cue/src/commands/link.rs`

`cue link` hardcodes `.cue/` (`cwd.join(".cue")`) without loading Config. Every other
command honors `config.dir_name` (via cue.json). A project that customized the dir
name gets a useless `.cue/STORE` while real commands look at `.mycue/` and silently
fall back to a local store. `cue link` must load config from the target dir.

### H5. cue link duplicates store validation instead of reusing validate_store_target

`crates/cue/src/commands/link.rs:6-19` vs `store.rs:56-70`

`link::handle` reimplements the exists+master/ check inline. When H1's chaining guard
lands in store.rs, cue link will happily link to a chained store, creating a proxy
that errors on every subsequent command. Make validate_store_target pub and reuse.

### H6. TOCTOU on point-in-time artifact creation under parallel agents

`crates/cue/src/add/mod.rs:66-94`

The `<timestamp>-<hash>` subdirectory is derived from the HEAD commit's timestamp +
hash. Two parallel agents spawned from the same base commit (before either commits
local changes) resolve to the IDENTICAL subdirectory. Non-atomic check-then-create ->
interleaved writes or silent overwrite. Spec's concurrency model only protects
per-scope log.md appends, not point-in-time artifact creation. Fix: append random id,
or use O_EXCL.

### H7. Test coverage gaps on the read/redirect paths

The 10 link tests cover proxy creation; switch*in_proxy*... covers the write-split.
No test exercises the redirect on the read side where subtle bugs live: cue add,
cue log add/list, cue list, cue context render inside a proxy. Negative
resolve_store unit cases also missing: relative path (C1), chained STORE (H1),
empty/whitespace STORE, symlink target, STORE pointing at a file.

## Medium

### M1. --task master produces spurious "no task card found" warning

`crates/cue/src/commands/link.rs:44-61`. Looks for master/task/master.md which never
exists. Spec explicitly permits --task master. Skip warning for the reserved slug.

### M2. Duplicate resolve_store call in cue list

`crates/cue/src/commands/list.rs:25` + `list/mod.rs:141`. Resolves twice per
invocation; TOCTOU smell if STORE changes between calls. Resolve once, thread through.

### M3. canonical_store.to_str().unwrap_or("") can silently write empty STORE

`crates/cue/src/commands/link.rs:41`. Non-UTF-8 path -> empty file -> confusing
downstream error. Fail loudly instead.

### M4. Symlink attack surface in STORE target validation

`crates/cuelib/src/store.rs:56-69`. exists()/is_dir() follow symlinks; a master/
symlinked into arbitrary location passes validation. Canonicalize before validating.

### M5. Silent exit 0 on dangling --task slug

`crates/cue/src/commands/link.rs`. Orchestrators parse exit codes, not stderr. A
typoed slug creates an isolated scope the orchestrator never inspects. Consider
--allow-dangling flag.

### M6. handle_init / context init prints broken "Created" path under redirect

`crates/cue/src/commands/context.rs:18-24`. Same strip-prefix class as H3.

### M7. --dir precedence is misleading

Spec lists --dir as overriding store location, but it actually overrides CWD for
git::get_git_root(). head_dir is always git-root-relative. Clarify in docs.

## Low / Nits

### L1. read_head swallows all read errors as "no HEAD"

`crates/cuelib/src/head.rs:7-16`. Corrupt local HEAD silently drops agent into master
scope. Pre-existing but feature raises stakes. Distinguish absent vs unreadable.

### L2. Spec text contradicts adopted decision on cue switch for agents

`spec/index.md` "Out of Scope" still lists the prohibition. Already noted in log as
open item. Reconcile spec text.

### L3. resolve_store takes PathBuf by value forcing clones

Consider &Path.

### L4. .gitignore requirement for proxy .cue/ unimplemented with no cue link hint

One-line stderr reminder would reduce foot-guns.

### L5. Unrelated curator/ui.rs changes (~130 lines) + formatting churn inflate PR diff

Consider isolating or excluding from the PR.

### L6. store.rs doc comment omits absolute-path and no-chaining requirements

Update once C1/H1 addressed.

## Converged Merge-Blockers (both reviewers independently flagged)

1. C1 - relative-path handling in STORE (disagree on fix, needs decision)
2. H1 - missing chaining detection
3. H2 - context path-traversal guard regression (over-narrowed)
