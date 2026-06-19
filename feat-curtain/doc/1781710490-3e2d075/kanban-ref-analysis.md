# Kanban Reference Project Analysis

**Source**: `/home/pl/code/palekiwi-labs/cue/.ref/kanban/`
**Purpose**: Patterns, design decisions, and architectural choices usable as a basis for `curtain`.
**Date**: 2026-06-18

---

## 1. Project Structure

### Workspace Layout

The project is a multi-crate Rust workspace partitioned by architectural concern:

| Crate | Path | Role |
|---|---|---|
| `kanban-cli` | `crates/kanban-cli/` | CLI entry point, arg parsing, TUI orchestration |
| `kanban-tui` | `crates/kanban-tui/` | Ratatui terminal UI |
| `kanban-service` | `crates/kanban-service/` | Business logic, `KanbanContext`, undo/redo |
| `kanban-domain` | `crates/kanban-domain/` | Pure domain types (Board, Column, Card, Sprint) |
| `kanban-persistence` | `crates/kanban-persistence/` | Trait-based storage (JSON + SQLite backends) |
| `kanban-core` | `crates/kanban-core/` | Shared traits, error types, common utilities |

**Key observation**: Strict dependency inversion — high-level crates depend on traits defined in `kanban-core` and `kanban-persistence`, not on concrete implementations. The TUI never touches persistence directly.

---

## 2. Domain Model

### Hierarchy: Board → Column → Card

All entities use `Uuid` for identity and `chrono::DateTime<Utc>` for timestamps.

#### Board (`crates/kanban-domain/src/board.rs:30-64`)
```rust
pub struct Board {
    pub id: BoardId,                          // Uuid
    pub name: String,
    pub description: Option<String>,
    pub sprint_prefix: Option<String>,        // Branch name prefix (e.g. "KAN")
    pub card_prefix: Option<String>,          // Human-readable card ID prefix
    pub task_sort_field,                      // User sort preference
    pub task_sort_order,
    pub card_counter: u32,                    // Incremented per card, drives card_number
    pub active_sprint_id: Option<Uuid>,
}
```

#### Column (`crates/kanban-domain/src/column.rs:11-19`)
```rust
pub struct Column {
    pub id: ColumnId,
    pub board_id: BoardId,                    // FK to parent board
    pub name: String,                         // e.g. "To Do", "In Progress"
    pub position: i32,                        // Sort order
    pub wip_limit: Option<i32>,               // WIP limit, enforced at service layer
}
```

#### Card (`crates/kanban-domain/src/card.rs:59-79`)
```rust
pub struct Card {
    pub id: CardId,
    pub column_id: ColumnId,                  // FK to current column
    pub title: String,
    pub priority: CardPriority,               // Low | Medium | High | Critical
    pub status: CardStatus,                   // Todo | InProgress | Blocked | Done
    pub card_number: u32,                     // Assigned from Board.card_counter
    pub sprint_id: Option<Uuid>,
    pub completed_at: Option<DateTime<Utc>>,  // Set automatically on Done transition
}
```

---

## 3. App Struct and State

### Central State Holder (`crates/kanban-tui/src/app/mod.rs:73-111`)

The `App` struct is large and intentionally monolithic — it owns all UI state directly rather than nesting state in components:

```rust
pub struct App {
    pub store_manager: Arc<StoreManager>,
    pub should_quit: bool,
    pub mode: AppMode,
    pub mode_stack: Vec<AppMode>,        // Stack for nested modes (dialogs on top)
    pub input: InputState,
    pub ctx: TuiContext,                 // Bridge to service layer
    pub app_config: AppConfig,
    pub selection: SelectionHub,         // Active board/card/column selection
    pub animation: AnimationState,
    pub filter: FilterState,             // Search and sprint filter
    pub dialog_input: DialogInputState,
    pub focus: FocusState,               // Which panel has focus
    pub persistence: PersistenceState,   // File watcher + save worker handles
    pub multi_select: MultiSelectState,  // Bulk operations
    pub ui_state: UiState,               // Banners, list offsets
    pub sprint_view: SprintViewState,
    pub view: ViewState,                 // Active ViewStrategy (Flat/Grouped/Kanban)
    pub model: model::Model,             // Local cache of domain entities for rendering
    pub relationship: RelationshipState,
    pub save_error: Option<String>,
    pub pending_key: Option<char>,       // Buffer for multi-key sequences (e.g. 'gg')
    pub needs_redraw: bool,
    pub error_log: Arc<Mutex<ErrorLogState>>,
    // ... more config and migration fields
}
```

**Key observation**: `model::Model` is a local rendering cache of domain data — the TUI does not read domain state directly from the service on every render. Changes go through `TuiContext` and the model is refreshed explicitly.

---

## 4. Modal Input System

### AppMode enum (`crates/kanban-tui/src/app/mode.rs`)

Modes are the primary input-routing mechanism:

- `Normal` — main board/card view, vim-style navigation
- `CardDetail`, `BoardDetail`, `SprintDetail` — entity detail views
- `Search` — active search input
- `ArchivedCardsView` — view for restored/deleted cards
- `Settings` — global settings
- `Help(Box<AppMode>)` — help overlay, preserves previous mode
- `Dialog(DialogMode)` — 31 specific dialog types (e.g. `CreateCard`, `ConflictResolution`)
- `ErrorLog` — scrollable error log

**Mode stack pattern** (`crates/kanban-tui/src/app/mod.rs:460-470`):

```rust
fn push_mode(&mut self, mode: AppMode) {
    self.mode_stack.push(self.mode.clone());
    self.mode = mode;
}
fn pop_mode(&mut self) {
    if let Some(prev) = self.mode_stack.pop() {
        self.mode = prev;
    }
}
```

Dialogs layer on top of the current mode, allowing nested interactions without losing context.

---

## 5. Event Loop Architecture

### Structure (`crates/kanban-tui/src/app/mod.rs:1985`)

```
while !should_quit {
    if needs_redraw {
        terminal.draw(|frame| ui::render(self, frame));
    }
    tokio::select! {
        event = events.next() => { self.handle_key_event(event) }
        save_result = save_rx.recv() => { ... }
        file_change = watcher_rx.recv() => { ... }
        export_result = export_rx.recv() => { ... }
    }
}
```

**Key patterns**:
- Single `needs_redraw` flag avoids unnecessary redraws
- `tokio::select!` multiplexes events, async save results, file watcher notifications, and export results
- `terminal.draw` is the sole rendering call; rendering is never triggered from handlers

### Terminal Setup/Teardown (`crates/kanban-tui/src/app/mod.rs:2408-2423`)
```rust
fn setup_terminal() -> Terminal<CrosstermBackend<Stdout>> {
    enable_raw_mode();
    execute!(stdout(), EnterAlternateScreen, EnableMouseCapture);
    // ...
}
fn restore_terminal(terminal: &mut Terminal<...>) {
    disable_raw_mode();
    execute!(terminal.backend_mut(), LeaveAlternateScreen, DisableMouseCapture);
}
```

### Event Dispatch Path

1. `EventHandler` spawns a task polling `crossterm::event::read()` → sends to `mpsc` channel
2. `App::run` receives via `events.next().await`
3. `App::handle_key_event` (`app/mod.rs:808`) routes based on `self.mode`
4. Large `match self.mode { AppMode::Normal => match key.code { ... } }` block
5. Handlers in `src/handlers/` module mutate state and call `TuiContext`

**Example dispatch** (`app/mod.rs:881-1144`):
```rust
match self.mode {
    AppMode::Normal => match key.code {
        KeyCode::Char('/') => { self.push_mode(AppMode::Search); }
        KeyCode::Char('n') => { self.handle_create_card_key(); }
        KeyCode::Char('u') => { self.undo(); }
        // ...
    }
    AppMode::Dialog(DialogMode::CreateCard) => { ... }
    // ...
}
```

---

## 6. Rendering Architecture

### Root Render (`crates/kanban-tui/src/ui/mod.rs:39-175`)

Two-phase rendering:

**Phase 1 — Base view**: Render the current (or pre-overlay) mode's view into the main area.
```rust
let chunks = Layout::vertical([
    Constraint::Min(0),         // main area
    Constraint::Length(3),      // footer
]).split(frame.area());
```

Delegates to mode-specific render fns: `main_view::render_main`, `card_detail::render_card_detail_view`, etc.

**Phase 2 — Overlays**: Render dialogs, banners, error logs on top using `Clear`:
```rust
frame.render_widget(Clear, popup_area);
// then render dialog content into same area
```

Z-ordering is controlled by call order; banners and error logs are called last.

### RenderStrategy Trait (`crates/kanban-tui/src/render_strategy.rs:13`)

```rust
pub trait RenderStrategy {
    fn render(&self, app: &App, frame: &mut Frame, area: Rect);
}
```

Three concrete strategies:
- `SinglePanelRenderer` — flat list of all cards, optional sticky column headers
- `MultiPanelRenderer` — one column per panel, `Constraint::Percentage(100 / column_count)`
- `GroupedRenderer` — grouping by some criterion

`App.view: ViewState` holds a `Box<dyn ViewStrategy>` — strategy pattern for switching board layouts.

### Component Model

Hybrid approach:
- **Stateful components** own scroll/selection state (`CardListComponent`, `ListComponent`)
- **Stateless render fns** borrow `&App` and write to `&mut Frame`
- No universal `Widget` trait implementation; components have ad-hoc `render` methods

### Popup Centering (`crates/kanban-tui/src/components/popup.rs`)

```rust
fn centered_rect(percent_x: u16, percent_y: u16, area: Rect) -> Rect { ... }
fn centered_rect_abs(width: u16, height: u16, area: Rect) -> Rect { ... }
```

All popups call `frame.render_widget(Clear, area)` before their own content.

### Theming (`crates/kanban-tui/src/theme/`)

- `colors.rs`: Named constants (`FOCUSED_BORDER = Color::Cyan`)
- `styles.rs`: Functions returning `Style` objects (`focused_border()`, `priority_style(priority)`, `sprint_status_style(status)`)
- Styles are never inlined in UI code — always retrieved via theme functions

---

## 7. Service Layer and Command Pattern

### KanbanContext (`crates/kanban-service/src/context.rs:48-55`)

Central orchestrator between TUI and persistence:
- `execute(commands: Vec<Command>)` — primary mutation entry point; wraps in transaction, captures inverse, appends to audit log
- `create_card`, `move_card`, `archive_cards_detailed` — high-level methods translating TUI intents to `Command` batches
- `save()` / `reload()` — mediate between in-memory and durable storage

### Undo Stack (`crates/kanban-service/src/undo_stack.rs`)

```rust
pub struct UndoEntry {
    pub forward: Vec<Command>,
    pub inverse: Vec<Command>,   // captured from current state before execution
}
```

- Before execution: `cmd.capture_inverse(store)` generates the revert command
- `cursor` tracks position; pushing a new command truncates the redo tail

### Create Card Data Flow (End-to-End)

1. TUI calls `context.create_card(board_id, column_id, title, options)`
2. Context fetches next `card_number` from board, generates new `Uuid`, computes `position`
3. Wraps into `Command::Card(CardCommand::Create(...))`
4. `context.execute(vec![cmd])`:
   - Captures inverse (`DeleteCard`) for undo
   - Calls `cmd.execute(&ctx)` → `store.upsert_card(card)` + `store.upsert_board(board)`
5. JSON backend: marks dirty, background worker flushes to disk
6. SQLite backend: runs inside native SQL transaction, persisted immediately

---

## 8. Persistence Layer

### PersistenceStore Trait (`crates/kanban-persistence/src/traits.rs:83-120`)

```rust
pub trait PersistenceStore: Send + Sync {
    async fn save(&self, snapshot: StoreSnapshot) -> PersistenceResult<PersistenceMetadata>;
    async fn load(&self) -> PersistenceResult<(StoreSnapshot, PersistenceMetadata)>;
    async fn exists(&self) -> bool;
    fn path(&self) -> &Path;
}
```

### Backend Auto-detection (`crates/kanban-persistence/src/registry.rs`)

Content-sniffing approach:
- SQLite: matches `SQLite format 3\0` magic bytes
- JSON: matches leading `{` or `[` characters
- Falls back to file extension (`.sqlite`, `.db`, `.json`) if file is missing or ambiguous

### Background Save Worker (`crates/kanban-tui/src/app/mod.rs:375`)

```rust
fn spawn_save_worker(...) {
    tokio::spawn(async move {
        // listens on mpsc channel for flush signals
        // performs disk I/O in background
    });
}
```

---

## 9. CLI and Configuration

### Argument Parsing (`crates/kanban-cli/src/cli.rs:9-16`)

Uses `clap` v4 with derive macros:
```rust
#[derive(Parser)]
pub struct Cli {
    #[arg(value_name = "FILE", env = "KANBAN_FILE")]
    pub file: Option<String>,
    #[command(subcommand)]
    pub command: Option<Commands>,
}
```

Subcommands: `board`, `column`, `card`, `relation`, `sprint`, `export`, `import`, `completions`, `migrate`, `init`.

TUI launches when no subcommand is provided and stdin is a terminal.

### Configuration

- Format: TOML (loaded via `kanban_service::config::load()`)
- Loaded in CLI before TUI start; passed into `App::new_with_store` as `AppConfig`
- CLI args can override config values

### Error Display

Two mechanisms:
1. **Banner** — temporary `tracing::warn!`/`error!` notification rendered at top/bottom
2. **Error Log** — persistent in-memory log, accessible via `F12`; an `InMemoryLogLayer` hooks into `tracing` and captures events

---

## 10. Keybinding System

Keybindings are **hardcoded** in `App::handle_key_event` (`app/mod.rs:881-1144`) but structured via a `KeybindingRegistry` / `KeybindingProvider` trait for the help popup display.

```rust
pub trait KeybindingProvider {
    fn get_context(&self) -> KeybindingContext;
}
```

The registry is used only for rendering the help overlay — actual dispatch is a nested `match` block.

---

## 11. Testing Strategy

- Unit tests within each crate's `src/`
- Integration tests in `tests/` directories per crate
- TUI tests: key event simulation via `crossterm` types, component instantiation with `InMemoryStore`
- Notable test files: `tests/undo_redo_tests.rs`, `tests/card_list_component.rs`
- Real I/O flows use `TempDir` for isolation

---

## Key Patterns Summary (for curtain)

| Pattern | How kanban does it | Notes |
|---|---|---|
| State | Monolithic `App` struct with sub-state structs | All sub-states use `Default` |
| Mode routing | `AppMode` enum + `mode_stack` Vec | `push_mode`/`pop_mode` for dialogs |
| Event loop | `tokio::select!` in `while !should_quit` | `needs_redraw` flag avoids spurious draws |
| Rendering | Two-phase: base view then overlays | `Clear` widget for popup z-ordering |
| Layout switching | `Box<dyn RenderStrategy>` on `App` | Strategy pattern |
| Theming | Centralised `theme/` module with named fn helpers | No inline styles |
| Domain | Separate `domain` crate, pure Rust types | No async, no I/O |
| Mutations | `Command` enum + `execute()` on service | Undo via inverse command capture |
| Persistence | Trait-based, content-sniffed backend detection | Background save worker via `tokio::mpsc` |
| Error UX | Banner (transient) + Error Log (`F12`) | Hooks into `tracing` |
| CLI | `clap` derive, TUI launched when no subcommand + `stdin.is_terminal()` | Config via TOML |
