# Reference Kanban Project — Rendering Approach

Research into the rendering approach of the reference kanban project at
`/home/pl/code/palekiwi-labs/cue/.ref/kanban/`. This project does NOT use
ratatui's `List` widget for rendering cards.

---

## 1. Project overview

- **TUI library**: Ratatui with Crossterm (`Cargo.toml:48-49`).
- **Architecture**: Modular workspace:
  - `kanban-core`: Shared primitives (pagination, selection logic).
  - `kanban-domain`: Pure business logic and models.
  - `kanban-tui`: Terminal UI implementation.
- **Entry point**: `crates/kanban-cli/src/main.rs` (CLI), `crates/kanban-tui/src/lib.rs` (TUI library).

---

## 2. Rendering approach

The project bypasses ratatui's `List` widget entirely. It uses a **manual
buffer preparation approach** based on `ratatui::widgets::Paragraph` and
`ratatui::text::Line`.

- **Primitives**: Cards are rendered as a collection of `Line` objects, passed
  to a `Paragraph` widget (`crates/kanban-tui/src/render_strategy.rs:332`).
  Each card is a single `Line` via `render_card_list_item`
  (`crates/kanban-tui/src/components/card_list_item.rs:21`).
- **Loop structure**: The rendering loop (`render_strategy.rs:155-208`)
  iterates `visible_card_indices` (from a pagination component), fetches card
  data, and pushes the resulting `Line` into a vector.
- **Visual separation**: Does **not** draw borders around individual cards.
  Uses a bullet (`●`), checkbox (`[ ]`), and background highlights for
  selection (`card_list_item.rs:104-109`).

---

## 3. Scrolling mechanism

Scrolling is handled by a custom `Page` component tracking a **numeric scroll
offset**.

- **Scroll state**: `crates/kanban-core/src/pagination.rs:88`. Tracks
  `total_items` and `scroll_offset`.
- **Computation**: `get_page_info` (`pagination.rs:116`) computes which item
  indices are visible based on `scroll_offset` and `viewport_height`.
- **Scroll-into-view**: `scroll_offset_to_keep_visible` (`pagination.rs:22`)
  implements minimal-scroll semantics: if the selected index is outside the
  viewport, shifts `scroll_offset` just enough to bring it to the nearest
  edge.

---

## 4. Selection handling

Selection is decoupled from rendering and tracked via a `SelectionState`
primitive.

- **Tracking**: `SelectionState` (`crates/kanban-core/src/selection.rs`)
  stores an `Option<usize>` — the index of the selected item.
- **Visual treatment**: Selected card highlighted via **background color
  change** (`SELECTED_BG`). Applied to each `Span` within the card's `Line`
  if `is_selected && is_focused` (`card_list_item.rs:45-48`).

---

## 5. Layout computation

- **Column layout**: Multi-panel mode uses `Layout` with
  `Constraint::Percentage` to split area horizontally into columns
  (`render_strategy.rs:367-378`).
- **Card height**: Assumed to be exactly **1 row**. No support for variable
  card heights.
- **Indicator overhead**: `get_adjusted_viewport_height`
  (`pagination.rs:175`) reduces available `viewport_height` by 1 for an
  "above" indicator and 1 for a "below" indicator if needed.

---

## 6. Reusable patterns

1. **Virtual Viewport**: Instead of rendering everything to a `List` and
   letting the widget scroll, compute exactly which indices are visible and
   only render those. Provides full control over interspersed elements (like
   headers/indicators).

2. **Adjusted Viewport Height**: Calculating available rows *before*
   rendering cards allows for "X items above/below" indicators that don't
   jitter or overlap with content.

3. **Decoupled Selection/Pagination**: Using pure data structures
   (`SelectionState`, `Page`) for logic makes the rendering code purely
   functional and easy to test.

---

## 7. Key differences from our needs

- **Card Height**: Reference project assumes all cards are 1 row high. Our
  requirement for **bordered cards** requires cards to be 6+ rows high (top
  border + 4 content lines + bottom border).
- **Scrolling**: Because we have multi-row card heights, we cannot use a
  simple numeric `scroll_offset` of *items*. We need either fixed-height
  division (rows / card_height) or a row-based offset that sums card heights.
- **Border Rendering**: Reference project uses `Paragraph` for the whole
  column. For bordered cards, we need to render each card as its own `Block`
  within the column area, requiring a `Rect`-splitting loop — which is the
  approach recommended in the ratatui internals research (see companion doc
  `ratatui-list-internals-bordered-cards.md`).
