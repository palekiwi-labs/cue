# Todo: mem add

TDD vertical slices — one RED→GREEN cycle per slice.

## Completed (mem add base)

- [x] Slice 1 (tracer bullet): `mem add index.md` creates `.mem/<branch>/spec/index.md`
      Touches: git::get_current_branch, cli Add variant, commands/add.rs skeleton
- [x] Slice 2: content arg is written to the file
- [x] Slice 3: omitting content reads from stdin
- [x] Slice 4: `--type trace` places file under `trace/<ts>-<hash>/`
      Touches: git::get_short_head_hash
- [x] Slice 5: `--type tmp` places file under `tmp/<ts>-<hash>/`
- [x] Slice 6: `--type ref` places file under `ref/`
- [x] Slice 7: filename with subdir (`tickets/FEAT-1.md`) preserved inside spec/
- [x] Slice 8: existing file without `--force` → non-zero exit + error message
- [x] Slice 9: `--force` overwrites existing file
- [x] Slice 10: not a git repo → error
- [x] Slice 11: .mem/ missing → error suggesting `mem init`

## Pending: --clipboard flag

- [x] Slice 12 (tracer bullet): `-c` conflicts with inline content arg → clap rejects with non-zero exit
      Touches: Cargo.toml (arboard), src/cli.rs (clipboard flag)
- [x] Slice 13: `-c` conflicts with `--file` → clap rejects with non-zero exit
- [x] Slice 14: unsupported image extension (e.g. `.webp`) with `-c` → non-zero exit + "Unsupported image format" error
      Touches: src/main.rs (extension check before clipboard access)
      Note: extension is validated before clipboard is touched — no display needed

Manual verification only (require live display + clipboard):
  - Text in clipboard → correct text written to file
  - Image in clipboard + `.png` → valid PNG written
  - Image in clipboard + `.jpg` → valid JPEG written
  - Text in clipboard + `.png` extension → "Clipboard does not contain an image"
  - Image in clipboard + text extension → "Clipboard does not contain text"
