# Implementation Plan: mem add

## Overview

Add a `mem add` subcommand that creates new files within the `.mem/<branch>/`
directory structure of the current git repository. Content can be supplied
inline, from a file, from stdin, or from the system clipboard.

## CLI Interface

    mem add [OPTIONS] <filename> [content]

    Arguments:
      [content]               Inline content string (conflicts with --file, --clipboard)

    Options:
      -t, --type <TYPE>       Destination [default: spec] [possible values: spec, trace, tmp, ref]
      -f, --file <FILE>       Read content from a file (conflicts with content, --clipboard)
      -c, --clipboard         Read content from system clipboard (conflicts with content, --file)
          --force             Overwrite if file already exists

## Content Resolution Priority

Resolved in `main.rs` before `commands::add::handle` is called:

1. `--clipboard` — read text via `arboard::Clipboard::get_text()`
2. `--file <path>` — read raw bytes from the given path
3. `[content]` inline arg — use as-is
4. (default) — read from `stdin` (the inline arg defaults to `-` as sentinel)

All three explicit flags conflict with each other via `clap`'s
`conflicts_with_all`; the fallback to stdin requires no flag.

## Path Resolution

- `spec`  → `.mem/<branch>/spec/<filename>`
- `trace` → `.mem/<branch>/trace/<unix_ts>-<short_hash>/<filename>`
- `tmp`   → `.mem/<branch>/tmp/<unix_ts>-<short_hash>/<filename>`
- `ref`   → `.mem/<branch>/ref/<filename>`

Branch is resolved from the current working branch
(`git rev-parse --abbrev-ref HEAD`), not the mem branch.
Subdirectories in `<filename>` are preserved
(e.g. `tickets/FEAT-1.md` → `spec/tickets/FEAT-1.md`).

## Clipboard Support

**Crate**: `arboard = "3.4"` — cross-platform clipboard access.
  - macOS: AppKit `NSPasteboard` (no external binary required).
  - Linux X11: `libxcb` (bundled in arboard).
  - Linux Wayland: requires `wl-clipboard` runtime tools.

**Scope**: Text only for v1. `get_text()` is called; if the clipboard
holds only an image or is empty, a user-friendly error is returned.

**Error Cases**:
- No display server (headless Linux): arboard error surfaced with context
  "Failed to access clipboard. Ensure a display server (X11 or Wayland) is running."
- Clipboard holds no text: arboard error surfaced with context
  "Clipboard does not contain text."

## Files Changed

- `Cargo.toml` — Add `arboard = "3.4"`
- `src/cli.rs` — Add `clipboard: bool` to `Add` variant;
  update `content` and `file` to `conflicts_with_all = ["file"/"content", "clipboard"]`
- `src/main.rs` — Add clipboard branch to content resolution block
- `tests/add.rs` — Integration tests for `--clipboard` flag

## Key Design Decisions

- Single `--type`/`-t` enum flag over three mutually exclusive booleans.
  Maps directly to a Rust `ValueEnum`; scales as destinations grow.
- No auto-commit: file written to .mem/ worktree; agent commits separately.
- `--force` required to overwrite; reinforces mem add = new files only.
- `trace/` and `tmp/` use `<unix_ts>-<short_hash>/` subdir for
  sortability and commit traceability.
- Clipboard is text-only for v1; image encoding (PNG via the `image` crate)
  deferred to avoid binary size overhead (~2-3 MB) until a concrete use case
  warrants it.