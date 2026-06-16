# Project Log

## [f929815-dirty] Refactor commands/ to thin handlers -- complete

Implemented the full rust-cli handler refactor in a single commit on refactor/commands-logic. All domain logic extracted from src/commands/ into four new top-level modules. All 114 tests pass, clippy clean.

- **Found:** commands/add.rs had resolve_clipboard embedded in main.rs, not in the handler -- moved to src/add/mod.rs
- **Found:** commands/list.rs Filter type was imported by cli.rs via crate::commands::list::Filter -- required a one-line import fix in cli.rs
- **Found:** commands/log/ was a subdirectory with mod.rs+add.rs+list.rs -- collapsed to single src/commands/log.rs in the thin handler; log list logic (43 lines) stayed in handler as it had no extractable domain logic
- **Found:** serde_yaml bare use statement triggered clippy::single_component_path_imports -- removed
- **Decided:** Re-exported AddOptions and ListOptions from commands/add.rs and commands/list.rs via pub use to keep main.rs unchanged
- **Decided:** Made CueFile, to_cue_file, and helper fns pub in src/list/mod.rs so commands/list.rs can reference them for output formatting
- **Decided:** Kept log list presentation (file read + print) in commands/log.rs rather than extracting to src/log/ -- it is trivially thin already

