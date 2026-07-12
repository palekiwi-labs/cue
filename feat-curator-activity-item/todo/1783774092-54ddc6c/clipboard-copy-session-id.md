---
status: closed
priority: low
refs: undefined
---
# Clipboard copy: C-y copies selected session_id

From note `feat-curator-activity-item/note/sessions-list-improvement.md`.

In the Activity view, pressing `<C-y>` should copy the currently selected
session's `session_id` to the system clipboard.

## Implementation options

- **arboard crate** — cross-platform clipboard crate, pure Rust. Requires a new
  Cargo dependency.
- **xclip/pbcopy shell-out** — `std::process::Command` calling `xclip -selection clipboard`
  (Linux) or `pbcopy` (macOS). No new dep but platform-specific.

## Wiring sketch

1. Add `Action::CopySessionId` (unit variant, Copy-safe).
2. In `input.rs`: map `KeyCode::Char('y')` with `KeyModifiers::CONTROL` to `Action::CopySessionId`.
3. In `app.rs` / `process_msg`: handle the action — read `sel_session_id`, write to clipboard.
4. Optionally flash a status-bar message "copied: {id}" for 1–2 seconds.
