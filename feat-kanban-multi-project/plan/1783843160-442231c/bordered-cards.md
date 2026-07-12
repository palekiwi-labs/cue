---
status: complete
refs:
- .cue/feat-kanban-multi-project/doc/1783843160-442231c/ratatui-list-internals-bordered-cards.md
- .cue/feat-kanban-multi-project/doc/1783843160-442231c/reference-kanban-rendering-approach.md
- .cue/feat-kanban-multi-project/plan/1783833119-949ca9f/review-fixes.md
---
## Foreword

This plan replaces the `List`-based card rendering in `render_column`
(`crates/curator/src/ui.rs:154-238`) with a manual `Block`-per-card approach
that draws visible borders around each kanban card.

### Why this is necessary
`ListItem` holds `Text` (`Vec<Line>`) and is written flat to the buffer via
`item.content.render_ref(item_area, buf)` (`ratatui/src/widgets/list/rendering.rs:115`).
There is no per-item `Block` and no hook to draw one. Hacking border characters
into `Line`s breaks in five ways (highlight symbol overwrite, height inflation,
`highlight_style` recoloring borders, no corner connection, truncation). The
only clean path is to drop `List` and render a `Block` into a per-card `Rect`
manually. Research details:
`.cue/feat-kanban-multi-project/doc/1783843160-442231c/ratatui-list-internals-bordered-cards.md`

### Decisions locked
All of the following were agreed in the design discussion:

- **D1 — Drop blank-pad line.** The fourth content line (blank padding) was a
  workaround for visual separation between borderless cards. Borders make it
  redundant. Card content = 3 lines: title-1, title-2-or-blank, project/priority.
- **D2 — CARD_HEIGHT = 5.** 3 content lines + 1 top border + 1 bottom border.
- **D3 — Border-only selection.** Selected card: `Cyan + BorderType::Thick`.
  Unselected: `DarkGray + BorderType::Plain`. No title color change, no
  background fill.
- **D4 — Drop title Cyan highlight.** All title spans get `Style::default()`.
  Border color is the sole selection signal.
- **D5 — Persist scroll offset in `App`.** Add `offset_open`, `offset_in_progress`,
  `offset_complete` fields (type `Cell<usize>`, default 0) so the viewport does
  not jump to pin the selected card at the bottom on every render.

### Card structure (5 rows per card)
```
┌──────────────────────────────────────┐  ← Block top border (row 0 of card)
│ title line 1                         │  ← row 1
│ title line 2 or blank                │  ← row 2
│ project-basename  priority           │  ← row 3
└──────────────────────────────────────┘  ← Block bottom border (row 4 of card)
```

### Scroll-into-view algorithm (O(1) for fixed-height cards)
```
visible_count = inner.height / CARD_HEIGHT  // floor division
current_offset = app.column_offset(col).get()
new_offset = if sel < current_offset {
    sel                              // selected scrolled above viewport — scroll up
} else if sel >= current_offset + visible_count {
    sel - visible_count + 1          // selected scrolled below viewport — scroll down
} else {
    current_offset                   // no change
};
app.column_offset(col).set(new_offset)
```

This replicates `ListState::get_items_bounds` for the fixed-height case
(ratatui `rendering.rs:146-224`) and replaces the previous "recompute from zero
each frame" behavior.

### Green gate for every step
`cargo test -p curator` + `cargo clippy -p curator -- -D warnings`

---

## Steps

### 1 — Add offset fields to `App`

- [ ] Add to `App` struct (`app.rs`):
  ```rust
  pub offset_open: Cell<usize>,
  pub offset_in_progress: Cell<usize>,
  pub offset_complete: Cell<usize>,
  ```
  Use `std::cell::Cell` so `render_column` (which takes `&App`) can update the
  offset without requiring `&mut App`. Add `use std::cell::Cell;` to imports.
- [ ] Initialize all three to `Cell::new(0)` in `App::new`.
- [ ] Add a helper on `App`:
  ```rust
  pub fn column_offset(&self, col: Column) -> &Cell<usize> {
      match col {
          Column::Open        => &self.offset_open,
          Column::InProgress  => &self.offset_in_progress,
          Column::Complete    => &self.offset_complete,
      }
  }
  ```
- [ ] Reset all three offsets to 0 in `App::reset` (if it exists) alongside the
  selection resets.
- [ ] Green gate.

### 2 — Rewrite `render_column`

This is the core change. Replace the entire `List`/`ListItem`/`ListState` block
with a manual `Block`-per-card loop.

- [ ] Remove the imports that will become unused:
  `List`, `ListItem`, `ListState`, `HighlightSpacing` from the ratatui use tree.
- [ ] Rewrite `render_column` (`ui.rs:154-238`):

  ```
  const CARD_HEIGHT: u16 = 5;

  // 1. Render the outer column Block (unchanged).
  frame.render_widget(&block, area);
  let inner = block.inner(area);
  if inner.is_empty() { return; }

  // 2. Compute visible window.
  let visible_count = (inner.height / CARD_HEIGHT) as usize;
  let offset_cell = app.column_offset(col);
  let old_offset = offset_cell.get();
  let new_offset = if tasks.is_empty() {
      0
  } else {
      let s = sel.min(tasks.len() - 1);
      if s < old_offset { s }
      else if s >= old_offset + visible_count.max(1) { s - visible_count.saturating_sub(1) }
      else { old_offset }
  };
  offset_cell.set(new_offset);

  // 3. Render each visible card.
  let buf = frame.buffer_mut();
  let mut y = inner.top();
  for (i, task) in tasks.iter().enumerate().skip(new_offset).take(visible_count) {
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
      let ci = card_block.inner(card_area);   // ci = card inner Rect

      let inner_w = ci.width as usize;
      let mut title_lines = wrap_title(&task.meta.title, inner_w);
      if title_lines.len() == 1 { title_lines.push(String::new()); }

      // Line 1 & 2: title (no color — D4)
      for (row, text) in title_lines.iter().take(2).enumerate() {
          buf.set_line(ci.left(), ci.top() + row as u16,
              &Line::from(Span::raw(text.clone())), ci.width);
      }
      // Line 3: project  priority
      let proj = task.project_root.file_name()
          .and_then(|n| n.to_str()).unwrap_or("");
      let prio = task.meta.priority_raw.as_deref().unwrap_or("normal");
      buf.set_line(ci.left(), ci.top() + 2,
          &Line::from(vec![
              Span::styled(proj.to_string(), Style::default().fg(Color::Magenta)),
              Span::raw("  "),
              Span::styled(prio.to_string(),
                  Style::default().fg(priority_colour(task.meta.priority_raw.as_deref()))),
          ]), ci.width);

      y += CARD_HEIGHT;
  }
  ```

- [ ] Verify `frame.render_widget` and `frame.buffer_mut()` are not mixed in a
  way that causes borrow conflicts. If they are, render the outer block via
  `block.render_ref(area, buf)` directly (same buf as cards) instead of
  `frame.render_widget`.
- [ ] Green gate.

### 3 — Update tests

- [ ] `render_column_cards_have_uniform_height` (`ui.rs`): update the expected
  `proj_rows` from `vec![3, 7, 11]` to `vec![4, 9, 14]`.
  Rationale: with CARD_HEIGHT=5, the column border is at row 0, then each card
  occupies 5 rows. Card 1: top-border=1, title1=2, title2=3, proj=4, bot-border=5.
  Card 2: top-border=6, title1=7, title2=8, proj=9, bot-border=10.
  Card 3: top-border=11, title1=12, title2=13, proj=14, bot-border=15.
- [ ] `render_column_scroll_keeps_last_selected_visible` and
  `render_column_scroll_keeps_first_selected_visible`: verify they still pass.
  With CARD_HEIGHT=5 and height=12, inner_height=10, visible_count=2. Selected=14:
  new_offset=13; cards 13 and 14 render. "task-14" still appears. If the tests
  fail due to the height change, adjust the terminal height parameter so at least
  2 cards fit (height >= 12 remains fine).
- [ ] If any test asserts a Cyan title color, remove or update it (D4: no title
  highlight). Grep for `Color::Cyan` in the test module.
- [ ] Green gate.

### 4 — Cleanup

- [ ] Run `cargo clippy -p curator -- -D warnings` and resolve any newly
  introduced warnings (unused imports, dead fields).
- [ ] Run `cargo test -p curator` — all tests must pass.
- [ ] Check that `Cell<usize>` fields on `App` do not break any existing tests
  that construct `App` directly (e.g. `App::new(tasks)` calls in tests) — `Cell`
  implements `Default` so `..App::new(tasks)` struct-update syntax remains valid.
