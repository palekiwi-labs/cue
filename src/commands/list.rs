use crate::config::Config;
use crate::git;
use anyhow::{Context, Result};
use serde::Serialize;
use std::fs;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::str::FromStr;

const FRONTMATTER_MAX_LINES: usize = 64;

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

/// Returns `true` if the file at `path` satisfies every filter (AND semantics).
/// Files without frontmatter will fail any `=` / `~=` filter and pass any `!=` filter.
fn matches_filters(path: &Path, filters: &[Filter]) -> bool {
    if filters.is_empty() {
        return true;
    }
    let fm: serde_json::Value = extract_frontmatter_yaml(path)
        .and_then(|yaml| serde_yaml::from_str(&yaml).ok())
        .unwrap_or(serde_json::Value::Null);
    filters.iter().all(|f| evaluate_filter(f, &fm))
}

#[derive(Serialize)]
struct MemFile {
    path: String,
    name: String,
    branch: String,
    category: String,
    hash: Option<String>,
    commit_hash: Option<String>,
    commit_timestamp: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    frontmatter: Option<serde_json::Value>,
}

pub struct ListOptions {
    pub branch_name: Option<String>,
    pub all: bool,
    pub mem_type: Option<String>,
    pub include_gitignored: bool,
    pub json: bool,
    pub frontmatter: bool,
    pub filters: Vec<Filter>,
}

pub fn handle(cwd: &Path, opts: ListOptions) -> Result<()> {
    let ListOptions {
        branch_name,
        all,
        mem_type,
        include_gitignored,
        json,
        frontmatter,
        filters,
    } = opts;

    let json_output = json || frontmatter;
    // 1. Verify git repo
    git::run_git(["rev-parse", "--git-dir"], cwd).context("Not in a git repository")?;

    // 2. Get git root
    let root = git::get_git_root(cwd)?;

    // 3. Load config
    let config = Config::load(&root)?;

    // 4. Check if .mem exists
    let mem_path = root.join(&config.dir_name);
    if !mem_path.is_dir() {
        anyhow::bail!(
            "{} directory does not exist. Run `mem init` first.",
            config.dir_name
        );
    }

    // 5. Determine scan directory/directories
    let mut paths = resolve_scan_paths(&root, &mem_path, all, branch_name)?;

    // 6. Sort
    paths.sort();

    // 7. Filter by structure (type, gitignored)
    let valid_paths = paths.into_iter().filter(|path| {
        is_valid_mem_file(
            path,
            &mem_path,
            mem_type.as_deref(),
            include_gitignored,
            &config.ignored_types,
        )
    });

    // 8. Filter by frontmatter (reads files only when filters are present)
    let filtered_paths = valid_paths.filter(|path| matches_filters(path, &filters));

    // 9. Process files and output
    if !json_output {
        for path in filtered_paths {
            let rel_path = path.strip_prefix(&root).unwrap_or(&path);
            println!("{}", rel_path.display());
        }
    } else {
        let mem_files: Vec<MemFile> = filtered_paths
            .filter_map(|path| {
                let mf = to_mem_file(&path, &mem_path, &root)?;
                Some(if frontmatter {
                    enrich_frontmatter(mf, &path)
                } else {
                    mf
                })
            })
            .collect();
        println!("{}", serde_json::to_string_pretty(&mem_files)?);
    }

    Ok(())
}

fn resolve_scan_paths(
    root: &Path,
    mem_path: &Path,
    all: bool,
    branch_name: Option<String>,
) -> Result<Vec<PathBuf>> {
    if all {
        collect_files(mem_path)
    } else {
        let branch = if let Some(b) = branch_name {
            b
        } else {
            git::get_current_branch(root)?
        };
        let branch_dir = branch.replace(['/', '\\'], "-");
        let scan_dir = mem_path.join(&branch_dir);

        if scan_dir.exists() {
            collect_files(&scan_dir)
        } else {
            Ok(Vec::new())
        }
    }
}

fn is_valid_mem_file(
    path: &Path,
    mem_path: &Path,
    mem_type: Option<&str>,
    include_gitignored: bool,
    ignored_types: &[String],
) -> bool {
    let Ok(rel_to_mem) = path.strip_prefix(mem_path) else {
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

    if let Some(requested) = mem_type {
        if category != requested {
            return false;
        }
    } else if !include_gitignored && ignored_types.iter().any(|t| t == category.as_ref()) {
        return false;
    }

    true
}

fn to_mem_file(path: &Path, mem_path: &Path, root: &Path) -> Option<MemFile> {
    let rel_to_mem = path.strip_prefix(mem_path).ok()?;
    let mut components = rel_to_mem.components();

    let branch = components
        .next()?
        .as_os_str()
        .to_string_lossy()
        .into_owned();
    let category = components
        .next()?
        .as_os_str()
        .to_string_lossy()
        .into_owned();

    let rel_path = path
        .strip_prefix(root)
        .unwrap_or(path)
        .to_string_lossy()
        .to_string();

    let mut mem_file = MemFile {
        path: rel_path,
        name: String::new(),
        branch: branch.clone(),
        category: category.clone(),
        hash: None,
        commit_hash: None,
        commit_timestamp: 0,
        frontmatter: None,
    };

    // Detect pinned artifacts structurally: any category with depth >= 4
    // where the 3rd component parses as <timestamp>-<hash>.
    let comp_count = rel_to_mem.components().count();
    if comp_count >= 4 {
        let mut comps = rel_to_mem.components();
        comps.next(); // branch
        comps.next(); // category
        if let Some(ts_hash_dir) = comps.next() {
            let ts_hash_str = ts_hash_dir.as_os_str().to_string_lossy();
            if let Some((ts_str, hash_str)) = ts_hash_str.split_once('-')
                && let Ok(ts) = ts_str.parse::<u64>()
            {
                mem_file.commit_timestamp = ts;
                mem_file.hash = Some(hash_str.to_string());
                mem_file.commit_hash = Some(hash_str.to_string());

                // name is relative to the ts-hash dir
                let prefix = mem_path.join(&branch).join(&category).join(ts_hash_dir);
                if let Ok(rel_name) = path.strip_prefix(&prefix) {
                    mem_file.name = rel_name.to_string_lossy().to_string();
                }
                return Some(mem_file);
            }
        }
    }

    // Flat artifact: name is relative to the category dir
    let prefix = mem_path.join(&branch).join(&category);
    if let Ok(rel_name) = path.strip_prefix(&prefix) {
        mem_file.name = rel_name.to_string_lossy().to_string();
    }

    Some(mem_file)
}

fn extract_frontmatter_yaml(path: &Path) -> Option<String> {
    let file = fs::File::open(path).ok()?;
    let mut reader = BufReader::new(file);
    let mut line = String::new();

    // First line must be exactly "---"
    reader.read_line(&mut line).ok()?;
    if line.trim_end() != "---" {
        return None;
    }

    let mut yaml = String::new();
    for _ in 0..FRONTMATTER_MAX_LINES {
        line.clear();
        let n = reader.read_line(&mut line).ok()?;
        if n == 0 {
            return None; // EOF before closing fence — malformed
        }
        if line.trim_end() == "---" {
            return Some(yaml);
        }
        yaml.push_str(&line);
    }

    None // Exceeded line budget — treat as malformed
}

fn enrich_frontmatter(mut mem_file: MemFile, path: &Path) -> MemFile {
    if let Some(yaml_str) = extract_frontmatter_yaml(path) {
        mem_file.frontmatter = serde_yaml::from_str::<serde_json::Value>(&yaml_str).ok();
    }
    mem_file
}

fn collect_files(dir: &Path) -> Result<Vec<PathBuf>> {
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
