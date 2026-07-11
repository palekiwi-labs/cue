use crate::app::{AcuityStatus, ActivityLayout, App, Column, SessionSummary, View};
use acuity_api::{AcuityEvent, EventRecord};
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, List, ListItem, ListState, Paragraph},
    Frame,
};

// ---------------------------------------------------------------------------
// Top-level dispatch
// ---------------------------------------------------------------------------

/// Render the full TUI, dispatching to the active view.
pub fn render(frame: &mut Frame, app: &App) {
    match app.active_view {
        View::Kanban => render_kanban(frame, app),
        View::Activity => render_activity(frame, app),
        View::Diagnostics => render_diagnostics(frame, app),
    }
}

// ---------------------------------------------------------------------------
// Shared layout helper
// ---------------------------------------------------------------------------

/// Split `area` into a main content area and a 1-line help/status bar.
fn layout_with_help(area: Rect) -> (Rect, Rect) {
    let [main, help] = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(0), Constraint::Length(1)])
        .areas(area);
    (main, help)
}

// ---------------------------------------------------------------------------
// Kanban view
// ---------------------------------------------------------------------------

/// Render the kanban board (View 1).
fn render_kanban(frame: &mut Frame, app: &App) {
    let area = frame.area();
    let (board_area, help_area) = layout_with_help(area);

    // Three equal columns.
    let [open_area, in_progress_area, complete_area] = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Ratio(1, 3),
            Constraint::Ratio(1, 3),
            Constraint::Ratio(1, 3),
        ])
        .areas(board_area);

    for (col, col_area) in [
        (Column::Open, open_area),
        (Column::InProgress, in_progress_area),
        (Column::Complete, complete_area),
    ] {
        render_column(frame, app, col, col_area);
    }

    let help = Line::from(vec![
        Span::styled(" q", Style::default().fg(Color::Yellow)),
        Span::raw(" quit  "),
        Span::styled("1/2/3", Style::default().fg(Color::Yellow)),
        Span::raw(" views  "),
        Span::styled("h/l ← →", Style::default().fg(Color::Yellow)),
        Span::raw(" column  "),
        Span::styled("j/k ↑ ↓", Style::default().fg(Color::Yellow)),
        Span::raw(" navigate  "),
        Span::styled("r", Style::default().fg(Color::Yellow)),
        Span::raw(" reload"),
    ]);
    frame.render_widget(help, help_area);
}

/// Colour scheme for priority badges.
fn priority_colour(priority: Option<&str>) -> Color {
    match priority {
        Some("critical") => Color::Red,
        Some("high") => Color::Yellow,
        Some("low") => Color::DarkGray,
        _ => Color::Gray,
    }
}

fn render_column(frame: &mut Frame, app: &App, col: Column, area: Rect) {
    let is_active = app.active_col == col;
    let tasks = app.column_tasks(col);
    let sel = app.column_sel(col);

    let border_style = if is_active {
        Style::default()
            .fg(Color::Cyan)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(Color::DarkGray)
    };

    let block = Block::default()
        .title(format!(" {} ({}) ", col.title(), tasks.len()))
        .borders(Borders::ALL)
        .border_type(if is_active {
            BorderType::Thick
        } else {
            BorderType::Plain
        })
        .border_style(border_style);

    let items: Vec<ListItem> = tasks
        .iter()
        .map(|task| {
            let priority_label = task.priority_raw.as_deref().unwrap_or("normal");
            let colour = priority_colour(task.priority_raw.as_deref());
            let line = Line::from(vec![
                Span::raw(task.title.as_str()),
                Span::raw("  "),
                Span::styled(format!("[{priority_label}]"), Style::default().fg(colour)),
            ]);
            ListItem::new(line)
        })
        .collect();

    let highlight_style = if is_active {
        Style::default()
            .bg(Color::DarkGray)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default().add_modifier(Modifier::DIM)
    };

    let list = List::new(items)
        .block(block)
        .highlight_style(highlight_style)
        .highlight_symbol("> ");

    let mut list_state = ListState::default();
    if !tasks.is_empty() {
        list_state.select(Some(sel));
    }

    frame.render_stateful_widget(list, area, &mut list_state);
}

// ---------------------------------------------------------------------------
// Activity Feed view  (two-pane: Sessions left 1/3, Events right 2/3)
// ---------------------------------------------------------------------------

/// Render the Activity Feed (View 2).
///
/// Three layouts driven by `app.activity_layout`:
/// - `SessionsFull`: sessions list fullscreen.
/// - `Split`: sessions left (1/3) + detail right (2/3), sessions focused.
/// - `DetailFull`: detail pane fullscreen, events navigable.
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
            render_detail_pane(frame, app, detail_area, false);
        }
        ActivityLayout::DetailFull => {
            render_detail_pane(frame, app, view_area, true);
        }
    }

    frame.render_widget(
        activity_help_line(&app.acuity_status, app.activity_layout),
        help_area,
    );
}

/// Render the Sessions pane (Pane 1 — left 1/3).
///
/// One row per session, sorted newest-first by `sorted_sessions()`.
/// Row format: `<project>  <harness>  <title-or-placeholder>`.
fn render_sessions_pane(frame: &mut Frame, app: &App, area: Rect) {
    // Sessions pane is only rendered when it is the focused element.
    let is_active = true;

    let sessions = app.sorted_sessions();

    // Find the visual index of the selected session.
    let sel_visual: Option<usize> = app
        .sel_session_id
        .as_deref()
        .and_then(|id| sessions.iter().position(|s| s.session_id == id));

    let items: Vec<ListItem> = sessions
        .iter()
        .map(|s| {
            let project = trunc_pad(project_basename(&s.project_dir), 20);
            let datetime = format!("{:<10}", format_datetime(&s.last_seen));
            let hx = harness_abbrev(&s.harness);
            let (label, is_placeholder) = session_label(Some(s), &s.session_id);
            let title_style = if is_placeholder {
                Style::default().fg(Color::DarkGray)
            } else {
                Style::default()
                    .fg(Color::White)
                    .add_modifier(Modifier::BOLD)
            };
            let line = Line::from(vec![
                Span::styled(hx.to_string(), Style::default().fg(Color::Blue)),
                Span::raw("  "),
                Span::styled(datetime, Style::default().fg(Color::LightCyan)),
                Span::raw("  "),
                Span::styled(project, Style::default().fg(Color::Magenta)),
                Span::raw("  "),
                Span::styled(label, title_style),
            ]);
            ListItem::new(line)
        })
        .collect();

    let highlight_style = if is_active {
        Style::default()
            .bg(Color::DarkGray)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default().add_modifier(Modifier::DIM)
    };

    let border_style = if is_active {
        Style::default()
            .fg(Color::Cyan)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(Color::DarkGray)
    };

    let list = List::new(items)
        .block(
            Block::default()
                .title(" Sessions ")
                .borders(Borders::ALL)
                .border_style(border_style),
        )
        .highlight_style(highlight_style);

    let mut list_state = ListState::default();
    list_state.select(sel_visual);
    frame.render_stateful_widget(list, area, &mut list_state);
}

/// Render the session Info block (top section of the detail pane).
///
/// Always static — never focused. Height is fixed at 10 rows (8 data + 2 border).
/// Shows title, session id, project path, agents, models, parent_id, token
/// totals, and error count from the selected session's `SessionSummary`.
fn render_session_info(frame: &mut Frame, app: &App, area: Rect) {
    let content: Vec<Line> = if let Some(id) = app.sel_session_id.as_deref()
        && let Some(s) = app.sessions.get(id)
    {
        let title = s.session_title.as_deref().unwrap_or("(no title)");
        let agents = session_unique_agents(app, id);
        let agents_str = if agents.is_empty() {
            "\u{2014}".to_string()
        } else {
            agents.join(", ")
        };
        let models = session_unique_models(app, id);
        let models_str = if models.is_empty() {
            "\u{2014}".to_string()
        } else {
            models.join(", ")
        };
        let parent = s.parent_id.as_deref().unwrap_or("\u{2014}");
        vec![
            Line::from(vec![
                Span::styled(" Title:      ", Style::default().fg(Color::DarkGray)),
                Span::styled(
                    title.to_string(),
                    Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD),
                ),
            ]),
            Line::from(vec![
                Span::styled(" ID:         ", Style::default().fg(Color::DarkGray)),
                Span::raw(id.to_string()),
            ]),
            Line::from(vec![
                Span::styled(" Project:    ", Style::default().fg(Color::DarkGray)),
                Span::styled(s.project_dir.clone(), Style::default().fg(Color::Magenta)),
            ]),
            Line::from(vec![
                Span::styled(" Agents:     ", Style::default().fg(Color::DarkGray)),
                Span::raw(agents_str),
            ]),
            Line::from(vec![
                Span::styled(" Models:     ", Style::default().fg(Color::DarkGray)),
                Span::raw(models_str),
            ]),
            Line::from(vec![
                Span::styled(" Parent:     ", Style::default().fg(Color::DarkGray)),
                Span::raw(parent.to_string()),
            ]),
            Line::from(vec![
                Span::styled(" Tokens In:  ", Style::default().fg(Color::DarkGray)),
                Span::raw(format_tokens(s.input_tokens)),
            ]),
            Line::from(vec![
                Span::styled(" Tokens Out: ", Style::default().fg(Color::DarkGray)),
                Span::raw(format_tokens(s.output_tokens)),
            ]),
            Line::from(vec![
                Span::styled(" Errors:     ", Style::default().fg(Color::DarkGray)),
                Span::raw(format!("{}", s.error_count)),
            ]),
        ]
    } else {
        vec![Line::from(Span::styled(
            "  (no session selected)",
            Style::default().fg(Color::DarkGray),
        ))]
    };

    let block = Block::default()
        .title(" Session Info ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::DarkGray));
    let para = Paragraph::new(content).block(block);
    frame.render_widget(para, area);
}

/// Render the detail pane: Info block (top, static) + Events list (bottom).
///
/// `is_focused` controls whether the events list shows the active highlight
/// style and a Cyan+BOLD border (`DetailFull` mode) or a dim style (`Split`).
fn render_detail_pane(frame: &mut Frame, app: &App, area: Rect, is_focused: bool) {
    let [info_area, events_area] = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(11), Constraint::Min(0)])
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
        Style::default()
            .bg(Color::DarkGray)
            .add_modifier(Modifier::BOLD)
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
                let ts = format_event_datetime(&record.received_at);
                let summary = event_summary(record);
                let line = Line::from(vec![
                    Span::styled(
                        format!(" {ts}  "),
                        Style::default().fg(Color::LightCyan),
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
        .highlight_style(highlight_style);

    let mut list_state = ListState::default();
    list_state.select(sel_visual);
    frame.render_stateful_widget(list, events_area, &mut list_state);
}

// ---------------------------------------------------------------------------
// Diagnostics view
// ---------------------------------------------------------------------------

/// Render the Diagnostics view (View 3) — tool-call events only.
fn render_diagnostics(frame: &mut Frame, app: &App) {
    let (list_area, help_area) = layout_with_help(frame.area());

    // Collect only tool-call events; no payload deserialization for the filter.
    let diag: Vec<&EventRecord> = app
        .events
        .iter()
        .rev()
        .filter(|e| is_diagnostic(&e.event_type))
        .collect();

    let items: Vec<ListItem> = diag
        .iter()
        .map(|record| {
            let ts = record
                .received_at
                .get(..19)
                .unwrap_or(record.received_at.as_str());
            match serde_json::from_str::<AcuityEvent>(&record.payload) {
                Ok(AcuityEvent::ToolCallRequested(ev)) => {
                    let line = Line::from(vec![
                        Span::styled(format!(" {ts}  "), Style::default().fg(Color::DarkGray)),
                        Span::raw(format!("req  {}", ev.tool_name)),
                    ]);
                    ListItem::new(line)
                }
                Ok(AcuityEvent::ToolCallCompleted(ev)) => {
                    let (label, style) = if ev.is_error {
                        let err = ev.error_text.as_deref().unwrap_or("error");
                        (
                            format!("ERR  {} \u{2014} {}", ev.tool_name, err),
                            Style::default().fg(Color::Red),
                        )
                    } else {
                        (
                            format!("ok   {}", ev.tool_name),
                            Style::default().fg(Color::Green),
                        )
                    };
                    let line = Line::from(vec![
                        Span::styled(format!(" {ts}  "), Style::default().fg(Color::DarkGray)),
                        Span::styled(label, style),
                    ]);
                    ListItem::new(line)
                }
                _ => ListItem::new(Line::from(format!(" {ts}  {}", record.event_type))),
            }
        })
        .collect();

    let list = List::new(items)
        .block(
            Block::default()
                .title(" Diagnostics ")
                .borders(Borders::ALL),
        )
        .highlight_style(
            Style::default()
                .bg(Color::DarkGray)
                .add_modifier(Modifier::BOLD),
        )
        .highlight_symbol("> ");

    let mut list_state = ListState::default();
    if !diag.is_empty() {
        list_state.select(Some(app.sel_diagnostics.min(diag.len() - 1)));
    }
    frame.render_stateful_widget(list, list_area, &mut list_state);

    frame.render_widget(status_help_line(&app.acuity_status), help_area);
}

// ---------------------------------------------------------------------------
// Shared render helpers (pub(crate) for unit tests in Slice 8)
// ---------------------------------------------------------------------------

/// Produce a one-line summary string for a live event record.
///
/// Exported `pub(crate)` so Slice 8 can unit-test all four event types without
/// constructing a full TUI frame.
pub(crate) fn event_summary(record: &EventRecord) -> String {
    match record.event_type.as_str() {
        "session_idle" => {
            if let Ok(AcuityEvent::SessionIdle(ev)) =
                serde_json::from_str::<AcuityEvent>(&record.payload)
            {
                let title = ev.session_title.as_deref().unwrap_or("(no title)");
                format!("idle: {title}")
            } else {
                "idle".to_string()
            }
        }
        "agent_turn_completed" => {
            if let Ok(AcuityEvent::AgentTurnCompleted(ev)) =
                serde_json::from_str::<AcuityEvent>(&record.payload)
            {
                let input = ev.input_tokens.unwrap_or(0);
                let output = ev.output_tokens.unwrap_or(0);
                match ev.model {
                    Some(m) => format!("turn: in={input} out={output} \u{00b7} {m}"),
                    None => format!("turn: in={input} out={output}"),
                }
            } else {
                "turn".to_string()
            }
        }
        "tool_call_requested" => {
            if let Ok(AcuityEvent::ToolCallRequested(ev)) =
                serde_json::from_str::<AcuityEvent>(&record.payload)
            {
                format!("tool: {}", ev.tool_name)
            } else {
                "tool".to_string()
            }
        }
        "tool_call_completed" => {
            if let Ok(AcuityEvent::ToolCallCompleted(ev)) =
                serde_json::from_str::<AcuityEvent>(&record.payload)
            {
                if ev.is_error {
                    let err = ev.error_text.as_deref().unwrap_or("error");
                    format!("ERR:  {} \u{2014} {}", ev.tool_name, err)
                } else {
                    format!("done: {}", ev.tool_name)
                }
            } else {
                "done".to_string()
            }
        }
        _ => record.event_type.clone(),
    }
}

/// Returns `true` if the event type belongs in the Diagnostics view.
///
/// Exported `pub(crate)` for unit testing in Slice 8.
pub(crate) fn is_diagnostic(event_type: &str) -> bool {
    event_type.starts_with("tool_call_")
}

/// Returns `true` if the event type should be **hidden** in the Activity Feed.
///
/// `session_updated` rows are pure noise: `push_event` absorbs their payload
/// into `SessionSummary` synchronously (`app.rs:230-250`) before render, so the
/// session header already shows their content. Mirrors `is_diagnostic`.
pub(crate) fn is_hidden_in_activity(event_type: &str) -> bool {
    event_type == "session_updated"
}

/// Build the session-group label for an Activity Feed header.
///
/// Returns `(text, is_placeholder)`. When a non-empty title is known (from
/// `SessionSummary.session_title`, populated via `session_updated`), the title
/// is used directly and `is_placeholder` is `false`. Otherwise the label falls
/// back to an **id suffix** (last ~8 chars of the session id, prefixed with
/// `…`) — the unique part of opencode session ids, which share a per-run
/// prefix. This fixes the old `.get(..8)` prefix-collision bug
/// (`ui.rs:214`).
///
/// `is_placeholder` lets the renderer dim the label until a real title arrives,
/// making the title-flip a visible verification signal.
pub(crate) fn session_label(
    summary: Option<&SessionSummary>,
    session_id: &str,
) -> (String, bool) {
    if let Some(s) = summary
        && let Some(title) = &s.session_title
        && !title.is_empty()
    {
        return (title.clone(), false);
    }
    // Suffix: last ~8 chars. `get` is boundary-guarded (returns None at a
    // non-char-boundary); `unwrap_or` falls back to the full id. Session ids
    // are ASCII in practice, so this is exact.
    let start = session_id.len().saturating_sub(8);
    let suffix = session_id.get(start..).unwrap_or(session_id);
    (format!("\u{2026}{suffix}"), true)
}

/// Extract the basename (last path component) from a project_dir string.
///
/// Returns the full `project_dir` if it has no `/` or the basename is empty.
/// Used by `render_sessions_pane` to display a compact project name per row.
pub(crate) fn project_basename(project_dir: &str) -> &str {
    project_dir
        .rsplit('/')
        .next()
        .filter(|b| !b.is_empty())
        .unwrap_or(project_dir)
}

/// Build the help/status bar line for the Activity view.
///
/// Layout-aware: shows Enter/→ hints in `SessionsFull`/`Split` and Esc in
/// `DetailFull`. Separate from `status_help_line` so Diagnostics is unaffected.
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
            spans.push(Span::styled("\u{2192}", Style::default().fg(Color::Yellow)));
            spans.push(Span::raw(" fullscreen  "));
        }
    }
    spans.push(Span::styled("j/k", Style::default().fg(Color::Yellow)));
    spans.push(Span::raw(" navigate  |  acuity: "));
    spans.push(Span::styled(text, Style::default().fg(color)));
    Line::from(spans)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use acuity_api::EventRecord;
    use acuity_schema::{
        AcuityEvent, AgentTurnCompleted, SessionIdle, ToolCallCompleted, ToolCallRequested,
    };

    fn make_record(seq: i64, event: AcuityEvent) -> EventRecord {
        EventRecord {
            seq,
            received_at: "2026-01-01T00:00:00Z".to_string(),
            event_type: event.event_type().to_string(),
            session_id: "s1".to_string(),
            turn_id: event.turn_id().map(str::to_string),
            project_dir: event.project_dir().to_string(),
            harness: event.harness().to_string(),
            payload: serde_json::to_string(&event).unwrap(),
        }
    }

    // --- event_summary ---

    #[test]
    fn event_summary_session_idle_with_title() {
        let record = make_record(
            1,
            AcuityEvent::SessionIdle(SessionIdle {
                session_id: "s1".to_string(),
                project_dir: "/my/project".to_string(),
                harness: "opencode".to_string(),
                session_title: Some("My Project".to_string()),
            }),
        );
        assert_eq!(event_summary(&record), "idle: My Project");
    }

    #[test]
    fn event_summary_session_idle_no_title() {
        let record = make_record(
            1,
            AcuityEvent::SessionIdle(SessionIdle {
                session_id: "s1".to_string(),
                project_dir: "/my/project".to_string(),
                harness: "opencode".to_string(),
                session_title: None,
            }),
        );
        assert_eq!(event_summary(&record), "idle: (no title)");
    }

    #[test]
    fn event_summary_agent_turn_completed() {
        let record = make_record(
            2,
            AcuityEvent::AgentTurnCompleted(AgentTurnCompleted {
                session_id: "s1".to_string(),
                turn_id: "t1".to_string(),
                project_dir: "/my/project".to_string(),
                harness: "opencode".to_string(),
                input_tokens: Some(100),
                output_tokens: Some(200),
                model: None,
            }),
        );
        // No model -> no appended segment.
        assert_eq!(event_summary(&record), "turn: in=100 out=200");
    }

    #[test]
    fn event_summary_agent_turn_completed_with_model() {
        let record = make_record(
            2,
            AcuityEvent::AgentTurnCompleted(AgentTurnCompleted {
                session_id: "s1".to_string(),
                turn_id: "t1".to_string(),
                project_dir: "/my/project".to_string(),
                harness: "opencode".to_string(),
                input_tokens: Some(100),
                output_tokens: Some(200),
                model: Some("anthropic/claude-sonnet".to_string()),
            }),
        );
        // Per-turn model appended as " · {model}".
        assert_eq!(
            event_summary(&record),
            "turn: in=100 out=200 \u{00b7} anthropic/claude-sonnet"
        );
    }

    #[test]
    fn event_summary_tool_call_requested() {
        let record = make_record(
            3,
            AcuityEvent::ToolCallRequested(ToolCallRequested {
                session_id: "s1".to_string(),
                turn_id: "t1".to_string(),
                project_dir: "/my/project".to_string(),
                harness: "opencode".to_string(),
                tool_call_id: "c1".to_string(),
                tool_name: "bash".to_string(),
                args: serde_json::Value::Null,
            }),
        );
        assert_eq!(event_summary(&record), "tool: bash");
    }

    #[test]
    fn event_summary_tool_call_completed_ok() {
        let record = make_record(
            4,
            AcuityEvent::ToolCallCompleted(ToolCallCompleted {
                session_id: "s1".to_string(),
                turn_id: "t1".to_string(),
                project_dir: "/my/project".to_string(),
                harness: "opencode".to_string(),
                tool_call_id: "c1".to_string(),
                tool_name: "bash".to_string(),
                is_error: false,
                error_text: None,
            }),
        );
        assert_eq!(event_summary(&record), "done: bash");
    }

    #[test]
    fn event_summary_tool_call_completed_error() {
        let record = make_record(
            5,
            AcuityEvent::ToolCallCompleted(ToolCallCompleted {
                session_id: "s1".to_string(),
                turn_id: "t1".to_string(),
                project_dir: "/my/project".to_string(),
                harness: "opencode".to_string(),
                tool_call_id: "c1".to_string(),
                tool_name: "bash".to_string(),
                is_error: true,
                error_text: Some("exit 1".to_string()),
            }),
        );
        assert_eq!(event_summary(&record), "ERR:  bash \u{2014} exit 1");
    }

    // --- is_diagnostic ---

    #[test]
    fn is_diagnostic_matches_tool_call_prefix_only() {
        assert!(is_diagnostic("tool_call_requested"));
        assert!(is_diagnostic("tool_call_completed"));
        assert!(!is_diagnostic("session_idle"));
        assert!(!is_diagnostic("agent_turn_completed"));
        assert!(!is_diagnostic("tool_other")); // "tool_" prefix not enough
    }

    // --- is_hidden_in_activity ---

    #[test]
    fn is_hidden_in_activity_hides_only_session_updated() {
        assert!(is_hidden_in_activity("session_updated"));
        assert!(!is_hidden_in_activity("session_idle"));
        assert!(!is_hidden_in_activity("agent_turn_completed"));
        assert!(!is_hidden_in_activity("tool_call_requested"));
        assert!(!is_hidden_in_activity("tool_call_completed"));
    }

    // --- session_label ---

    fn summary(title: Option<&str>) -> SessionSummary {
        SessionSummary {
            session_id: String::new(),
            project_dir: String::new(),
            session_title: title.map(str::to_string),
            first_seen: String::new(),
            last_seen: String::new(),
            input_tokens: 0,
            output_tokens: 0,
            error_count: 0,
            harness: String::new(),
            parent_id: None,
            agent: None,
            model: None,
        }
    }

    #[test]
    fn session_label_title_present() {
        let s = summary(Some("Build the activity feed"));
        let (label, is_placeholder) = session_label(Some(&s), "ses_0f14abc123");
        assert_eq!(label, "Build the activity feed");
        assert!(!is_placeholder);
    }

    #[test]
    fn session_label_title_none() {
        let s = summary(None);
        let (label, is_placeholder) = session_label(Some(&s), "ses_0f14abc123");
        // "ses_0f14abc123" is 14 chars; last 8 = "14abc123".
        assert_eq!(label, "\u{2026}14abc123");
        assert!(is_placeholder);
    }

    #[test]
    fn session_label_title_empty_string_is_placeholder() {
        // An empty title string should fall through to the id-suffix
        // placeholder (mirrors the plugin's `info.title || null` guard).
        let s = summary(Some(""));
        let (label, is_placeholder) = session_label(Some(&s), "ses_0f14abc123");
        assert_eq!(label, "\u{2026}14abc123");
        assert!(is_placeholder);
    }

    #[test]
    fn session_label_prefix_sharing_ids_are_distinct() {
        // Regression for ui.rs:214 — `.get(..8)` took the PREFIX, so two
        // opencode sessions sharing a per-run prefix (`ses_0f14...`) rendered
        // identically. The suffix-based label must distinguish them.
        let s = summary(None);
        let (a, _) = session_label(Some(&s), "ses_0f14aaaaaa");
        let (b, _) = session_label(Some(&s), "ses_0f14bbbbbb");
        assert_ne!(a, b, "distinct suffixes must yield distinct labels");
    }

    // --- format_datetime ---

    #[test]
    fn format_datetime_invalid_falls_back_to_first_10_chars() {
        let ts = "not-a-timestamp-but-long-enough";
        assert_eq!(format_datetime(ts), "not-a-time");
    }

    #[test]
    fn format_datetime_invalid_short_falls_back_to_full_string() {
        let ts = "bad";
        assert_eq!(format_datetime(ts), "bad");
    }

    #[test]
    fn format_datetime_past_date_is_yyyy_mm_dd() {
        let ts = "2020-01-15T09:30:00Z";
        let result = format_datetime(ts);
        // Other-day format is YYYY-MM-DD (10 chars, no time component).
        assert_eq!(result.len(), 10, "other-day format must be 10 chars: {result}");
        assert!(result.contains('-'), "other-day format must contain '-': {result}");
        assert!(!result.contains(':'), "other-day format must not contain ':': {result}");
    }

    // --- trunc_pad ---

    #[test]
    fn trunc_pad_short_string_is_padded() {
        assert_eq!(trunc_pad("cue", 10), "cue       ");
    }

    #[test]
    fn trunc_pad_exact_width_unchanged() {
        assert_eq!(trunc_pad("palekiwi", 8), "palekiwi");
    }

    #[test]
    fn trunc_pad_long_string_is_truncated() {
        assert_eq!(trunc_pad("palekiwi-labs-cue", 8), "palekiwi");
    }

    // --- format_tokens ---

    #[test]
    fn format_tokens_zero() {
        assert_eq!(format_tokens(0), "0");
    }

    #[test]
    fn format_tokens_small_no_comma() {
        assert_eq!(format_tokens(999), "999");
    }

    #[test]
    fn format_tokens_thousands() {
        assert_eq!(format_tokens(1_000), "1,000");
    }

    #[test]
    fn format_tokens_millions() {
        assert_eq!(format_tokens(1_234_567), "1,234,567");
    }

    // --- format_event_datetime ---

    #[test]
    fn format_event_datetime_invalid_falls_back_to_first_19_chars() {
        let ts = "not-a-timestamp-but-long-enough";
        assert_eq!(format_event_datetime(ts), "not-a-timestamp-but");
    }

    #[test]
    fn format_event_datetime_valid_has_date_and_time() {
        let ts = "2020-06-15T09:30:45Z";
        let result = format_event_datetime(ts);
        // YYYY-MM-DD HH:MM:SS = 19 chars; contains both '-' and ':'
        assert_eq!(result.len(), 19, "event datetime must be 19 chars: {result}");
        assert!(result.contains('-'));
        assert!(result.contains(':'));
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

    // --- project_basename ---

    #[test]
    fn project_basename_extracts_last_component() {
        assert_eq!(project_basename("/home/pl/code/palekiwi-labs/cue"), "cue");
    }

    #[test]
    fn project_basename_single_component() {
        assert_eq!(project_basename("cue"), "cue");
    }

    #[test]
    fn project_basename_trailing_slash_falls_back_to_full() {
        // rsplit('/').next() on a trailing-slash path yields "" (the empty
        // component after the last slash). filter(!empty) rejects it, so
        // unwrap_or falls back to the full project_dir string.
        assert_eq!(project_basename("/home/pl/code/"), "/home/pl/code/");
    }

    #[test]
    fn project_basename_root_slash_falls_back_to_full() {
        assert_eq!(project_basename("/"), "/");
    }
}

/// Format a UTC ISO-8601 timestamp string for display in the sessions pane.
///
/// Converts to the host local timezone. Returns:
/// - `"HH:MM:SS"` (8 chars) when the date is today
/// - `"YYYY-MM-DD"` (10 chars) for other days
///
/// Falls back to the first 10 chars of `ts` if parsing fails (or the full
/// string if it is shorter than 10 chars).
pub(crate) fn format_datetime(ts: &str) -> String {
    use chrono::{DateTime, Local, Utc};
    let Ok(dt_utc) = ts.parse::<DateTime<Utc>>() else {
        return ts.get(..10).unwrap_or(ts).to_string();
    };
    let local = dt_utc.with_timezone(&Local);
    let today = Local::now().date_naive();
    if local.date_naive() == today {
        local.format("%H:%M:%S").to_string()
    } else {
        local.format("%Y-%m-%d").to_string()
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

/// Truncate `s` to at most `width` chars and left-pad to exactly `width`.
///
/// Unlike `format!("{:<width$}")`, this also truncates strings that are
/// already longer than `width`, producing a fixed-width column.
pub(crate) fn trunc_pad(s: &str, width: usize) -> String {
    let char_count = s.chars().count();
    if char_count >= width {
        s.chars().take(width).collect()
    } else {
        format!("{s:<width$}")
    }
}

/// Format an integer with thousands-separator commas (e.g. `1234567` → `"1,234,567"`).
pub(crate) fn format_tokens(n: u64) -> String {
    let s = n.to_string();
    let bytes = s.as_bytes();
    let total = bytes.len();
    let mut result = String::with_capacity(total + total / 3);
    for (i, b) in bytes.iter().enumerate() {
        let remaining = total - i;
        if i > 0 && remaining.is_multiple_of(3) {
            result.push(',');
        }
        result.push(*b as char);
    }
    result
}

/// Format a UTC ISO-8601 timestamp for display in the events pane.
///
/// Converts to host local timezone and returns `"YYYY-MM-DD HH:MM:SS"` (19 chars).
/// Falls back to the first 19 chars of the raw string on parse error.
pub(crate) fn format_event_datetime(ts: &str) -> String {
    use chrono::{DateTime, Local, Utc};
    let Ok(dt_utc) = ts.parse::<DateTime<Utc>>() else {
        return ts.get(..19).unwrap_or(ts).to_string();
    };
    let local = dt_utc.with_timezone(&Local);
    local.format("%Y-%m-%d %H:%M:%S").to_string()
}

/// Collect unique agent identifiers for a session by scanning ring-buffer events.
///
/// Scans `session_updated` events (which carry the per-session agent field).
/// Order is first-seen within the ring buffer.
pub(crate) fn session_unique_agents(app: &App, session_id: &str) -> Vec<String> {
    let mut seen = std::collections::HashSet::new();
    let mut result = Vec::new();
    for ev in app
        .events
        .iter()
        .filter(|e| e.session_id == session_id && e.event_type == "session_updated")
    {
        if let Ok(AcuityEvent::SessionUpdated(e)) = serde_json::from_str(&ev.payload)
            && let Some(agent) = e.agent
            && seen.insert(agent.clone())
        {
            result.push(agent);
        }
    }
    // Fall back to SessionSummary.agent if no session_updated events in ring buffer.
    if result.is_empty()
        && let Some(s) = app.sessions.get(session_id)
        && let Some(a) = &s.agent
    {
        result.push(a.clone());
    }
    result
}

/// Collect unique model identifiers for a session by scanning ring-buffer events.
///
/// Scans `agent_turn_completed` events (which carry the per-turn resolved model).
/// Order is first-seen within the ring buffer.
pub(crate) fn session_unique_models(app: &App, session_id: &str) -> Vec<String> {
    let mut seen = std::collections::HashSet::new();
    let mut result = Vec::new();
    for ev in app.events.iter().filter(|e| {
        e.session_id == session_id && e.event_type == "agent_turn_completed"
    }) {
        if let Ok(AcuityEvent::AgentTurnCompleted(e)) = serde_json::from_str(&ev.payload)
            && let Some(model) = e.model
            && seen.insert(model.clone())
        {
            result.push(model);
        }
    }
    result
}

// ---------------------------------------------------------------------------
// Private helpers
// ---------------------------------------------------------------------------

/// Format the acuity connection status as `(text, Color)` for the status bar.
fn acuity_status_parts(status: &AcuityStatus) -> (String, Color) {
    match status {
        AcuityStatus::Connected => ("connected".to_string(), Color::Green),
        AcuityStatus::Reconnecting { attempt } => {
            (format!("reconnecting (attempt {attempt})"), Color::Yellow)
        }
        AcuityStatus::Disabled => ("disabled".to_string(), Color::DarkGray),
    }
}

/// Build the shared help/status bar line used in Activity and Diagnostics views.
fn status_help_line(status: &AcuityStatus) -> Line<'static> {
    let (text, color) = acuity_status_parts(status);
    Line::from(vec![
        Span::styled(" q", Style::default().fg(Color::Yellow)),
        Span::raw(" quit  "),
        Span::styled("1/2/3", Style::default().fg(Color::Yellow)),
        Span::raw(" views  "),
        Span::styled("j/k", Style::default().fg(Color::Yellow)),
        Span::raw(" navigate  |  acuity: "),
        Span::styled(text, Style::default().fg(color)),
    ])
}
