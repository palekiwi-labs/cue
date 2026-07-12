# Ratatui 0.29 List Internals & Bordered Card Alternatives

Research into why ratatui's `List` widget cannot render bordered items, and
what manual construction approach would replace it. Source: ratatui 0.29.0
at `/home/pl/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/ratatui-0.29.0/`.

---

## 1. How List renders items

The render loop lives in `src/widgets/list/rendering.rs:41-142`.

**Step-by-step flow:**

1. **Base style + outer block** (rendering.rs:45-47): `buf.set_style(area, self.style)`, then `self.block.render_ref(area, buf)`, then `list_area = self.block.inner_if_some(area)`.

2. **Bounds + offset** (rendering.rs:63-69): `list_height = list_area.height as usize`, then `get_items_bounds(state.selected, state.offset, list_height)` returns `(first_visible_index, last_visible_index)`. `state.offset` is mutated to `first_visible_index` so it persists across frames.

3. **The render loop** (rendering.rs:77-140) iterates `self.items.iter().enumerate().skip(state.offset).take(last - first)`:
   - **Position** (rendering.rs:84-91): `pos = (list_area.left(), list_area.top() + current_height)`, then `current_height += item.height()`.
   - **Per-item Rect** (rendering.rs:93-98): `row_area` with `width: list_area.width`, `height: item.height()`. A Rect IS allocated per item — but only used for `set_style` and as the content render target.
   - **Item style** (rendering.rs:100-101): `buf.set_style(row_area, item_style)`.
   - **Selection spacing** (rendering.rs:105-114): if active, `item_area` shrunk from left by `highlight_symbol_width`.
   - **Content render** (rendering.rs:115): `item.content.render_ref(item_area, buf)` — writes `Vec<Line>`s directly into the buffer. **No border/Block concept.**
   - **Highlight symbol** (rendering.rs:117-135): `buf.set_stringn(x, y+j, symbol, ...)` overwrites the first `highlight_symbol_width` columns of every line.
   - **Highlight style** (rendering.rs:137-139): `buf.set_style(row_area, self.highlight_style)` — patches Style of existing cells; does not change symbols.

**Variable-height items:** `ListItem::height()` = `self.content.height()` (number of lines). Used for both y-accumulation and row_area height. Scroll algorithm sums `item.height()` per item. Fully supported.

**`HighlightSpacing`** (`src/widgets/table/highlight_spacing.rs:5-24`): enum `Always | WhenSelected | Never`. Curator uses `Never` (ui.rs:237) — no symbol column reserved, no highlight symbol drawn.

---

## 2. Why List can't render bordered items

**Fundamental constraint:** `ListItem` holds a `Text<'a>` (`item.rs:72`), and `Text` is just `Vec<Line<'a>>`. The render loop writes Lines flat into the buffer via `item.content.render_ref(item_area, buf)` (rendering.rs:115). There is **no per-item `Block`** and no hook to draw one. The `row_area` Rect is used only for `set_style`, not for rendering a border widget.

**Hacking border characters into Lines breaks in 5 ways:**

1. **Highlight symbol overwrites left border.** When `selection_spacing` is active (default `WhenSelected`), the loop at rendering.rs:117-135 calls `buf.set_stringn` over the first `highlight_symbol_width` columns of every line. A `│` left-border char would be replaced by `>` or blank space. *(Curator sidesteps with `HighlightSpacing::Never`, but it's a latent trap.)*

2. **Height accounting breaks.** `item.height()` = `content.height()` = number of lines. Embedding border lines in content inflates the reported height, which feeds into `get_items_bounds` and y-accumulation.

3. **`highlight_style` recolors borders too.** `buf.set_style(row_area, self.highlight_style)` patches every cell in the row, including border chars.

4. **No corner connection / proper border set.** Manually placing `┌─┐│└┘` chars gives no `BorderType` support, no `border_set`, no title-on-border. Fragile char-painting.

5. **Text alignment/truncation interferes.** `Text::render_ref` truncates to `item_area.width`. Border chars at line edges could be truncated or misaligned.

**Conclusion:** List has no per-item border primitive. The only clean path is to drop `List` and render a `Block` widget into a per-item `Rect` yourself.

---

## 3. ListState scroll algorithm

`ListState` (`src/widgets/list/state.rs:47-50`): two fields — `offset: usize`, `selected: Option<usize>`. Navigation methods only mutate `selected`. **Offset is recomputed during render, not during navigation.**

The scroll-into-view algorithm is `List::get_items_bounds` at **rendering.rs:146-224**:

1. **Clamp offset** (rendering.rs:152): `offset = offset.min(items.len().saturating_sub(1))`.
2. **Forward pass** (rendering.rs:163-171): iterate from `offset`, accumulate heights, break when next item doesn't fit. `last_visible_index` = first item that doesn't fit.
3. **scroll_padding** (rendering.rs:176-183): with `scroll_padding=0` (curator default), `index_to_display = selected`.
4. **Scroll down if selected below viewport** (rendering.rs:189-204): while `selected >= last_visible`, grow last forward, drop from top if overflow.
5. **Scroll up if selected above viewport** (rendering.rs:208-221): while `selected < first_visible`, grow first backward, drop from bottom if overflow.
6. **Return** `(first_visible_index, last_visible_index)`. `state.offset = first_visible_index` persists.

**For fixed-height cards (curator's case), this collapses to:**
- If `selected < offset`: `offset = selected`
- If `selected >= offset + visible_count`: `offset = selected - visible_count + 1`
- Otherwise: `offset` unchanged

**Critical detail:** curator creates a **fresh `ListState` each frame** (ui.rs:239-242) with `offset=0`. So offset is NOT persisted — List recomputes scroll-into-view from `selected` every render. With `offset=0` input: if `selected >= visible_count`, `first_visible = selected - visible_count + 1`; else `first_visible = 0`. The manual implementation should replicate this (or improve by persisting offset in `App`).

---

## 4. Available primitives for manual construction

### `Block` (`src/widgets/block.rs`)
- `Block::bordered()` (block.rs:231-235): block with `Borders::ALL`.
- `Block::render_ref(area, buf)` (block.rs:702-711): renders borders into any arbitrary `Rect`.
- `Block::inner(area)` (block.rs:638-667): returns inner `Rect` subtracting borders + padding.
- `border_type(Plain|Rounded|Double|Thick)` (block.rs:554-557): selects symbol set.
- `border_style(Style)` (block.rs:466-469): colors only border cells.

### `Layout` (`src/layout/layout.rs`)
- `Layout::vertical([Constraint::Length(h1), ...]).areas(area)`: splits area via cassowary solver.
- **Limitation:** constraint count must be known; can't express "card extends past viewport" or partial clipping. For fixed-height uniform cards, manual Rect math is simpler.

### `Rect` (`src/layout/rect.rs`)
- `Rect::new(x, y, width, height)` (rect.rs:71-87).
- `Rect::intersection(other)` (rect.rs:188-199): overlapping Rect, or zero-area if none. **Use to clip a card Rect to the viewport.**
- `Rect::clamp(other)` (rect.rs:254-260): moves Rect to fit inside other.
- `top()/bottom()/left()/right()`: bottom/right are exclusive.

### `Buffer` (`src/buffer/buffer.rs`)
- `set_string(x, y, string, style)` (buffer.rs:319-325).
- `set_line(x, y, line, max_width)` (buffer.rs:368-387): prints a `Line` with spans/styles.
- `set_style(area, style)` (buffer.rs:400-408): patches style, doesn't change symbols.
- `buf[(x,y)]`: direct cell access.

### `Table` — does NOT support per-row borders
`Table::render_ref` writes cell content flat, same as List. No per-row `Block`.

---

## 5. Recommended approach

Render a scrollable column of bordered cards by **manually computing per-card `Rect`s and rendering a `Block` + inner content into each**, clipping to the viewport with `Rect::intersection`.

### Constants
Current cards are 4 content lines. With borders:
```rust
const CARD_CONTENT_HEIGHT: u16 = 4;
const CARD_HEIGHT: u16 = CARD_CONTENT_HEIGHT + 2; // +top/bottom border = 6
```

### Scroll-into-view (fixed-height simplification)
Since curator doesn't persist offset (fresh ListState each frame):
```rust
let visible_count = (inner.height / CARD_HEIGHT) as usize;
let offset = if tasks.is_empty() { 0 } else {
    let s = sel.min(tasks.len() - 1);
    if visible_count == 0 || s < visible_count { 0 } else { s - visible_count + 1 }
};
```

### Rendering each card
```rust
let buf = frame.buffer_mut();
let mut y = inner.top();
for (i, task) in tasks.iter().enumerate().skip(offset).take(visible_count) {
    let card_area = Rect::new(inner.left(), y, inner.width, CARD_HEIGHT);
    if card_area.intersection(inner).is_empty() { break; }

    let is_selected = i == sel && is_active;
    let card_block = Block::bordered()
        .border_type(if is_selected { BorderType::Thick } else { BorderType::Plain })
        .border_style(if is_selected {
            Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(Color::DarkGray)
        });
    card_block.render_ref(card_area, buf);
    let card_inner = card_block.inner(card_area);

    // Render 4 content lines into card_inner (reuse wrap_title logic)
    // ... title lines, project/priority line via buf.set_line

    y += CARD_HEIGHT;
}
```

### Selection highlighting
Border color + border type on the card's `Block` (Cyan+Thick when selected, DarkGray+Plain otherwise) — mirroring the existing column-border convention. Replaces `highlight_style`/`highlight_symbol` entirely.

---

## 6. Risks and tradeoffs

### What we lose by dropping List
- **Built-in scroll-into-view** — must reimplement (~5 lines for fixed-height).
- **`highlight_style`/`highlight_symbol`** — irrelevant; border-based highlight replaces it.
- **`ListState` navigation helpers** — still usable for `selected` index alone.
- **Variable-height support** — curator already enforces fixed 4-line cards, so non-issue.

### What's harder
- **Partial last-card clipping:** if `inner.height` isn't a multiple of `CARD_HEIGHT`, floor to fully-visible cards only.
- **Offset persistence:** current curator recomputes from selected each frame (jumps to pin at bottom when scrolling down). Persisting offset in `App` gives smoother scrolling — a design decision.
- **Test rewriting:** scroll tests need updating — card heights change from 4 to 6, project/priority row offsets shift from `vec![3, 7, 11]` to `vec![3, 9, 15]`.
