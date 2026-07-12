---
status: complete
refs:
- .cue/feat-curator-activity-item/spec/index.md
- .cue/feat-curator-activity-item/plan/1782659497-0c2ff37/slice6b-rendering.md
---
## Foreword

This plan implements Slice 6d: a gitui-inspired UX for the curator Activity view,
plus columnar session rows in the Sessions pane.

**Branch:** `feat/curator-activity-item` at commit `1ce0a99`

**Pipeline recap:** opencode-plugin → acuity server (SQLite) → curator TUI (SSE).
Slice 6c (two-pane activity view) is shipped and live-verified. Slice 6d
redesigns the navigation model and enriches the Sessions row layout.

**Two primary changes:**

1. **Columnar session rows** — four fixed-width columns (project | datetime |
   harness-abbrev | title) matching gitui's commit list layout. Datetime uses
   the host local timezone (resolves the high-priority deferred todo
   `fix-displayed-timezone-of-received-at`).

2. **gitui navigation model** — a three-state `ActivityLayout` enum replaces the
   current `ActivityPane` + `pane_expanded` bool. Default is fullscreen sessions
   list; Enter toggles a static detail pane; Right arrow enters fullscreen detail;
   Escape returns to split. The detail pane shows session metadata (Info block)
   above the events list.

**Repos:** cue only (`/home/pl/code/palekiwi-labs/cue`).

**No cue-plugins changes.** No acuity-schema changes.

**Commits planned:** 4 (helpers → columnar render → layout refactor → detail pane)

### Design decisions (locked)

- `ActivityLayout` enum: `SessionsFull` (default) | `Split` | `DetailFull`
- Enter: toggles `SessionsFull ↔ Split` (no-op in `DetailFull`)
- Right: from `SessionsFull` or `Split` → `DetailFull`; no-op in `DetailFull`
- Escape: from `DetailFull` → `Split`; no-op elsewhere
- Left: **no-op in Activity view** (does not navigate back; user spec)
- Tab/SwitchPane and z/ToggleExpand: removed entirely
- Datetime color: **Cyan** (not DarkGray — DarkGray matches selected row bg, making it invisible)
- Datetime format: `HH:MM` (today, 5 chars) or `Mmm DD HH:MM` (other days, 12 chars), padded to 12
- harness abbreviation: `opencode→oc`, `claudecode→cc`, `pi→pi`, else `??`
- Info block height: `Constraint::Length(8)` (6 data rows + 2 border rows)
- Info block fields: title | agent | model | parent_id | tokens (in/out) | errors
- In `Split`: sessions pane focused (j/k navigates sessions); detail pane static (dim)
- In `DetailFull`: events list navigable (j/k); Cyan+BOLD border
- `render_sessions_pane` is always called with `is_active = true` (it is only
  rendered when sessions is the focused element)

---

## Step 1 — chrono dep + pure helpers (TDD, commit 1)

**Files:** `crates/curator/Cargo.toml`, `crates/curator/src/ui.rs`

**Commit message:** `feat(curator): add format_datetime + harness_abbrev helpers`

**Exit criteria:** `cargo test -p curator` green + `cargo clippy -p curator -- -D warnings` clean

### 1a. Add chrono to Cargo.toml

```toml
chrono = { version = "0.4", features = ["clock"] }
```

### 1b. TDD — write tests first (RED)

Add to `ui.rs` `#[cfg(test)] mod tests` at the bottom, before the existing helpers tests:

```rust
// --- format_datetime ---

#[test]
fn format_datetime_invalid_falls_back_to_first_16_chars() {
    // Invalid ISO-8601 → returns first 16 chars of the raw string.
    let ts = "not-a-timestamp-but-long-enough";
    assert_eq!(format_datetime(ts), "not-a-timestamp-");
}

#[test]
fn format_datetime_invalid_short_falls_back_to_full_string() {
    let ts = "bad";
    assert_eq!(format_datetime(ts), "bad");
}

#[test]
fn format_datetime_past_date_contains_colon() {
    // A past date (definitely not today) should produce a string with ':'.
    // Format: "Mmm DD HH:MM" — always contains ':'.
    let ts = "2020-01-15T09:30:00Z";
    assert!(format_datetime(ts).contains(':'), "non-today format should contain ':'");
}

// --- harness_abbrev ---

#[test]
fn harness_abbrev_known_values() {
    assert_eq!(harness_abbrev("opencode"), "oc");
    assert_eq!(harness_abbrev("claudecode"), "cc");
    assert_eq!(harness_abbrev("pi"), "pi");
}

#[test]
fn harness_abbrev_unknown_falls_back_to_question_marks() {
    assert_eq!(harness_abbrev("unknown"), "??");
    assert_eq!(harness_abbrev(""), "??");
}
```

Confirm RED: these tests reference functions that do not exist yet.

### 1c. Implement (GREEN)

Add to `ui.rs` (above the `#[cfg(test)]` block, in the "Private helpers" section):

```rust
/// Format a UTC ISO-8601 timestamp string for display in the sessions pane.
///
/// Converts to the host local timezone. Returns:
/// - `"HH:MM"` (5 chars) when the date is today
/// - `"Mmm DD HH:MM"` (12 chars) for other days
///
/// Falls back to the first 16 chars of `ts` if parsing fails (or the full
/// string if it is shorter than 16 chars).
pub(crate) fn format_datetime(ts: &str) -> String {
    use chrono::{DateTime, Local, Utc};
    let Ok(dt_utc) = ts.parse::<DateTime<Utc>>() else {
        return ts.get(..16).unwrap_or(ts).to_string();
    };
    let local = dt_utc.with_timezone(&Local);
    let today = Local::now().date_naive();
    if local.date_naive() == today {
        local.format("%H:%M").to_string()
    } else {
        local.format("%b %d %H:%M").to_string()
    }
}

/// Map a harness identifier to its two-letter abbreviation for the sessions pane.
pub(crate) fn harness_abbrev(harness: &str) -> &'static str {
    match harness {
        "opencode" => "oc",
        "claudecode" => "cc",
        "pi" => "pi",
        _ => "??",
    }
}
```

Add `use chrono::{DateTime, Local, Utc};` inside `format_datetime` (local `use` is fine; or add at module top alongside other uses).

### 1d. Confirm GREEN

`cargo test -p curator` — all existing 78 tests + 5 new tests pass.
`cargo clippy -p curator -- -D warnings` — clean.

---

## Step 2 — Columnar session rows (commit 2)

**File:** `crates/curator/src/ui.rs`

**Commit message:** `feat(curator): columnar session rows with local datetime + harness abbrev`

**Exit criteria:** `cargo test -p curator` still green (no new tests — render fn). Visual QA at end of slice.

### 2a. Rewrite `render_sessions_pane` item construction

Replace the existing `items: Vec<ListItem>` map in `render_sessions_pane` (`ui.rs:191-212`):

```rust
let items: Vec<ListItem> = sessions
    .iter()
    .map(|s| {
        let project = format!("{:<8}", project_basename(&s.project_dir));
        let datetime = format!("{:<12}", format_datetime(&s.last_seen));
        let hx = harness_abbrev(&s.harness);
        let (label, is_placeholder) = session_label(Some(s), &s.session_id);
        let title_style = if is_placeholder {
            Style::default().fg(Color::DarkGray)
        } else {
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD)
        };
        let line = Line::from(vec![
            Span::styled(project, Style::default().fg(Color::Magenta)),
            Span::raw("  "),
            Span::styled(datetime, Style::default().fg(Color::Cyan)),
            Span::raw("  "),
            Span::styled(hx.to_string(), Style::default().fg(Color::Blue)),
            Span::raw("  "),
            Span::styled(label, title_style),
        ]);
        ListItem::new(line)
    })
    .collect();
```

**Note:** `datetime` is Cyan (not DarkGray) so it remains visible when the row
is highlighted (selected row uses `DarkGray` bg — DarkGray text would vanish).
`title` non-placeholder is `Cyan + BOLD` — visually distinct from plain Cyan
datetime by the bold modifier. `title` placeholder remains `DarkGray`.

---

## Step 3 — ActivityLayout refactor (TDD, commit 3)

**Files:** `crates/curator/src/app.rs`, `crates/curator/src/event.rs`,
`crates/curator/src/input.rs`, `crates/curator/src/main.rs`,
`crates/curator/src/ui.rs` (dispatch only — render bodies unchanged here)

**Commit message:** `refactor(curator): replace ActivityPane+pane_expanded with ActivityLayout`

**Exit criteria:** `cargo test -p curator` green + clippy clean. All existing
78 tests pass (2 old tests replaced by 3 new). RED→GREEN TDD for layout methods.

This step must update all five files atomically — removing `ActivityPane` from
`app.rs` breaks compilation in `main.rs` and `ui.rs` if not updated together.

### 3a. TDD — write tests first in app.rs (RED)

Replace the two existing tests `switch_activity_pane_toggles` and
`toggle_pane_expand_toggles` (`app.rs:1190-1208`) with:

```rust
// --- ActivityLayout transitions ---

#[test]
fn toggle_detail_pane_sessions_full_to_split() {
    let mut app = empty_app();
    assert_eq!(app.activity_layout, ActivityLayout::SessionsFull);
    app.toggle_detail_pane();
    assert_eq!(app.activity_layout, ActivityLayout::Split);
}

#[test]
fn toggle_detail_pane_split_to_sessions_full() {
    let mut app = empty_app();
    app.activity_layout = ActivityLayout::Split;
    app.toggle_detail_pane();
    assert_eq!(app.activity_layout, ActivityLayout::SessionsFull);
}

#[test]
fn toggle_detail_pane_noop_in_detail_full() {
    let mut app = empty_app();
    app.activity_layout = ActivityLayout::DetailFull;
    app.toggle_detail_pane();
    assert_eq!(app.activity_layout, ActivityLayout::DetailFull);
}

#[test]
fn enter_detail_full_from_sessions_full() {
    let mut app = empty_app();
    app.enter_detail_full();
    assert_eq!(app.activity_layout, ActivityLayout::DetailFull);
}

#[test]
fn enter_detail_full_from_split() {
    let mut app = empty_app();
    app.activity_layout = ActivityLayout::Split;
    app.enter_detail_full();
    assert_eq!(app.activity_layout, ActivityLayout::DetailFull);
}

#[test]
fn enter_detail_full_noop_when_already_full() {
    let mut app = empty_app();
    app.activity_layout = ActivityLayout::DetailFull;
    app.enter_detail_full();
    assert_eq!(app.activity_layout, ActivityLayout::DetailFull);
}

#[test]
fn return_from_detail_full_goes_to_split() {
    let mut app = empty_app();
    app.activity_layout = ActivityLayout::DetailFull;
    app.return_from_detail_full();
    assert_eq!(app.activity_layout, ActivityLayout::Split);
}

#[test]
fn return_from_detail_full_noop_in_sessions_full() {
    let mut app = empty_app();
    app.return_from_detail_full();
    assert_eq!(app.activity_layout, ActivityLayout::SessionsFull);
}

#[test]
fn return_from_detail_full_noop_in_split() {
    let mut app = empty_app();
    app.activity_layout = ActivityLayout::Split;
    app.return_from_detail_full();
    assert_eq!(app.activity_layout, ActivityLayout::Split);
}
```

Confirm RED before proceeding.

### 3b. app.rs changes (GREEN)

**Add new enum** (after the `Column` impl block, before `AcuityStatus`):

```rust
/// Three-state layout for the Activity view.
///
/// Transitions:
/// - `SessionsFull` ↔ Enter ↔ `Split`
/// - `SessionsFull` | `Split` + Right → `DetailFull`
/// - `DetailFull` + Escape → `Split`
/// - Left: no-op in Activity view (user spec)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActivityLayout {
    /// Default: sessions list fullscreen. j/k navigates sessions.
    SessionsFull,
    /// Split: sessions pane left (focused, j/k), detail pane right (static).
    Split,
    /// Detail pane fullscreen. j/k navigates events list.
    DetailFull,
}
```

**Remove** `ActivityPane` enum (lines 17-21).

**App struct** — replace `active_activity_pane` + `pane_expanded` with:

```rust
/// Layout state of the Activity view.
pub activity_layout: ActivityLayout,
```

**App::new** — replace the two old field initialisations with:

```rust
activity_layout: ActivityLayout::SessionsFull,
```

**Add new methods** (in the `impl App` block, replacing `switch_activity_pane`
and `toggle_pane_expand`):

```rust
/// Toggle the detail pane visibility: `SessionsFull ↔ Split`.
/// No-op when in `DetailFull` (Escape handles the return from there).
pub fn toggle_detail_pane(&mut self) {
    self.activity_layout = match self.activity_layout {
        ActivityLayout::SessionsFull => ActivityLayout::Split,
        ActivityLayout::Split => ActivityLayout::SessionsFull,
        ActivityLayout::DetailFull => ActivityLayout::DetailFull,
    };
}

/// Enter the fullscreen detail view from any other layout state.
/// No-op if already in `DetailFull`.
pub fn enter_detail_full(&mut self) {
    match self.activity_layout {
        ActivityLayout::SessionsFull | ActivityLayout::Split => {
            self.activity_layout = ActivityLayout::DetailFull;
        }
        ActivityLayout::DetailFull => {}
    }
}

/// Return from fullscreen detail to the split layout.
/// No-op if not in `DetailFull`.
pub fn return_from_detail_full(&mut self) {
    if self.activity_layout == ActivityLayout::DetailFull {
        self.activity_layout = ActivityLayout::Split;
    }
}
```

**Remove** `switch_activity_pane` and `toggle_pane_expand` methods.

### 3c. event.rs changes

Add `Enter` and `Escape` variants. Remove `SwitchPane` and `ToggleExpand`:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    Quit,
    Down,
    Up,
    Left,
    Right,
    Enter,
    Escape,
    SwitchView(View),
    Refresh,
    None,
}
```

### 3d. input.rs changes

Update `map_key`:

```rust
fn map_key(code: KeyCode) -> Action {
    match code {
        KeyCode::Char('q') | KeyCode::Char('Q') => Action::Quit,
        KeyCode::Char('j') | KeyCode::Down => Action::Down,
        KeyCode::Char('k') | KeyCode::Up => Action::Up,
        KeyCode::Char('h') | KeyCode::Left => Action::Left,
        KeyCode::Char('l') | KeyCode::Right => Action::Right,
        KeyCode::Enter => Action::Enter,
        KeyCode::Esc => Action::Escape,
        KeyCode::Char('1') => Action::SwitchView(View::Kanban),
        KeyCode::Char('2') => Action::SwitchView(View::Activity),
        KeyCode::Char('3') => Action::SwitchView(View::Diagnostics),
        KeyCode::Char('r') => Action::Refresh,
        _ => Action::None,
    }
}
```

### 3e. main.rs changes

Update imports — remove `ActivityPane`:

```rust
use app::{ActivityLayout, App, View};
```

Rewrite `process_msg` to use the new layout and action model:

```rust
fn process_msg(msg: Msg, app: &mut App, root: &Path, branch: &str) -> Result<LoopControl> {
    match msg {
        Msg::Input(Action::Quit) => return Ok(LoopControl::Quit),
        Msg::Input(Action::SwitchView(v)) => app.active_view = v,
        Msg::Input(Action::Refresh) => reload_tasks(app, root, branch)?,
        Msg::Input(Action::Down) => match app.active_view {
            View::Kanban => app.scroll_down(),
            View::Activity => match app.activity_layout {
                ActivityLayout::DetailFull => app.scroll_down_activity(),
                _ => app.scroll_down_sessions(),
            },
            View::Diagnostics => app.scroll_down_diagnostics(),
        },
        Msg::Input(Action::Up) => match app.active_view {
            View::Kanban => app.scroll_up(),
            View::Activity => match app.activity_layout {
                ActivityLayout::DetailFull => app.scroll_up_activity(),
                _ => app.scroll_up_sessions(),
            },
            View::Diagnostics => app.scroll_up_diagnostics(),
        },
        Msg::Input(Action::Left) => {
            // Left navigates kanban columns only. No-op in Activity (user spec)
            // and Diagnostics.
            if app.active_view == View::Kanban {
                app.move_left();
            }
        }
        Msg::Input(Action::Right) => match app.active_view {
            View::Kanban => app.move_right(),
            View::Activity => app.enter_detail_full(),
            View::Diagnostics => {}
        },
        Msg::Input(Action::Enter) => {
            if app.active_view == View::Activity {
                app.toggle_detail_pane();
            }
        }
        Msg::Input(Action::Escape) => {
            if app.active_view == View::Activity {
                app.return_from_detail_full();
            }
        }
        Msg::Input(Action::None) => {}
        Msg::Redraw => {}
        Msg::Sse(record) => {
            app.push_event(record);
            app.ensure_session_selection();
        }
        Msg::SseStatus(s) => app.acuity_status = s.into(),
    }
    Ok(LoopControl::Continue)
}
```

### 3f. ui.rs dispatch changes only

Update the import line to remove `ActivityPane`, import `ActivityLayout`:

```rust
use crate::app::{AcuityStatus, ActivityLayout, App, Column, SessionSummary, View};
```

Update `render_activity` dispatch (bodies of `render_sessions_pane` and
`render_events_pane` stay unchanged in this commit — that is step 4):

```rust
fn render_activity(frame: &mut Frame, app: &App) {
    let (view_area, help_area) = layout_with_help(frame.area());

    match app.activity_layout {
        ActivityLayout::SessionsFull => {
            render_sessions_pane(frame, app, view_area);
        }
        ActivityLayout::Split => {
            let [sessions_area, detail_area] = Layout::horizontal([
                Constraint::Ratio(1, 3),
                Constraint::Ratio(2, 3),
            ])
            .areas(view_area);
            render_sessions_pane(frame, app, sessions_area);
            render_events_pane(frame, app, detail_area); // renamed in step 4
        }
        ActivityLayout::DetailFull => {
            render_events_pane(frame, app, view_area); // renamed in step 4
        }
    }

    frame.render_widget(activity_help_line(&app.acuity_status, app.activity_layout), help_area);
}
```

Update `render_sessions_pane` — remove the `is_active` local variable that
references `ActivityPane::Sessions`; replace with `let is_active = true;`
(sessions pane is only rendered when it is the focused element).

Update `render_events_pane` — remove the `is_active` local variable that
references `ActivityPane::Events`; derive `is_active` from `app.activity_layout`:

```rust
let is_active = app.activity_layout == ActivityLayout::DetailFull;
```

Update `activity_help_line` signature and body — now takes `ActivityLayout`:

```rust
fn activity_help_line(status: &AcuityStatus, layout: ActivityLayout) -> Line<'static> {
    let (text, color) = acuity_status_parts(status);
    let mut spans = vec![
        Span::styled(" q", Style::default().fg(Color::Yellow)),
        Span::raw(" quit  "),
        Span::styled("1/2/3", Style::default().fg(Color::Yellow)),
        Span::raw(" views  "),
    ];
    match layout {
        ActivityLayout::DetailFull => {
            spans.push(Span::styled("Esc", Style::default().fg(Color::Yellow)));
            spans.push(Span::raw(" back  "));
        }
        _ => {
            spans.push(Span::styled("Enter", Style::default().fg(Color::Yellow)));
            spans.push(Span::raw(" detail  "));
            spans.push(Span::styled("→", Style::default().fg(Color::Yellow)));
            spans.push(Span::raw(" fullscreen  "));
        }
    }
    spans.push(Span::styled("j/k", Style::default().fg(Color::Yellow)));
    spans.push(Span::raw(" navigate  |  acuity: "));
    spans.push(Span::styled(text, Style::default().fg(color)));
    Line::from(spans)
}
```

### 3g. Confirm GREEN

`cargo test -p curator` — 78 existing + 9 new layout tests = 87 tests, all pass.
`cargo clippy -p curator -- -D warnings` — clean.

---

## Step 4 — Detail pane Info section + render redesign (commit 4)

**File:** `crates/curator/src/ui.rs`

**Commit message:** `feat(curator): detail pane with session Info block + gitui layout`

**Exit criteria:** `cargo test -p curator` green. Manual live QA (step 5).

No new unit tests (render functions). Existing helper tests continue to pass.

### 4a. Add render_session_info

New private function renders the Info block — the top section of the detail pane:

```rust
/// Render the session Info block (top section of the detail pane).
///
/// Always static — never focused. Height is fixed at 8 rows (6 data + 2 border).
/// Shows title, agent, model, parent_id, token totals, and error count from
/// the selected session's `SessionSummary`.
fn render_session_info(frame: &mut Frame, app: &App, area: Rect) {
    let content: Vec<Line> = if let Some(id) = app.sel_session_id.as_deref()
        && let Some(s) = app.sessions.get(id)
    {
        let title = s
            .session_title
            .as_deref()
            .unwrap_or("(no title)");
        let agent = s.agent.as_deref().unwrap_or("\u{2014}");
        let model = s.model.as_deref().unwrap_or("\u{2014}");
        let parent = s.parent_id.as_deref().unwrap_or("\u{2014}");
        vec![
            Line::from(vec![
                Span::styled(" Title:  ", Style::default().fg(Color::DarkGray)),
                Span::styled(title.to_string(), Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
            ]),
            Line::from(vec![
                Span::styled(" Agent:  ", Style::default().fg(Color::DarkGray)),
                Span::raw(agent.to_string()),
            ]),
            Line::from(vec![
                Span::styled(" Model:  ", Style::default().fg(Color::DarkGray)),
                Span::raw(model.to_string()),
            ]),
            Line::from(vec![
                Span::styled(" Parent: ", Style::default().fg(Color::DarkGray)),
                Span::raw(parent.to_string()),
            ]),
            Line::from(vec![
                Span::styled(" Tokens: ", Style::default().fg(Color::DarkGray)),
                Span::raw(format!("in={}  out={}", s.input_tokens, s.output_tokens)),
            ]),
            Line::from(vec![
                Span::styled(" Errors: ", Style::default().fg(Color::DarkGray)),
                Span::raw(format!("{}", s.error_count)),
            ]),
        ]
    } else {
        vec![Line::from(Span::styled(
            "  (no session selected)",
            Style::default().fg(Color::DarkGray),
        ))]
    };

    use ratatui::widgets::Paragraph;
    let block = Block::default()
        .title(" Session ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::DarkGray));
    let para = Paragraph::new(content).block(block);
    frame.render_widget(para, area);
}
```

Add `Paragraph` to the `ratatui::widgets` import at the top of `ui.rs`.

### 4b. Rewrite render_events_pane → render_detail_pane

Rename `render_events_pane` to `render_detail_pane`. The function now:
1. Vertically splits its `area` into an Info block (top) and events list (bottom).
2. Calls `render_session_info` for the Info block.
3. Renders the events list (extracted to a new `render_events_list` inner section)
   with `is_active` derived from whether the pane is focused.

```rust
/// Render the detail pane: Info block (top, static) + Events list (bottom).
///
/// `is_focused` controls whether the events list shows the active highlight
/// style and a Cyan+BOLD border (`DetailFull` mode) or a dim style (`Split`).
fn render_detail_pane(frame: &mut Frame, app: &App, area: Rect, is_focused: bool) {
    let [info_area, events_area] = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(8), Constraint::Min(0)])
        .areas(area);

    render_session_info(frame, app, info_area);

    // --- Events list ---
    let sel_id = app.sel_session_id.as_deref();
    let block_title = match sel_id {
        None => " Events ".to_string(),
        Some(id) => {
            let summary = app.sessions.get(id);
            let (label, _) = session_label(summary, id);
            format!(" Events \u{00b7} {label} ")
        }
    };

    let border_style = if is_focused {
        Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(Color::DarkGray)
    };
    let highlight_style = if is_focused {
        Style::default().bg(Color::DarkGray).add_modifier(Modifier::BOLD)
    } else {
        Style::default().add_modifier(Modifier::DIM)
    };

    let visible: Vec<&EventRecord> = app
        .events
        .iter()
        .rev()
        .filter(|e| {
            sel_id.is_some_and(|id| e.session_id == id)
                && !is_hidden_in_activity(&e.event_type)
        })
        .collect();

    let items: Vec<ListItem> = if visible.is_empty() {
        vec![ListItem::new(Line::from(Span::styled(
            "  (no events)",
            Style::default().fg(Color::DarkGray),
        )))]
    } else {
        visible
            .iter()
            .map(|record| {
                let ts = record
                    .received_at
                    .get(..19)
                    .unwrap_or(record.received_at.as_str());
                let summary = event_summary(record);
                let line = Line::from(vec![
                    Span::styled(
                        format!(" {ts}  "),
                        Style::default().fg(Color::DarkGray),
                    ),
                    Span::styled(
                        format!("{:<24}", record.event_type),
                        Style::default().fg(Color::White),
                    ),
                    Span::raw(format!("  {summary}")),
                ]);
                ListItem::new(line)
            })
            .collect()
    };

    let sel_visual = if visible.is_empty() {
        None
    } else {
        Some(app.sel_activity.min(visible.len() - 1))
    };

    let list = List::new(items)
        .block(
            Block::default()
                .title(block_title)
                .borders(Borders::ALL)
                .border_style(border_style),
        )
        .highlight_style(highlight_style)
        .highlight_symbol("> ");

    let mut list_state = ListState::default();
    list_state.select(sel_visual);
    frame.render_stateful_widget(list, events_area, &mut list_state);
}
```

### 4c. Update render_activity callers

In `render_activity` (from step 3), update the two `render_events_pane` calls
to `render_detail_pane` with the correct `is_focused` argument:

```rust
ActivityLayout::Split => {
    ...
    render_sessions_pane(frame, app, sessions_area);
    render_detail_pane(frame, app, detail_area, false); // static in split
}
ActivityLayout::DetailFull => {
    render_detail_pane(frame, app, view_area, true); // navigable
}
```

### 4d. Confirm GREEN + run full workspace tests

`cargo test --workspace` — all tests pass.
`cargo clippy --workspace -- -D warnings` — clean.

---

## Step 5 — Manual live QA

Start the curator against the live acuity instance and verify:

| # | Criterion | Verify by |
|---|-----------|-----------|
| 1 | Session list starts fullscreen (no right pane visible) | visual |
| 2 | Session rows show 4 columns: project / local-TZ datetime / 2-char harness / title | visual |
| 3 | Datetime is in host local timezone (not UTC) | compare with `date` output |
| 4 | j/k navigates sessions in fullscreen and split modes | keyboard |
| 5 | Enter opens split view (detail pane appears on right) | keyboard |
| 6 | Enter again closes split (returns to fullscreen sessions) | keyboard |
| 7 | Right arrow from fullscreen → fullscreen detail | keyboard |
| 8 | Right arrow from split → fullscreen detail | keyboard |
| 9 | Escape from fullscreen detail → split | keyboard |
| 10 | Left arrow in fullscreen detail → no-op (does not navigate back) | keyboard |
| 11 | Detail pane Info block shows title/agent/model/parent/tokens/errors | visual |
| 12 | j/k navigates events in fullscreen detail | keyboard |
| 13 | Tab and z keys are now no-ops (no pane switching / expand) | keyboard |
| 14 | Diagnostics view unaffected | visual + j/k |

---

## Out of scope

- Event tree folding (stage C)
- Per-session ring-buffer eviction (`improve-session-data-loading` todo)
- Human prompt collection (`collect-user-turn-messages` todo)
- acuity server log trim (separate todo)
