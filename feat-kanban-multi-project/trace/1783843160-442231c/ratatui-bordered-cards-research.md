---
refs: .cue/master/spec/index.md
status: open
---
# Ratatui 0.29 Bordered Card Items — Source Investigation

Investigation of how to render bordered card items in a ratatui 0.29 TUI,
replacing the `List` widget approach used by `curator`'s kanban columns.

## Source location
`/home/pl/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/ratatui-0.29.0/`

## Key findings

### List rendering internals
- Render loop: `src/widgets/list/rendering.rs:41-142` (`StatefulWidgetRef::render_ref`)
- Allocates a per-item `row_area` Rect (rendering.rs:93-98) but renders item
  content as flat `Text` via `item.content.render_ref(item_area, buf)`
  (rendering.rs:115). `ListItem` holds only a `Text<'a>` (item.rs:72).
- `highlight_style` applied AFTER content via `buf.set_style(row_area, ...)`
  (rendering.rs:137-139) — patches cell styles, does not overwrite symbols.
- `highlight_symbol` drawn via `buf.set_stringn` over the first column
  (rendering.rs:117-135) — WOULD overwrite a left border char.

### Why bordered items are impossible with List
No per-item Block concept. Content is `Vec<Line>` written flat. Embedding
border chars in Lines breaks: highlight_symbol overwrites left border;
height accounting (item.height()) includes border lines, breaking
get_items_bounds. Must render `Block` into a per-item `Rect` manually.

### ListState scroll algorithm
`get_items_bounds` at rendering.rs:146-224. For fixed-height cards it
collapses to: offset = clamp(selected - visible_count + 1, 0, ...).
curator creates a fresh ListState each frame (ui.rs:239) so offset is
NOT persisted — recomputed from selected each render.

### Available primitives
- `Block::bordered().render_ref(area, buf)` draws borders around any Rect
  (block.rs:702, render_borders at block.rs:714). `Block::inner(area)`
  (block.rs:638) gives inner Rect.
- `Layout::vertical([Length(h); n]).areas(area)` — needs known count,
  divides whole area; awkward for scrollable variable-N lists.
- `Rect::intersection` (rect.rs:188) clips card to viewport.
- `Buffer::set_string/set_line/set_style` (buffer.rs:319-408).

### Recommended approach
Manual per-card Rect computation + `Block::bordered()` per card. Fixed
CARD_HEIGHT=6 (2 border + 4 content). Simplified scroll-into-view since
heights are uniform. Selection via border color/type change.

See full report in the session output for code sketch and tradeoffs.
