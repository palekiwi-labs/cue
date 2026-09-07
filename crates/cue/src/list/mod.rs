use crate::config::Config;
use anyhow::Result;
use cuelib::artifact::{collect_files, extract_frontmatter_yaml};
use cuelib::store;
use serde::Serialize;
use std::path::{Path, PathBuf};
use std::str::FromStr;

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
fn parse_metadata(path: &Path) -> serde_json::Value {
    if path.parent().and_then(Path::file_name) == Some(std::ffi::OsStr::new("bin")) {
        return std::fs::File::open(path)
            .ok()
            .and_then(|file| serde_json::from_reader(file).ok())
            .unwrap_or(serde_json::Value::Null);
    }

    extract_frontmatter_yaml(path)
        .and_then(|yaml| serde_yaml::from_str(&yaml).ok())
        .unwrap_or(serde_json::Value::Null)
}

/// Returns `true` if `fm` satisfies every filter (AND semantics).
/// `Null` (no frontmatter) will fail any `=` / `~=` filter and pass any `!=` filter.
fn apply_filters(fm: &serde_json::Value, filters: &[Filter]) -> bool {
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
    pub scope: Option<String>,
    pub all: bool,
    pub cue_type: Option<String>,
    pub include_gitignored: bool,
    pub json: bool,
    pub frontmatter: bool,
    pub filters: Vec<Filter>,
}

pub fn list(
    root: &Path,
    config: &Config,
    opts: ListOptions,
) -> Result<Vec<(PathBuf, Option<serde_json::Value>)>> {
    let ListOptions {
        scope,
        all,
        cue_type,
        include_gitignored,
        frontmatter,
        filters,
        ..
    } = opts;

    // Parse frontmatter once when either filtering or outputting it requires it.
    let need_frontmatter = frontmatter || !filters.is_empty();

    // 1. Resolve the current repository's directory in the central store.
    let store_dir = store::root()?.join(store::repository_scope(root)?);

    // 2. Determine scan directory/directories
    let active_context = cuelib::head::resolve_active_context(root, scope.as_deref())?;
    let mut paths = resolve_central_scan_paths(&store_dir, all, active_context.as_deref())?;

    // 3. Sort
    paths.sort();

    // 4. Filter by structure (type, gitignored)
    let valid_paths = paths.into_iter().filter(|path| {
        is_valid_cue_file(
            path,
            &store_dir,
            cue_type.as_deref(),
            include_gitignored,
            &config.ignored_types,
        )
    });

    // 5. Parse frontmatter once (if needed), apply filters, carry value forward.
    let filtered: Vec<(PathBuf, Option<serde_json::Value>)> = valid_paths
        .filter_map(|path| {
            let fm_val = if need_frontmatter {
                let fm = parse_metadata(&path);
                if !apply_filters(&fm, &filters) {
                    return None;
                }
                Some(fm)
            } else {
                None
            };
            Some((path, fm_val))
        })
        .collect();

    Ok(filtered)
}

fn resolve_central_scan_paths(
    store_dir: &Path,
    all: bool,
    context: Option<&str>,
) -> Result<Vec<PathBuf>> {
    let scan_dir = if all {
        store_dir.to_path_buf()
    } else if let Some(context) = context {
        store_dir.join(context)
    } else {
        store_dir.to_path_buf()
    };
    collect_files(&scan_dir)
}

pub fn is_valid_cue_file(
    path: &Path,
    cue_path: &Path,
    cue_type: Option<&str>,
    include_gitignored: bool,
    ignored_types: &[String],
) -> bool {
    let Ok(rel_to_mem) = path.strip_prefix(cue_path) else {
        return false;
    };
    let mut components = rel_to_mem.components();

    let _branch = components.next();
    let Some(category_comp) = components.next() else {
        return false;
    };
    let Some(_name_comp) = components.next() else {
        return false; // Ensures len >= 3
    };

    let category = category_comp.as_os_str().to_string_lossy();

    if let Some(requested) = cue_type {
        if category != requested {
            return false;
        }
    } else if !include_gitignored && ignored_types.iter().any(|t| t == category.as_ref()) {
        return false;
    }

    true
}

pub fn to_cue_file(path: &Path, cue_path: &Path) -> Option<CueFile> {
    let rel_to_mem = path.strip_prefix(cue_path).ok()?;
    let mut components = rel_to_mem.components();

    let context = components
        .next()?
        .as_os_str()
        .to_string_lossy()
        .into_owned();
    let cue_type = components
        .next()?
        .as_os_str()
        .to_string_lossy()
        .into_owned();
    let name = components
        .collect::<PathBuf>()
        .to_string_lossy()
        .into_owned();

    Some(CueFile {
        path: path.to_string_lossy().into_owned(),
        name,
        context,
        cue_type,
        frontmatter: None,
    })
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
