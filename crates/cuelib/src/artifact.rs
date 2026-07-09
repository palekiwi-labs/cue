use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::fs;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::str::FromStr;

const FRONTMATTER_MAX_LINES: usize = 64;

/// Canonical artifact types supported by cue out of the box.
pub const CANONICAL_TYPES: &[&str] = &[
    "spec", "plan", "trace", "doc", "todo", "bin", "tmp", "ref", "task", "note",
];

/// Default artifact types that are gitignored and not listed.
pub const DEFAULT_IGNORED_TYPES: &[&str] = &["tmp", "ref"];

/// Canonical status values for todo artifacts, in kanban column order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TodoStatus {
    Open,
    InProgress,
    Complete,
    /// Hidden in the kanban view.
    Closed,
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
            _ => Err(()),
        }
    }
}

/// Canonical status values for task artifacts, in kanban column order.
///
/// Tasks do not use `archived`; that status is reserved for `todo` artifacts.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TaskStatus {
    Open,
    InProgress,
    Complete,
    /// Hidden in the kanban view.
    Closed,
}

impl TaskStatus {
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
        }
    }
}

impl std::str::FromStr for TaskStatus {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "open" => Ok(Self::Open),
            "in-progress" => Ok(Self::InProgress),
            "complete" => Ok(Self::Complete),
            "closed" => Ok(Self::Closed),
            _ => Err(()),
        }
    }
}

/// Canonical status values for note artifacts.
///
/// A note has no `complete` status: it does not finish, it *dissolves* into
/// its outcome (a `task`, `spec`, `doc`, etc.) and is then `closed`. Notes are
/// conversational, not work items, so they are never kanban-visible.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NoteStatus {
    Open,
    InProgress,
    /// Terminal state: the conversation concluded; the outcome now lives in a
    /// different artifact. The note itself is deletable.
    Closed,
}

impl NoteStatus {
    /// Returns `false` unconditionally: notes are never kanban work items.
    pub fn is_kanban_visible(&self) -> bool {
        false
    }

    /// Return the canonical string representation.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Open => "open",
            Self::InProgress => "in-progress",
            Self::Closed => "closed",
        }
    }
}

impl std::str::FromStr for NoteStatus {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "open" => Ok(Self::Open),
            "in-progress" => Ok(Self::InProgress),
            "closed" => Ok(Self::Closed),
            _ => Err(()),
        }
    }
}

// ── File utilities ────────────────────────────────────────────────────────────

/// Walk `dir` recursively and return all file paths.
///
/// Returns an empty `Vec` if `dir` does not exist or is not a directory.
pub fn collect_files(dir: &Path) -> Result<Vec<PathBuf>> {
    if !dir.is_dir() {
        return Ok(vec![]);
    }
    fs::read_dir(dir)?
        .map(|entry| -> Result<Vec<PathBuf>> {
            let path = entry?.path();
            if path.is_dir() {
                collect_files(&path)
            } else {
                Ok(vec![path])
            }
        })
        .collect::<Result<Vec<_>>>()
        .map(|v| v.into_iter().flatten().collect())
}

/// Extract the raw YAML block from between the opening and closing `---`
/// fences of `path`.
///
/// Returns `None` if the file cannot be opened, has no frontmatter, or the
/// closing fence is missing within `FRONTMATTER_MAX_LINES`.
pub fn extract_frontmatter_yaml(path: &Path) -> Option<String> {
    let file = fs::File::open(path).ok()?;
    let mut reader = BufReader::new(file);
    let mut line = String::new();

    // First line must be exactly "---" (no leading or trailing whitespace)
    reader.read_line(&mut line).ok()?;
    if line.trim() != "---" {
        return None;
    }

    let mut yaml = String::new();
    for _ in 0..FRONTMATTER_MAX_LINES {
        line.clear();
        let n = reader.read_line(&mut line).ok()?;
        if n == 0 {
            return None; // EOF before closing fence — malformed
        }
        if line.trim() == "---" {
            return Some(yaml);
        }
        yaml.push_str(&line);
    }

    None // Exceeded line budget — treat as malformed
}

// ── Artifact reader ───────────────────────────────────────────────────────────

/// Subset of frontmatter fields relevant to the kanban view.
#[derive(Deserialize, Default)]
struct RawFrontmatter {
    title: Option<String>,
    status: Option<String>,
    priority: Option<String>,
}

/// Metadata extracted from a single `.cue` artifact file.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ArtifactMeta {
    /// Display title: `title:` frontmatter field, or filename stem as fallback.
    pub title: String,
    /// Raw `status` string (e.g. `"open"`, `"in-progress"`). `None` if absent.
    pub status_raw: Option<String>,
    /// Raw `priority` string (e.g. `"normal"`, `"high"`). `None` if absent.
    pub priority_raw: Option<String>,
    /// Artifact type (`"task"`, `"plan"`, `"todo"`, etc.).
    pub artifact_type: String,
    /// Absolute path to the file.
    pub path: PathBuf,
}

impl ArtifactMeta {
    /// Parse `status_raw` into a typed status enum.
    ///
    /// Returns `None` if the field is absent or is not a recognised value
    /// for `T`. The caller supplies the target type via turbofish:
    ///
    /// ```ignore
    /// let status: Option<TaskStatus> = artifact.status::<TaskStatus>();
    /// ```
    pub fn status<T: FromStr>(&self) -> Option<T> {
        self.status_raw.as_deref()?.parse().ok()
    }
}

/// Read all artifacts of `artifact_type` from `.cue/<branch>/<artifact_type>/`
/// under `root`.
///
/// Only `.md` files are included. Results are sorted by path for
/// deterministic ordering. Returns an empty `Vec` if the directory does
/// not exist.
pub fn read_artifacts(root: &Path, branch: &str, artifact_type: &str) -> Result<Vec<ArtifactMeta>> {
    let dir = root.join(".cue").join(branch).join(artifact_type);
    // collect_files already guards with is_dir(); delegate rather than duplicating.
    let mut files = collect_files(&dir)?;
    files.sort();

    let mut result = Vec::new();
    for path in files {
        if path.extension().and_then(|e| e.to_str()) != Some("md") {
            continue;
        }

        let fm: RawFrontmatter = extract_frontmatter_yaml(&path)
            .and_then(|yaml| serde_yaml::from_str(&yaml).ok())
            .unwrap_or_default();

        let title = fm.title.unwrap_or_else(|| {
            path.file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("untitled")
                .to_string()
        });

        result.push(ArtifactMeta {
            title,
            status_raw: fm.status,
            priority_raw: fm.priority,
            artifact_type: artifact_type.to_string(),
            path,
        });
    }
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use std::str::FromStr;
    use tempfile::{NamedTempFile, TempDir};

    // ── Constants ─────────────────────────────────────────────────────────────

    #[test]
    fn canonical_types_contains_expected() {
        assert!(CANONICAL_TYPES.contains(&"spec"));
        assert!(CANONICAL_TYPES.contains(&"plan"));
        assert!(CANONICAL_TYPES.contains(&"todo"));
        assert!(CANONICAL_TYPES.contains(&"bin"));
        assert!(CANONICAL_TYPES.contains(&"task"));
        assert!(CANONICAL_TYPES.contains(&"note"));
        assert_eq!(CANONICAL_TYPES.len(), 10);
    }

    #[test]
    fn default_ignored_types() {
        assert_eq!(DEFAULT_IGNORED_TYPES, &["tmp", "ref"]);
    }

    // ── TodoStatus ────────────────────────────────────────────────────────────

    #[test]
    fn todo_status_kanban_visibility() {
        assert!(TodoStatus::Open.is_kanban_visible());
        assert!(TodoStatus::InProgress.is_kanban_visible());
        assert!(TodoStatus::Complete.is_kanban_visible());
        assert!(!TodoStatus::Closed.is_kanban_visible());
    }

    #[test]
    fn todo_status_round_trip() {
        for (s, expected) in &[
            ("open", TodoStatus::Open),
            ("in-progress", TodoStatus::InProgress),
            ("complete", TodoStatus::Complete),
            ("closed", TodoStatus::Closed),
        ] {
            let parsed = TodoStatus::from_str(s).unwrap();
            assert_eq!(&parsed, expected);
            assert_eq!(parsed.as_str(), *s);
        }
    }

    #[test]
    fn todo_status_unknown_returns_err() {
        assert!(TodoStatus::from_str("unknown").is_err());
        assert!(TodoStatus::from_str("").is_err());
    }

    // ── TaskStatus ────────────────────────────────────────────────────────────

    #[test]
    fn task_status_kanban_visibility() {
        assert!(TaskStatus::Open.is_kanban_visible());
        assert!(TaskStatus::InProgress.is_kanban_visible());
        assert!(TaskStatus::Complete.is_kanban_visible());
        assert!(!TaskStatus::Closed.is_kanban_visible());
    }

    #[test]
    fn task_status_round_trip() {
        for (s, expected) in &[
            ("open", TaskStatus::Open),
            ("in-progress", TaskStatus::InProgress),
            ("complete", TaskStatus::Complete),
            ("closed", TaskStatus::Closed),
        ] {
            let parsed = TaskStatus::from_str(s).unwrap();
            assert_eq!(&parsed, expected);
            assert_eq!(parsed.as_str(), *s);
        }
    }

    #[test]
    fn task_status_unknown_returns_err() {
        assert!(TaskStatus::from_str("archived").is_err());
        assert!(TaskStatus::from_str("unknown").is_err());
        assert!(TaskStatus::from_str("").is_err());
    }

    #[test]
    fn status_archived_is_invalid() {
        // `archived` is not a status; it should be an orthogonal flag if ever implemented.
        assert!(TaskStatus::from_str("archived").is_err());
        assert!(TodoStatus::from_str("archived").is_err());
    }

    // ── NoteStatus ────────────────────────────────────────────────────────────

    #[test]
    fn note_status_is_never_kanban_visible() {
        // Notes are not work items; they never appear on the kanban board.
        assert!(!NoteStatus::Open.is_kanban_visible());
        assert!(!NoteStatus::InProgress.is_kanban_visible());
        assert!(!NoteStatus::Closed.is_kanban_visible());
    }

    #[test]
    fn note_status_round_trip() {
        for (s, expected) in &[
            ("open", NoteStatus::Open),
            ("in-progress", NoteStatus::InProgress),
            ("closed", NoteStatus::Closed),
        ] {
            let parsed = NoteStatus::from_str(s).unwrap();
            assert_eq!(&parsed, expected);
            assert_eq!(parsed.as_str(), *s);
        }
    }

    #[test]
    fn note_status_unknown_returns_err() {
        assert!(NoteStatus::from_str("complete").is_err());
        assert!(NoteStatus::from_str("archived").is_err());
        assert!(NoteStatus::from_str("unknown").is_err());
        assert!(NoteStatus::from_str("").is_err());
    }

    // ── collect_files ─────────────────────────────────────────────────────────

    #[test]
    fn collect_files_returns_empty_for_missing_dir() {
        let dir = TempDir::new().unwrap();
        let missing = dir.path().join("nonexistent");
        let files = collect_files(&missing).unwrap();
        assert!(files.is_empty());
    }

    #[test]
    fn collect_files_finds_nested_files() {
        let dir = TempDir::new().unwrap();
        let sub = dir.path().join("sub");
        fs::create_dir_all(&sub).unwrap();
        fs::write(dir.path().join("a.md"), "").unwrap();
        fs::write(sub.join("b.md"), "").unwrap();

        let mut files = collect_files(dir.path()).unwrap();
        files.sort();
        assert_eq!(files.len(), 2);
        assert!(files.iter().any(|p| p.ends_with("a.md")));
        assert!(files.iter().any(|p| p.ends_with("b.md")));
    }

    #[test]
    fn collect_files_returns_empty_for_dir_with_only_subdirs() {
        let dir = TempDir::new().unwrap();
        fs::create_dir_all(dir.path().join("sub1")).unwrap();
        fs::create_dir_all(dir.path().join("sub2")).unwrap();
        let files = collect_files(dir.path()).unwrap();
        assert!(files.is_empty());
    }

    // ── extract_frontmatter_yaml ──────────────────────────────────────────────

    #[test]
    fn frontmatter_extracted_from_valid_file() {
        let mut f = NamedTempFile::new().unwrap();
        writeln!(f, "---").unwrap();
        writeln!(f, "status: open").unwrap();
        writeln!(f, "---").unwrap();
        writeln!(f, "# Body").unwrap();

        let yaml = extract_frontmatter_yaml(f.path()).unwrap();
        assert!(yaml.contains("status: open"));
    }

    #[test]
    fn frontmatter_returns_none_for_file_without_fence() {
        let mut f = NamedTempFile::new().unwrap();
        writeln!(f, "# Just a header").unwrap();
        assert!(extract_frontmatter_yaml(f.path()).is_none());
    }

    #[test]
    fn frontmatter_returns_none_for_unclosed_fence() {
        let mut f = NamedTempFile::new().unwrap();
        writeln!(f, "---").unwrap();
        writeln!(f, "status: open").unwrap();
        // no closing ---
        assert!(extract_frontmatter_yaml(f.path()).is_none());
    }

    #[test]
    fn frontmatter_returns_none_for_empty_file() {
        let f = NamedTempFile::new().unwrap();
        // file is empty — first read_line returns Ok(0), trim() gives ""
        assert!(extract_frontmatter_yaml(f.path()).is_none());
    }

    // ── ArtifactMeta::status ──────────────────────────────────────────────────

    #[test]
    fn artifact_meta_status_accessor_parses_task_status() {
        let meta = ArtifactMeta {
            title: "t".into(),
            status_raw: Some("in-progress".into()),
            priority_raw: None,
            artifact_type: "task".into(),
            path: PathBuf::from("fake.md"),
        };
        assert_eq!(meta.status::<TaskStatus>(), Some(TaskStatus::InProgress));
    }

    #[test]
    fn artifact_meta_status_accessor_returns_none_when_absent() {
        let meta = ArtifactMeta {
            title: "t".into(),
            status_raw: None,
            priority_raw: None,
            artifact_type: "task".into(),
            path: PathBuf::from("fake.md"),
        };
        assert_eq!(meta.status::<TaskStatus>(), None);
    }

    #[test]
    fn artifact_meta_status_accessor_returns_none_for_unknown_value() {
        let meta = ArtifactMeta {
            title: "t".into(),
            status_raw: Some("bogus".into()),
            priority_raw: None,
            artifact_type: "task".into(),
            path: PathBuf::from("fake.md"),
        };
        assert_eq!(meta.status::<TaskStatus>(), None);
    }

    // ── read_artifacts ────────────────────────────────────────────────────────

    fn make_artifact(dir: &Path, filename: &str, content: &str) {
        fs::write(dir.join(filename), content).unwrap();
    }

    #[test]
    fn read_artifacts_returns_empty_when_dir_absent() {
        let root = TempDir::new().unwrap();
        let result = read_artifacts(root.path(), "master", "task").unwrap();
        assert!(result.is_empty());
    }

    #[test]
    fn read_artifacts_parses_task_frontmatter() {
        let root = TempDir::new().unwrap();
        let task_dir = root.path().join(".cue").join("master").join("task");
        fs::create_dir_all(&task_dir).unwrap();
        make_artifact(
            &task_dir,
            "my-task.md",
            "---\ntitle: \"Do the thing\"\nstatus: open\npriority: high\n---\n# Body\n",
        );

        let artifacts = read_artifacts(root.path(), "master", "task").unwrap();
        assert_eq!(artifacts.len(), 1);
        let a = &artifacts[0];
        assert_eq!(a.title, "Do the thing");
        assert_eq!(a.status_raw.as_deref(), Some("open"));
        assert_eq!(a.priority_raw.as_deref(), Some("high"));
        assert_eq!(a.artifact_type, "task");
        assert_eq!(a.status::<TaskStatus>(), Some(TaskStatus::Open));
    }

    #[test]
    fn read_artifacts_uses_filename_stem_when_title_absent() {
        let root = TempDir::new().unwrap();
        let task_dir = root.path().join(".cue").join("master").join("task");
        fs::create_dir_all(&task_dir).unwrap();
        make_artifact(
            &task_dir,
            "no-title.md",
            "---\nstatus: in-progress\n---\n# Body\n",
        );

        let artifacts = read_artifacts(root.path(), "master", "task").unwrap();
        assert_eq!(artifacts[0].title, "no-title");
    }

    #[test]
    fn read_artifacts_skips_non_md_files() {
        let root = TempDir::new().unwrap();
        let task_dir = root.path().join(".cue").join("master").join("task");
        fs::create_dir_all(&task_dir).unwrap();
        make_artifact(&task_dir, "task.md", "---\ntitle: \"Real\"\n---\n");
        make_artifact(&task_dir, "task.log", "not markdown");

        let artifacts = read_artifacts(root.path(), "master", "task").unwrap();
        assert_eq!(artifacts.len(), 1);
        assert_eq!(artifacts[0].title, "Real");
    }

    #[test]
    fn read_artifacts_handles_missing_frontmatter() {
        let root = TempDir::new().unwrap();
        let task_dir = root.path().join(".cue").join("master").join("task");
        fs::create_dir_all(&task_dir).unwrap();
        make_artifact(&task_dir, "bare.md", "# Just a heading\n");

        let artifacts = read_artifacts(root.path(), "master", "task").unwrap();
        assert_eq!(artifacts.len(), 1);
        assert_eq!(artifacts[0].title, "bare");
        assert!(artifacts[0].status_raw.is_none());
        assert!(artifacts[0].priority_raw.is_none());
    }

    #[test]
    fn read_artifacts_handles_invalid_yaml_frontmatter() {
        // Syntactically broken YAML must not fail the call; the artifact
        // survives with None status and title falling back to filename stem.
        let root = TempDir::new().unwrap();
        let task_dir = root.path().join(".cue").join("master").join("task");
        fs::create_dir_all(&task_dir).unwrap();
        make_artifact(&task_dir, "broken.md", "---\nstatus: [unclosed\n---\n");

        let artifacts = read_artifacts(root.path(), "master", "task").unwrap();
        assert_eq!(artifacts.len(), 1);
        assert_eq!(artifacts[0].title, "broken");
        assert!(artifacts[0].status_raw.is_none());
    }

    #[test]
    fn read_artifacts_walks_nested_timestamp_dirs() {
        let root = TempDir::new().unwrap();
        let nested = root
            .path()
            .join(".cue")
            .join("master")
            .join("task")
            .join("1700000000-abc1234");
        fs::create_dir_all(&nested).unwrap();
        make_artifact(
            &nested,
            "nested-task.md",
            "---\ntitle: \"Nested\"\nstatus: complete\n---\n",
        );

        let artifacts = read_artifacts(root.path(), "master", "task").unwrap();
        assert_eq!(artifacts.len(), 1);
        assert_eq!(artifacts[0].title, "Nested");
        assert_eq!(artifacts[0].status_raw.as_deref(), Some("complete"));
    }

    #[test]
    fn read_artifacts_results_are_sorted_by_path() {
        let root = TempDir::new().unwrap();
        let task_dir = root.path().join(".cue").join("master").join("task");
        fs::create_dir_all(&task_dir).unwrap();
        make_artifact(&task_dir, "zzz.md", "---\ntitle: \"Z\"\n---\n");
        make_artifact(&task_dir, "aaa.md", "---\ntitle: \"A\"\n---\n");

        let artifacts = read_artifacts(root.path(), "master", "task").unwrap();
        assert_eq!(artifacts[0].title, "A");
        assert_eq!(artifacts[1].title, "Z");
    }
}
