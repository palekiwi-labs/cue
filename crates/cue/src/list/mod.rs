use anyhow::Result;
use cuelib::artifact::{collect_files, extract_frontmatter_yaml};
use cuelib::store;
use serde::Serialize;
use std::path::{Path, PathBuf};
use std::str::FromStr;

// ── Artifact addressing ──────────────────────────────────────────────────────

/// The artifact types cue recognises.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ArtifactType {
    Task,
    Spec,
    Plan,
    Note,
    Trace,
    Review,
    Bin,
    Tmp,
}

impl ArtifactType {
    fn from_segment(segment: &str) -> Option<Self> {
        Some(match segment {
            "task" => Self::Task,
            "spec" => Self::Spec,
            "plan" => Self::Plan,
            "note" => Self::Note,
            "trace" => Self::Trace,
            "review" => Self::Review,
            "bin" => Self::Bin,
            "tmp" => Self::Tmp,
            _ => return None,
        })
    }

    fn as_str(self) -> &'static str {
        match self {
            Self::Task => "task",
            Self::Spec => "spec",
            Self::Plan => "plan",
            Self::Note => "note",
            Self::Trace => "trace",
            Self::Review => "review",
            Self::Bin => "bin",
            Self::Tmp => "tmp",
        }
    }
}

/// A store path split on cue's single addressing rule,
/// `<scope root>/<context>/<artifact type>/<caller path>`.
struct ArtifactAddress {
    context: String,
    cue_type: ArtifactType,
    /// Everything the caller named below the type, which may be nested.
    name: String,
}

/// Read an artifact's address out of its location under the scope root.
///
/// Returns `None` for any path that does not satisfy the rule, so a directory
/// naming no known type is simply not an artifact and is ignored. Only the
/// segment directly below the context names the type: artifacts may be grouped
/// into subdirectories (`review/rounds/first.json`), and a directory further
/// down may itself be named after a type (`note/review/decisions.md`).
fn artifact_address(path: &Path, scope_root: &Path) -> Option<ArtifactAddress> {
    let relative = path.strip_prefix(scope_root).ok()?;
    let mut components = relative.components();

    let context = components.next()?.as_os_str().to_string_lossy();
    let cue_type = ArtifactType::from_segment(&components.next()?.as_os_str().to_string_lossy())?;
    let name = components.collect::<PathBuf>();
    if name.as_os_str().is_empty() {
        return None;
    }

    Some(ArtifactAddress {
        context: context.into_owned(),
        cue_type,
        name: name.to_string_lossy().into_owned(),
    })
}

// ── Frontmatter filter ───────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq)]
enum FilterOp {
    Eq,
    NotEq,
    Contains,
}

#[derive(Debug, Clone)]
pub struct Filter {
    /// Dot-separated key path into the frontmatter object (e.g. `["meta", "status"]`).
    path: Vec<String>,
    op: FilterOp,
    /// Right-hand side, pre-coerced to a JSON scalar.
    rhs: serde_json::Value,
}

impl FromStr for Filter {
    type Err = String;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        // Check operators longest-first so "!=" is not swallowed by "=".
        let (op, sep) = if s.contains("!=") {
            (FilterOp::NotEq, "!=")
        } else if s.contains("~=") {
            (FilterOp::Contains, "~=")
        } else if s.contains('=') {
            (FilterOp::Eq, "=")
        } else {
            return Err(format!(
                "no operator found in {:?}; supported operators: =, !=, ~=",
                s
            ));
        };

        let mut parts = s.splitn(2, sep);
        let key = parts.next().unwrap().trim();
        let val = parts.next().unwrap_or("").trim();

        if key.is_empty() {
            return Err("filter key cannot be empty".into());
        }

        // Coerce RHS: numbers and booleans become JSON scalars; everything else is a string.
        let rhs = serde_json::from_str(val)
            .unwrap_or_else(|_| serde_json::Value::String(val.to_string()));

        Ok(Filter {
            path: key.split('.').map(str::to_string).collect(),
            op,
            rhs,
        })
    }
}

/// Walk a dot-separated path into a JSON value.
fn get_nested<'a>(value: &'a serde_json::Value, path: &[String]) -> Option<&'a serde_json::Value> {
    path.iter().try_fold(value, |v, key| v.get(key))
}

fn evaluate_filter(filter: &Filter, fm: &serde_json::Value) -> bool {
    let actual = get_nested(fm, &filter.path);
    match (&filter.op, actual) {
        (FilterOp::Eq, None) => false,
        (FilterOp::NotEq, None) => true,
        (FilterOp::Contains, None) => false,
        (FilterOp::Eq, Some(v)) => v == &filter.rhs,
        (FilterOp::NotEq, Some(v)) => v != &filter.rhs,
        (FilterOp::Contains, Some(v)) => match (v.as_str(), filter.rhs.as_str()) {
            (Some(haystack), Some(needle)) => haystack.contains(needle),
            _ => false,
        },
    }
}

/// Parse queryable artifact metadata, or `Null` if absent or malformed.
///
/// Where the metadata lives is a property of the type, so the type decides how
/// to decode it.
fn parse_metadata(path: &Path, cue_type: ArtifactType) -> serde_json::Value {
    use ArtifactType::*;
    match cue_type {
        // A review file *is* a JSON object, so the document is its metadata.
        Review => std::fs::File::open(path)
            .ok()
            .and_then(|file| serde_json::from_reader(file).ok())
            .unwrap_or(serde_json::Value::Null),
        // Markdown artifacts declare their metadata in YAML frontmatter.
        Task | Spec | Plan | Note | Trace => extract_frontmatter_yaml(path)
            .and_then(|yaml| serde_yaml::from_str(&yaml).ok())
            .unwrap_or(serde_json::Value::Null),
        // `bin` and `tmp` hold opaque content: there is nothing to decode.
        Bin | Tmp => serde_json::Value::Null,
    }
}

/// Returns `true` if `fm` satisfies every filter (AND semantics).
/// `Null` (no frontmatter) will fail any `=` / `~=` filter and pass any `!=` filter.
///
/// Shared with `cue context list`, so the two collection queries evaluate one
/// grammar over one metadata model rather than drifting into two.
pub(crate) fn apply_filters(fm: &serde_json::Value, filters: &[Filter]) -> bool {
    filters.iter().all(|f| evaluate_filter(f, fm))
}

#[derive(Serialize)]
pub struct CueFile {
    pub path: String,
    pub name: String,
    pub context: String,
    #[serde(rename = "type")]
    pub cue_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub frontmatter: Option<serde_json::Value>,
}

pub struct ListOptions {
    /// The active-context override, narrowing the listing to one context.
    /// Not a query scope: scope is the repository/store breadth a collection
    /// query looks over, and this listing does not offer one yet.
    pub context: Option<String>,
    pub cue_types: Vec<String>,
    pub json: bool,
    pub frontmatter: bool,
    pub store_root: Option<std::path::PathBuf>,
    pub filters: Vec<Filter>,
}

pub fn list(root: &Path, opts: ListOptions) -> Result<Vec<CueFile>> {
    let ListOptions {
        context,
        cue_types,
        frontmatter,
        store_root,
        filters,
        ..
    } = opts;

    // Parse metadata once when either filtering or outputting it requires it.
    let need_metadata = frontmatter || !filters.is_empty();

    // 1. Resolve the current repository's directory in the central store.
    let store_dir = store::root(store_root.as_deref())?.join(store::repository_scope(root)?);

    // 2. Determine scan directory/directories
    let active_context = cuelib::head::resolve_active_context(root, context.as_deref())?;
    let mut paths = resolve_central_scan_paths(&store_dir, active_context.as_deref())?;

    // 3. Sort
    paths.sort();

    // 4. Read each address once, then reuse the type it resolves to for the
    //    type filter, for metadata decoding, and for the emitted row.
    let rows: Vec<CueFile> = paths
        .into_iter()
        .filter_map(|path| {
            let address = artifact_address(&path, &store_dir)?;

            if !cue_types.is_empty()
                && !cue_types
                    .iter()
                    .any(|requested| requested == address.cue_type.as_str())
            {
                return None;
            }

            let metadata = if need_metadata {
                let metadata = parse_metadata(&path, address.cue_type);
                if !apply_filters(&metadata, &filters) {
                    return None;
                }
                Some(metadata)
            } else {
                None
            };

            Some(CueFile {
                path: path.to_string_lossy().into_owned(),
                name: address.name,
                context: address.context,
                cue_type: address.cue_type.as_str().to_string(),
                frontmatter: if frontmatter {
                    metadata.filter(|value| !value.is_null())
                } else {
                    None
                },
            })
        })
        .collect();

    Ok(rows)
}

fn resolve_central_scan_paths(store_dir: &Path, context: Option<&str>) -> Result<Vec<PathBuf>> {
    let scan_dir = if let Some(context) = context {
        store_dir.join(context)
    } else {
        store_dir.to_path_buf()
    };
    collect_files(&scan_dir)
}

// ── Unit tests ────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // ── Filter::from_str ──────────────────────────────────────────────────────

    #[test]
    fn parse_eq_string() {
        let f: Filter = "status=todo".parse().unwrap();
        assert_eq!(f.path, vec!["status"]);
        assert_eq!(f.op, FilterOp::Eq);
        assert_eq!(f.rhs, serde_json::Value::String("todo".into()));
    }

    #[test]
    fn parse_neq() {
        let f: Filter = "status!=done".parse().unwrap();
        assert_eq!(f.op, FilterOp::NotEq);
        assert_eq!(f.rhs, serde_json::Value::String("done".into()));
    }

    #[test]
    fn parse_contains() {
        let f: Filter = "title~=Meeting".parse().unwrap();
        assert_eq!(f.op, FilterOp::Contains);
        assert_eq!(f.rhs, serde_json::Value::String("Meeting".into()));
    }

    #[test]
    fn parse_nested_key() {
        let f: Filter = "meta.priority=high".parse().unwrap();
        assert_eq!(f.path, vec!["meta", "priority"]);
    }

    #[test]
    fn parse_numeric_rhs_coerced() {
        let f: Filter = "count=42".parse().unwrap();
        assert_eq!(f.rhs, serde_json::json!(42));
    }

    #[test]
    fn parse_boolean_rhs_coerced() {
        let f: Filter = "enabled=true".parse().unwrap();
        assert_eq!(f.rhs, serde_json::json!(true));
    }

    #[test]
    fn parse_no_operator_errors() {
        assert!("statusdone".parse::<Filter>().is_err());
    }

    #[test]
    fn parse_empty_key_errors() {
        assert!("=value".parse::<Filter>().is_err());
    }

    // ── evaluate_filter ───────────────────────────────────────────────────────

    fn fm(s: &str) -> serde_json::Value {
        serde_yaml::from_str(s).unwrap()
    }

    #[test]
    fn eq_matches() {
        let f: Filter = "status=todo".parse().unwrap();
        assert!(evaluate_filter(&f, &fm("status: todo")));
    }

    #[test]
    fn eq_no_match() {
        let f: Filter = "status=todo".parse().unwrap();
        assert!(!evaluate_filter(&f, &fm("status: done")));
    }

    #[test]
    fn neq_matches_different_value() {
        let f: Filter = "status!=done".parse().unwrap();
        assert!(evaluate_filter(&f, &fm("status: todo")));
    }

    #[test]
    fn neq_no_match_same_value() {
        let f: Filter = "status!=done".parse().unwrap();
        assert!(!evaluate_filter(&f, &fm("status: done")));
    }

    #[test]
    fn eq_missing_key_is_false() {
        let f: Filter = "status=todo".parse().unwrap();
        assert!(!evaluate_filter(&f, &fm("other: value")));
    }

    #[test]
    fn neq_missing_key_is_true() {
        let f: Filter = "status!=done".parse().unwrap();
        assert!(evaluate_filter(&f, &fm("other: value")));
    }

    #[test]
    fn contains_matches() {
        let f: Filter = "title~=Meeting".parse().unwrap();
        assert!(evaluate_filter(&f, &fm("title: Weekly Meeting Notes")));
    }

    #[test]
    fn contains_no_match() {
        let f: Filter = "title~=Meeting".parse().unwrap();
        assert!(!evaluate_filter(&f, &fm("title: Code Review")));
    }

    #[test]
    fn contains_non_string_value_is_false() {
        let f: Filter = "count~=1".parse().unwrap();
        assert!(!evaluate_filter(&f, &fm("count: 42")));
    }

    #[test]
    fn nested_key_eq() {
        let f: Filter = "meta.priority=high".parse().unwrap();
        assert!(evaluate_filter(&f, &fm("meta:\n  priority: high")));
    }

    #[test]
    fn nested_key_missing_is_false() {
        let f: Filter = "meta.priority=high".parse().unwrap();
        assert!(!evaluate_filter(&f, &fm("status: todo")));
    }

    #[test]
    fn null_frontmatter_eq_is_false() {
        let f: Filter = "status=todo".parse().unwrap();
        assert!(!evaluate_filter(&f, &serde_json::Value::Null));
    }

    #[test]
    fn null_frontmatter_neq_is_true() {
        let f: Filter = "status!=done".parse().unwrap();
        assert!(evaluate_filter(&f, &serde_json::Value::Null));
    }
}
