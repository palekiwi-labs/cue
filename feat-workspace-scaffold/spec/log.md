# Project Log

## [ef3c270] feat: six-crate workspace compiles; codegen binary wired

Commit ef3c270 adds acuity-schema, acuity-api, acuity, and curator as Cargo workspace members. Dependency graph matches spec. codegen binary accepts output dir as argv[1] — no hardcoded cross-repo path. 130 existing tests pass.

- **Found:** ts-rs 12 API uses Config::with_out_dir + TS::export_all (not export_all_to)
- **Found:** single-file output requires #[ts(export_to = "types.ts")] on each type rather than a directory attribute
- **Found:** ts-rs 12 resolves export_to relative to Config::export_dir, so the binary only needs to set out_dir via Config
- **Decided:** codegen output dir is argv[1] with dist/ default — never hardcoded
- **Decided:** single types.ts via export_to attribute (not per-type files)
- **Decided:** flake.nix untouched in Phase 0

## [a93ee63] codegen pipeline proven end-to-end; cue-plugins bootstrapped

cargo run -p acuity-schema --bin codegen -- ../cue-plugins/src produced cue-plugins/src/types.ts (commit bf960c6). All three acceptance criteria for task/workspace-scaffold.md are met. Task marked complete.

- **Found:** ts-rs 12 carries Rust doc comments through into the generated TypeScript output verbatim
- **Found:** cue-plugins was pre-initialised by user with initial commit 44eefe5; only src/types.ts needed adding
- **Found:** /home/pl/code/palekiwi-labs/ is root-owned so agent cannot create sibling repos directly
- **Decided:** task workspace-scaffold marked complete; branch field cleared

## [da8aeeb] fix: review findings addressed (commit da8aeeb)

- **Found:** #[ts(export)] causes cargo test to write to bindings/ as an implicit side-effect via Config::from_env() — separate from the codegen binary's controlled output
- **Found:** removing export flag leaves export_all() in the binary fully functional since output_path() is set by export_to alone
- **Decided:** drop #[ts(export)], keep #[ts(export_to)] — programmatic binary is the sole export mechanism
- **Decided:** gitignore /dist/ and /bindings/ at crate level to suppress both codegen output dirs

