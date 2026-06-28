use crate::app::{AcuityStatus, App, Column, View};
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
// Activity Feed view
// ---------------------------------------------------------------------------

/// Render the Activity Feed (View 2).
fn render_activity(frame: &mut Frame, app: &App) {
    let (list_area, help_area) = layout_with_help(frame.area());

    let mut items: Vec<ListItem> = Vec::new();
    let mut selected_visual: Option<usize> = None;
    let mut prev_session: Option<String> = None;

    for (idx, record) in app.events.iter().rev().enumerate() {
        // Inject a session header whenever the session_id changes.
        if prev_session.as_deref() != Some(record.session_id.as_str()) {
            let header = session_header(record, app);
            items.push(ListItem::new(Line::from(Span::styled(
                header,
                Style::default().fg(Color::DarkGray),
            ))));
            prev_session = Some(record.session_id.clone());
        }

        // Map the event index to the visual list position (accounting for
        // injected header rows).
        if idx == app.sel_activity {
            selected_visual = Some(items.len());
        }

        let ts = record
            .received_at
            .get(..19)
            .unwrap_or(record.received_at.as_str());
        let summary = event_summary(record);
        let line = Line::from(vec![
            Span::styled(format!(" {ts}  "), Style::default().fg(Color::DarkGray)),
            Span::styled(
                format!("{:<24}", record.event_type),
                Style::default().fg(Color::White),
            ),
            Span::raw(format!("  {summary}")),
        ]);
        items.push(ListItem::new(line));
    }

    let list = List::new(items)
        .block(
            Block::default()
                .title(" Activity Feed ")
                .borders(Borders::ALL),
        )
        .highlight_style(
            Style::default()
                .bg(Color::DarkGray)
                .add_modifier(Modifier::BOLD),
        )
        .highlight_symbol("> ");

    let mut list_state = ListState::default();
    list_state.select(selected_visual);
    frame.render_stateful_widget(list, list_area, &mut list_state);

    frame.render_widget(status_help_line(&app.acuity_status), help_area);
}

/// Build the session group header line for the Activity Feed.
fn session_header(record: &EventRecord, app: &App) -> String {
    let short_id = record.session_id.get(..8).unwrap_or(&record.session_id);
    let project = app
        .sessions
        .get(&record.session_id)
        .and_then(|s| {
            if s.project_dir.is_empty() {
                None
            } else {
                // Basename of the project_dir path.
                Some(
                    s.project_dir
                        .rsplit('/')
                        .next()
                        .unwrap_or(s.project_dir.as_str())
                        .to_string(),
                )
            }
        })
        .unwrap_or_else(|| short_id.to_string());
    format!(" \u{2500}\u{2500} {project} ({short_id}) \u{2500}\u{2500}")
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
                format!("turn: in={input} out={output}")
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
            }),
        );
        assert_eq!(event_summary(&record), "turn: in=100 out=200");
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
