---
status: complete
---
# Plan: Rename `mem` → `cue`

## Objective

Migrate the `mem` project to `cue`, covering the source code, configuration, tests,
Nix flake, documentation, and the GitHub repository. A second crate `cueban` (TUI)
is out of scope for this slice — only the core rename is addressed here.

## Scope

### In-scope
- Rust package / binary name
- CLI binary name and all internal Rust identifiers derived from `mem`
- Config file names (`mem.json` → `cue.json`)
- Default branch name (`mem` → `cue`)
- Default directory name (`.mem` → `.cue`)
- Environment variable prefix (`MEM_` → `CUE_`)
- Nix flake (`pname`, `name`, `description`)
- `AGENTS.md`
- Integration test helpers and env vars
- GitHub repository rename (`palekiwi-labs/mem` → `palekiwi-labs/cue`)

### Out-of-scope (future)
- Creating the `cueban` crate / workspace split
- `.mem/` worktree data migration (runtime concern, not source concern)

---

## Work Slices

### Slice 1 — Cargo & binary name

**Files**: `Cargo.toml`

Changes:
- `name = "mem"` → `name = "cue"` (this changes the compiled binary name automatically)

No `[[bin]]` section exists so no additional entry needed.

---

### Slice 2 — Source code: config defaults and env prefix

**File**: `src/config.rs`

| Location | Old | New |
|---|---|---|
| Line 36 | `branch_name: "mem".into()` | `branch_name: "cue".into()` |
| Line 37 | `dir_name: ".mem".into()` | `dir_name: ".cue".into()` |
| Line 49 | `std::env::var("MEM_CONFIG_DIR")` | `std::env::var("CUE_CONFIG_DIR")` |
| Line 50 | `.join("mem.json")` | `.join("cue.json")` |
| Line 53 | `.join(".config/mem/mem.json")` | `.join(".config/cue/cue.json")` |
| Line 57 | `project_root.join("mem.json")` | `project_root.join("cue.json")` |
| Line 60 | `Env::prefixed("MEM_")` | `Env::prefixed("CUE_")` |
| Line 89, 106 | `mem.json` in test strings | `cue.json` |
| Line 92–93, 109–111 | `MEM_ARTIFACT_TYPES`, `MEM_IGNORED_TYPES` in tests | `CUE_ARTIFACT_TYPES`, `CUE_IGNORED_TYPES` |
| Lines 120–138 | `MEM_CONTEXT__DEFAULT__INSTRUCTIONS` | `CUE_CONTEXT__DEFAULT__INSTRUCTIONS` |

---

### Slice 3 — Source code: internal Rust identifiers

**File**: `src/cli.rs`

- Field `mem_type` → `cue_type` (or `artifact_type` — prefer the more descriptive rename)
- Doc comment: `/// Manage mem configuration` → `/// Manage cue configuration`

**File**: `src/commands/list.rs`

| Symbol | Old | New |
|---|---|---|
| struct | `MemFile` | `CueFile` |
| field | `pub mem_type: Option<String>` | `pub cue_type: Option<String>` |
| function | `fn is_valid_mem_file(` | `fn is_valid_cue_file(` |
| function | `fn to_mem_file(` | `fn to_cue_file(` |
| variable | `let mem_path` (lines 149, list.rs; line 38, add.rs) | `let cue_path` |
| string literal | `"Run mem init first."` | `"Run cue init first."` |

**File**: `src/commands/add.rs`
- variable `let mem_path` → `let cue_path`

---

### Slice 4 — Integration tests

**File**: `tests/helpers.rs`

| Location | Old | New |
|---|---|---|
| Line 32 | `.cargo_bin("mem")` | `.cargo_bin("cue")` |
| Line 33 | `cmd.env("MEM_CONFIG_DIR", ...)` | `cmd.env("CUE_CONFIG_DIR", ...)` |
| Line 44–48 (doc comment) | `MEM_CONFIG_DIR`, `MEM_ARTIFACT_TYPES`, `MEM_IGNORED_TYPES` | `CUE_*` equivalents |
| Line 51 | `.cargo_bin("mem")` | `.cargo_bin("cue")` |
| Line 52–54 | `MEM_CONFIG_DIR`, `MEM_ARTIFACT_TYPES`, `MEM_IGNORED_TYPES` | `CUE_*` equivalents |
| Function name | `fn mem_cmd()` | `fn cue_cmd()` |

**File**: `tests/list.rs` (and all test files using `mem_cmd` or `MEM_*` vars)

- All `mem_cmd()` call sites → `cue_cmd()`
- `MEM_BRANCH_NAME` → `CUE_BRANCH_NAME`
- `MEM_DIR_NAME` → `CUE_DIR_NAME`

Run `rg "mem_cmd\|MEM_" tests/` to find all test files requiring updates.

---

### Slice 5 — Nix flake

**File**: `flake.nix`

| Location | Old | New |
|---|---|---|
| Line 2 | `"mem: a file-based memory system..."` | `"cue: a file-based memory system..."` |
| Line 21 | `pname = "mem"` | `pname = "cue"` |
| Line 42 | `name = "mem"` | `name = "cue"` |

---

### Slice 6 — Documentation and agent config

**File**: `AGENTS.md`

- Replace all references to `mem` CLI, `mem` protocol, `mem` skill, `.mem/` path,
  and `mem log add` command with their `cue` equivalents.

**File**: `.mem/master/spec/index.md`

- Update title, overview, and all occurrences of `mem` as the tool name.

---

### Slice 7 — GitHub repository rename

This is an administrative action performed via the GitHub web UI or `gh` CLI:

```bash
gh repo rename cue --repo palekiwi-labs/mem
```

Effects:
- Remote URL becomes `git@github.com:palekiwi-labs/cue`
- GitHub automatically redirects old URL, but local remote should be updated:

```bash
git remote set-url origin git@github.com:palekiwi-labs/cue
```

Update `opencode.json` if it contains a GitHub URL (currently it does not).

---

### Slice 8 — Local directory rename (optional / last step)

The repository itself lives at `/home/pl/code/palekiwi-labs/mem`.
After GitHub rename, optionally rename the local checkout:

```bash
mv /home/pl/code/palekiwi-labs/mem /home/pl/code/palekiwi-labs/cue
```

This is purely cosmetic and can be deferred.

---

## Ordering

```
Slice 1 (Cargo)
  → Slice 2 (config.rs — defaults + env)
  → Slice 3 (Rust identifiers — cli.rs, list.rs, add.rs)
  → Slice 4 (tests)
  → cargo test  ← gate: all tests must pass before proceeding
  → Slice 5 (flake.nix)
  → Slice 6 (docs / AGENTS.md / spec)
  → Slice 7 (GitHub rename + remote update)
  → Slice 8 (local dir rename, optional)
```

## Validation

After Slices 1–4:
```bash
cargo test
```

After all slices:
```bash
rg "mem" --type rust src/ tests/   # should return only generic-word matches
rg "MEM_" src/ tests/              # should be empty
```
