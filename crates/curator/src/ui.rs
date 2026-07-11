use crate::app::{AcuityStatus, ActivityPane, App, Column, SessionSummary, View};
use acuity_api::{AcuityEvent, EventRecord};
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, List, ListItem, ListState},
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
/// Two panes: Sessions (Pane 1, left 1/3) and Events (Pane 2, right 2/3).
/// When `app.pane_expanded`, the active pane fills the full area.
/// `Tab`/`Shift-Tab` switches panes; `z` toggles expand.
fn render_activity(frame: &mut Frame, app: &App) {
    let (view_area, help_area) = layout_with_help(frame.area());

    match (app.pane_expanded, app.active_activity_pane) {
        (true, ActivityPane::Sessions) => render_sessions_pane(frame, app, view_area),
        (true, ActivityPane::Events) => render_events_pane(frame, app, view_area),
        (false, _) => {
            let [sessions_area, events_area] = Layout::horizontal([
                Constraint::Ratio(1, 3),
                Constraint::Ratio(2, 3),
            ])
            .areas(view_area);
            render_sessions_pane(frame, app, sessions_area);
            render_events_pane(frame, app, events_area);
        }
    }

    frame.render_widget(activity_help_line(&app.acuity_status), help_area);
}

/// Render the Sessions pane (Pane 1 — left 1/3).
///
/// One row per session, sorted newest-first by `sorted_sessions()`.
/// Row format: `<project>  <harness>  <title-or-placeholder>`.
fn render_sessions_pane(frame: &mut Frame, app: &App, area: Rect) {
    let is_active = app.active_activity_pane == ActivityPane::Sessions;

    let sessions = app.sorted_sessions();

    // Find the visual index of the selected session.
    let sel_visual: Option<usize> = app
        .sel_session_id
        .as_deref()
        .and_then(|id| sessions.iter().position(|s| s.session_id == id));

    let items: Vec<ListItem> = sessions
        .iter()
        .map(|s| {
            let project = project_basename(&s.project_dir);
            let (label, is_placeholder) = session_label(Some(s), &s.session_id);
            let title_style = if is_placeholder {
                Style::default().fg(Color::DarkGray)
            } else {
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD)
            };
            let line = Line::from(vec![
                Span::styled(project.to_string(), Style::default().fg(Color::Magenta)),
                Span::raw("  "),
                Span::styled(s.harness.clone(), Style::default().fg(Color::Blue)),
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
        .highlight_style(highlight_style)
        .highlight_symbol("> ");

    let mut list_state = ListState::default();
    list_state.select(sel_visual);
    frame.render_stateful_widget(list, area, &mut list_state);
}

/// Render the Events pane (Pane 2 — right 2/3).
///
/// Shows events for the selected session, reverse-chrono, hiding
/// `session_updated` rows. Empty state shows a dim placeholder.
fn render_events_pane(frame: &mut Frame, app: &App, area: Rect) {
    let is_active = app.active_activity_pane == ActivityPane::Events;

    let sel_id = app.sel_session_id.as_deref();

    // Block title: " Events · <session label> ".
    let block_title = match sel_id {
        None => " Events ".to_string(),
        Some(id) => {
            let summary = app.sessions.get(id);
            let (label, _) = session_label(summary, id);
            format!(" Events \u{00b7} {label} ")
        }
    };

    let border_style = if is_active {
        Style::default()
            .fg(Color::Cyan)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(Color::DarkGray)
    };

    let highlight_style = if is_active {
        Style::default()
            .bg(Color::DarkGray)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default().add_modifier(Modifier::DIM)
    };

    // Collect visible events for the selected session, reverse-chrono.
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

    // Clamp selection to the visible list.
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
    frame.render_stateful_widget(list, area, &mut list_state);
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
/// Adds `Tab pane` and `z expand` hints that are specific to the two-pane
/// Activity layout. Separate from `status_help_line` so the Diagnostics
/// view's help bar is unaffected.
fn activity_help_line(status: &AcuityStatus) -> Line<'static> {
    let (text, color) = acuity_status_parts(status);
    Line::from(vec![
        Span::styled(" q", Style::default().fg(Color::Yellow)),
        Span::raw(" quit  "),
        Span::styled("1/2/3", Style::default().fg(Color::Yellow)),
        Span::raw(" views  "),
        Span::styled("Tab", Style::default().fg(Color::Yellow)),
        Span::raw(" pane  "),
        Span::styled("z", Style::default().fg(Color::Yellow)),
        Span::raw(" expand  "),
        Span::styled("j/k", Style::default().fg(Color::Yellow)),
        Span::raw(" navigate  |  acuity: "),
        Span::styled(text, Style::default().fg(color)),
    ])
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
