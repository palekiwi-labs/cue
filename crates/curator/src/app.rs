use cuelib::artifact::{ArtifactMeta, TaskStatus};

/// Which kanban column is currently active.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Column {
    Open,
    InProgress,
    Complete,
}

impl Column {
    pub fn title(&self) -> &'static str {
        match self {
            Self::Open => "Open",
            Self::InProgress => "In Progress",
            Self::Complete => "Complete",
        }
    }

    pub fn left(&self) -> Self {
        match self {
            Self::Open => Self::Open,
            Self::InProgress => Self::Open,
            Self::Complete => Self::InProgress,
        }
    }

    pub fn right(&self) -> Self {
        match self {
            Self::Open => Self::InProgress,
            Self::InProgress => Self::Complete,
            Self::Complete => Self::Complete,
        }
    }
}

/// Application state for the curator kanban board.
pub struct App {
    /// Tasks in the Open column.
    pub open: Vec<ArtifactMeta>,
    /// Tasks in the In Progress column.
    pub in_progress: Vec<ArtifactMeta>,
    /// Tasks in the Complete column.
    pub complete: Vec<ArtifactMeta>,

    /// Active (focused) column.
    pub active_col: Column,

    /// Selected item index within each column.
    pub sel_open: usize,
    pub sel_in_progress: usize,
    pub sel_complete: usize,
}

impl App {
    /// Classify `tasks` into kanban columns using the typed `TaskStatus` from
    /// `cuelib`. Tasks whose status parses as `Closed`, or whose status field
    /// is absent / unrecognised, are silently excluded (they are not kanban-
    /// visible by definition).
    pub fn new(tasks: Vec<ArtifactMeta>) -> Self {
        let mut open = Vec::new();
        let mut in_progress = Vec::new();
        let mut complete = Vec::new();

        for task in tasks {
            match task.status::<TaskStatus>() {
                Some(TaskStatus::Open) => open.push(task),
                Some(TaskStatus::InProgress) => in_progress.push(task),
                Some(TaskStatus::Complete) => complete.push(task),
                // Closed or unrecognised — not kanban-visible.
                Some(TaskStatus::Closed) | None => {}
            }
        }

        Self {
            open,
            in_progress,
            complete,
            active_col: Column::Open,
            sel_open: 0,
            sel_in_progress: 0,
            sel_complete: 0,
        }
    }

    /// Move the selection down within the active column.
    pub fn scroll_down(&mut self) {
        match self.active_col {
            Column::Open => {
                let len = self.open.len();
                if len > 0 {
                    self.sel_open = (self.sel_open + 1).min(len - 1);
                }
            }
            Column::InProgress => {
                let len = self.in_progress.len();
                if len > 0 {
                    self.sel_in_progress = (self.sel_in_progress + 1).min(len - 1);
                }
            }
            Column::Complete => {
                let len = self.complete.len();
                if len > 0 {
                    self.sel_complete = (self.sel_complete + 1).min(len - 1);
                }
            }
        }
    }

    /// Move the selection up within the active column.
    pub fn scroll_up(&mut self) {
        match self.active_col {
            Column::Open => {
                self.sel_open = self.sel_open.saturating_sub(1);
            }
            Column::InProgress => {
                self.sel_in_progress = self.sel_in_progress.saturating_sub(1);
            }
            Column::Complete => {
                self.sel_complete = self.sel_complete.saturating_sub(1);
            }
        }
    }

    /// Switch the active column to the left.
    pub fn move_left(&mut self) {
        self.active_col = self.active_col.left();
    }

    /// Switch the active column to the right.
    pub fn move_right(&mut self) {
        self.active_col = self.active_col.right();
    }

    /// Return the tasks for a given column.
    pub fn column_tasks(&self, col: Column) -> &[ArtifactMeta] {
        match col {
            Column::Open => &self.open,
            Column::InProgress => &self.in_progress,
            Column::Complete => &self.complete,
        }
    }

    /// Return the current selection index for a given column.
    pub fn column_sel(&self, col: Column) -> usize {
        match col {
            Column::Open => self.sel_open,
            Column::InProgress => self.sel_in_progress,
            Column::Complete => self.sel_complete,
        }
    }
}
