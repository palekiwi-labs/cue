use crate::app::{App, Column};
use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, List, ListItem, ListState},
};

/// Colour scheme for priority badges.
fn priority_colour(priority: Option<&str>) -> Color {
    match priority {
        Some("critical") => Color::Red,
        Some("high") => Color::Yellow,
        Some("low") => Color::DarkGray,
        _ => Color::Gray, // normal or absent
    }
}

/// Render the full kanban board.
pub fn render(frame: &mut Frame, app: &App) {
    let area = frame.area();

    // Help bar at the bottom (1 line).
    let [board_area, help_area] = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(0), Constraint::Length(1)])
        .areas(area);

    // Three equal columns.
    let [open_area, in_progress_area, complete_area] = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Ratio(1, 3),
            Constraint::Ratio(1, 3),
            Constraint::Ratio(1, 3),
        ])
        .areas(board_area);

    let cols = [
        (Column::Open, open_area),
        (Column::InProgress, in_progress_area),
        (Column::Complete, complete_area),
    ];

    for (col, col_area) in cols {
        render_column(frame, app, col, col_area);
    }

    // Help bar.
    let help = Line::from(vec![
        Span::styled(" q", Style::default().fg(Color::Yellow)),
        Span::raw(" quit  "),
        Span::styled("h/l ← →", Style::default().fg(Color::Yellow)),
        Span::raw(" switch column  "),
        Span::styled("j/k ↑ ↓", Style::default().fg(Color::Yellow)),
        Span::raw(" navigate"),
    ]);
    frame.render_widget(help, help_area);
}

fn render_column(frame: &mut Frame, app: &App, col: Column, area: ratatui::layout::Rect) {
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
                Span::raw(task.title.clone()),
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
