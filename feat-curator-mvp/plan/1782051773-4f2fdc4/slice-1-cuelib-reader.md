---
status: complete
---
# Slice 1 — cuelib artifact reader

## Foreword

This slice extends `cuelib` with the ability to read and parse artifact files
from a `.cue/` directory on disk. It is a prerequisite for Slice 2 (the
`curator` TUI). Two functions currently in the `cue` binary crate
(`extract_frontmatter_yaml`, `collect_files`) are migrated into `cuelib` so
both crates share one implementation.

**Branch**: `feat/curator-mvp`
**Implements**: Phase A of `plan/index.md`
**Prerequisite for**: `slice-2-curator-tui.md`

---

## Steps

- [x] Add `serde_yaml = "0.9"` to `crates/cuelib/Cargo.toml`
- [x] Migrate `extract_frontmatter_yaml` into `cuelib/src/artifact.rs`
      (identical logic, same signature — `pub fn`)
- [x] Migrate `collect_files` into `cuelib/src/artifact.rs`
      (identical logic — `pub fn`)
- [x] Add `ArtifactMeta` struct to `cuelib/src/artifact.rs`:
      `{ title: Option<String>, status_raw: Option<String>,
         priority_raw: Option<String>, artifact_type: String,
         path: PathBuf }`
- [x] Add `read_artifacts(root: &Path, branch: &str, artifact_type: &str)`
      function that walks `.cue/<branch>/<artifact_type>/`, calls
      `collect_files`, parses frontmatter, and returns `Vec<ArtifactMeta>`
- [x] Write unit tests for `read_artifacts` using `tempfile`
- [x] Update `cue` crate's `list/mod.rs` to call `cuelib` versions of
      `extract_frontmatter_yaml` and `collect_files` (remove local copies)
- [x] `cargo test -p cuelib` green
- [x] `cargo test -p cue` green
- [x] `cargo clippy --workspace` clean
- [x] Commit: `feat(cuelib): add artifact reader with frontmatter parsing`
