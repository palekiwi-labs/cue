---
title: C-y copies selected session_id to clipboard
status: open
priority: low
refs: .cue/feat-curator-activity-item/todo/1783774092-54ddc6c/clipboard-copy-session-id.md
---
# C-y copies selected session_id to clipboard

In the curator Activity view, pressing `<C-y>` should copy the currently
selected session's `session_id` to the system clipboard.

## Source

- Original todo: `.cue/feat-curator-activity-item/todo/1783774092-54ddc6c/clipboard-copy-session-id.md`

## Implementation options (decide at implementation time)

- **arboard crate** — cross-platform clipboard crate, pure Rust. Requires a
  new Cargo dependency.
- **xclip/pbcopy shell-out** — `std::process::Command` calling
  `xclip -selection clipboard` (Linux) or `pbcopy` (macOS). No new dep but
  platform-specific.

## Wiring sketch

1. Add `Action::CopySessionId` (unit variant, Copy-safe).
2. In `input.rs`: map `KeyCode::Char('y')` with `KeyModifiers::CONTROL` to
   `Action::CopySessionId`.
3. In `app.rs` / `process_msg`: handle the action — read `sel_session_id`,
   write to clipboard.
4. Optionally flash a status-bar message "copied: {id}" for 1-2 seconds.

## Acceptance Criteria

| # | Criterion (outcome) | Verify by | Evidence |
|---|---------------------|-----------|----------|
| 1 | `<C-y>` copies the selected session_id to the system clipboard | manual QA (paste elsewhere) | |
| 2 | Works on the host platform (Linux) | manual QA | |
| 3 | Optional status-bar confirmation shows the copied id | manual QA | |
| 4 | `cargo test --workspace` green | test run | |
| 5 | `cargo clippy --workspace -- -D warnings` clean | clippy run | |