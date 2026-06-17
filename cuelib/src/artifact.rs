/// Canonical artifact types supported by cue out of the box.
pub const CANONICAL_TYPES: &[&str] = &["spec", "plan", "trace", "doc", "todo", "bin", "tmp", "ref"];

/// Default artifact types that are gitignored and not listed.
pub const DEFAULT_IGNORED_TYPES: &[&str] = &["tmp", "bin"];

/// Canonical status values for todo artifacts, in kanban column order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TodoStatus {
    Open,
    InProgress,
    Complete,
    /// Hidden in the kanban view.
    Closed,
    /// Hidden in the kanban view.
    Archived,
}

impl TodoStatus {
    /// Returns `true` if the status should be shown in the kanban board.
    pub fn is_kanban_visible(&self) -> bool {
        matches!(self, Self::Open | Self::InProgress | Self::Complete)
    }

    /// Return the canonical string representation.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Open => "open",
            Self::InProgress => "in-progress",
            Self::Complete => "complete",
            Self::Closed => "closed",
            Self::Archived => "archived",
        }
    }
}

impl std::str::FromStr for TodoStatus {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "open" => Ok(Self::Open),
            "in-progress" => Ok(Self::InProgress),
            "complete" => Ok(Self::Complete),
            "closed" => Ok(Self::Closed),
            "archived" => Ok(Self::Archived),
            _ => Err(()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn canonical_types_contains_expected() {
        assert!(CANONICAL_TYPES.contains(&"spec"));
        assert!(CANONICAL_TYPES.contains(&"plan"));
        assert!(CANONICAL_TYPES.contains(&"todo"));
        assert!(CANONICAL_TYPES.contains(&"bin"));
        assert_eq!(CANONICAL_TYPES.len(), 8);
    }

    #[test]
    fn default_ignored_types() {
        assert_eq!(DEFAULT_IGNORED_TYPES, &["tmp", "bin"]);
    }

    #[test]
    fn todo_status_kanban_visibility() {
        assert!(TodoStatus::Open.is_kanban_visible());
        assert!(TodoStatus::InProgress.is_kanban_visible());
        assert!(TodoStatus::Complete.is_kanban_visible());
        assert!(!TodoStatus::Closed.is_kanban_visible());
        assert!(!TodoStatus::Archived.is_kanban_visible());
    }

    #[test]
    fn todo_status_round_trip() {
        use std::str::FromStr;
        for (s, expected) in &[
            ("open", TodoStatus::Open),
            ("in-progress", TodoStatus::InProgress),
            ("complete", TodoStatus::Complete),
            ("closed", TodoStatus::Closed),
            ("archived", TodoStatus::Archived),
        ] {
            let parsed = TodoStatus::from_str(s).unwrap();
            assert_eq!(&parsed, expected);
            assert_eq!(parsed.as_str(), *s);
        }
    }

    #[test]
    fn todo_status_unknown_returns_err() {
        use std::str::FromStr;
        assert!(TodoStatus::from_str("unknown").is_err());
        assert!(TodoStatus::from_str("").is_err());
    }
}
