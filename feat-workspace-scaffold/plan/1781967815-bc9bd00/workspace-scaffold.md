---
status: complete
---
# Workspace & Contract Scaffolding — Executive Plan

## Foreword

This plan covers **Phase 0** of the cue ecosystem roadmap: adding four new
crate skeletons to the Cargo workspace and establishing the `ts-rs` codegen
pipeline. It addresses task
`.cue/master/task/1781942441-cef325f/workspace-scaffold.md`.

**Branch:** `feat/workspace-scaffold`

**Prerequisites:**
- Working Rust toolchain (confirmed present via Nix devshell in `flake.nix`)
- Current workspace has two crates: `cuelib`, `cue` (in `crates/`)
- `cue-plugins` repo does **not** yet exist on disk (only `palekiwi-labs/cue`
  is present under `/home/pl/code/palekiwi-labs/`)

**Scope constraints (hard):**
- No implementation code beyond `lib.rs` / `main.rs` stubs
- No SQLite, no HTTP handlers, no TUI code
- No TypeScript plugin logic — only the generated `types.ts` from a stub struct
- The codegen command must be repeatable (a `cargo run` invocation, not a
  one-off script)

**Out of scope (Phase 1+):**
- Defining the real `SessionIdle` event type
- Any `acuity` HTTP server logic
- Any `curator` TUI logic

---

## Steps

- [x] **1. Create the branch**
  ```
  git checkout -b feat/workspace-scaffold
  ```

- [x] **2. Add four crate skeletons under `crates/`**

  Create the following directories and minimal `Cargo.toml` + `src/` files:

  | Crate | Type | `src/` entry |
  |---|---|---|
  | `acuity-schema` | library | `src/lib.rs` (empty `pub mod`) |
  | `acuity-api` | library | `src/lib.rs` (empty `pub mod`) |
  | `acuity` | binary | `src/main.rs` (`fn main() {}`) |
  | `curator` | binary | `src/main.rs` (`fn main() {}`) |

  Dependency wiring per spec (`spec/cue-monorepo/index.md`):

  ```
  acuity-schema  -> serde (derive), ts-rs
  acuity-api     -> serde (derive)
  acuity         -> acuity-schema (path), acuity-api (path)
  curator        -> cuelib (path), acuity-api (path)
  ```

  `acuity-schema` and `acuity-api` have **no internal crate deps**.

- [x] **3. Register all four crates in the root `Cargo.toml` workspace**

  Add to the `members` array:
  ```toml
  "crates/acuity-schema",
  "crates/acuity-api",
  "crates/acuity",
  "crates/curator",
  ```

- [x] **4. Add `ts-rs` to `acuity-schema`**

  Use the latest version (`12.0.1` per `cargo search`). Add to
  `crates/acuity-schema/Cargo.toml`:
  ```toml
  [dependencies]
  serde = { version = "1", features = ["derive"] }
  ts-rs = { version = "12", features = ["serde-compat"] }
  ```

  Add a stub struct to `src/lib.rs` with the necessary derives so the codegen
  pipeline has something concrete to generate from:
  ```rust
  use serde::{Deserialize, Serialize};
  use ts_rs::TS;

  #[derive(Debug, Serialize, Deserialize, TS)]
  #[ts(export)]
  pub struct Placeholder {
      pub name: String,
  }
  ```
  This is intentionally temporary — it will be replaced by `SessionIdle` in
  Phase 1. Its only purpose is to prove the codegen pipeline works end-to-end.

- [x] **5. Add the codegen binary target to `acuity-schema`**

  `ts-rs` can export types via a test harness (`#[test] fn export() { ... }`)
  or via a dedicated binary. Use a Cargo binary target so the command is
  explicit and does not require `cargo test`.

  In `crates/acuity-schema/Cargo.toml`, add:
  ```toml
  [[bin]]
  name = "codegen"
  path = "src/bin/codegen.rs"
  ```

  `src/bin/codegen.rs`:
  ```rust
  use acuity_schema::Placeholder;
  use std::path::PathBuf;
  use ts_rs::TS;

  fn main() {
      // Output dir is supplied as argv[1]; defaults to dist/ inside the crate.
      // This keeps the cross-repo path out of the binary and in the invocation,
      // which is the precondition for wrapping this in a Nix derivation later.
      let out_dir = std::env::args()
          .nth(1)
          .unwrap_or_else(|| "dist".to_string());
      let out = PathBuf::from(&out_dir).join("types.ts");
      std::fs::create_dir_all(&out_dir).expect("create output dir");
      Placeholder::export_all_to(&out).expect("ts-rs export failed");
      println!("wrote {}", out.display());
  }
  ```

  The output path is **never hardcoded**. The caller decides where `types.ts`
  lands. For the manual vendoring workflow:
  ```
  cargo run -p acuity-schema --bin codegen -- ../cue-plugins/src
  ```
  For a future Nix derivation the caller passes `$out` instead.

  **Output format:** single `types.ts` file (not per-type). This is one
  vendored artifact, trivial to drift-check, and correct for a crate that
  will stay at 3-5 types by design.

- [x] **6. Verify `cargo build --workspace` succeeds**

  All six crates must compile cleanly. No warnings treated as errors at this
  stage (the stubs are intentionally empty).

- [x] **7. Initialise the `cue-plugins` repo**

  ```
  mkdir -p /home/pl/code/palekiwi-labs/cue-plugins/src
  cd /home/pl/code/palekiwi-labs/cue-plugins
  git init
  echo "# cue-plugins" > README.md
  git add README.md
  git commit -m "chore: initial commit"
  ```

  Create `src/` so the codegen binary has a target directory to write into.

- [x] **8. Run the codegen command and verify output**

  From the workspace root:
  ```
  cargo run -p acuity-schema --bin codegen -- ../cue-plugins/src
  ```

  Verify:
  - Command exits 0
  - `cue-plugins/src/types.ts` exists and contains a TypeScript interface for
    `Placeholder`

- [x] **9. Commit `types.ts` into `cue-plugins`**

  ```
  cd /home/pl/code/palekiwi-labs/cue-plugins
  git add src/types.ts
  git commit -m "chore: add generated types.ts (stub Placeholder)"
  ```

- [x] **10. Open a PR for `feat/workspace-scaffold` in the `cue` repo** *(awaiting user)*

  Title: `feat: workspace scaffold — add four crate skeletons and ts-rs codegen`

- [x] **11. Update task status**

  Fill in Evidence cells in
  `.cue/master/task/1781942441-cef325f/workspace-scaffold.md` and set
  `status: complete`.

- [x] **12. Log the milestone**

  `cue log add` summarising: crates added, dependency wiring, codegen pipeline
  proven, `cue-plugins` initialised.

---

## Decisions (resolved)

1. **Codegen output path**: The binary accepts `argv[1]` as the output
   directory. No path is hardcoded in the binary. The cross-repo location
   (`../cue-plugins/src`) lives in the invocation only. This is the
   precondition for wrapping codegen in a Nix derivation later (where the
   caller passes `$out`).

2. **Single `types.ts`**: All exported types go into one file via
   `export_all_to`. One vendored artifact, one import path, self-pruning when
   types are removed. Correct for a crate capped at ~3-5 types by design.

3. **`flake.nix`**: Not touched in Phase 0. The Nix-native codegen derivation
   (reading `acuity-schema`'s flake output as an input to `cue-plugins`'s
   flake) is a Phase 1+ concern. The argv-based binary is already
   derivation-ready when we get there.
