use crate::config::{Config, ContextConfig, ContextProfile};
use crate::git::get_git_root;
use glob::glob;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

#[derive(Debug, Serialize, Deserialize)]
pub struct Artifact {
    pub path: PathBuf,
    pub content: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ResolvedContext {
    pub artifacts: Vec<Artifact>,
    pub instructions: Option<String>,
}

pub fn context_json_path(root: &Path, scope: &str, dir_name: &str) -> PathBuf {
    root.join(dir_name).join(scope).join("context.json")
}

pub fn load_context_config(path: &Path) -> anyhow::Result<ContextConfig> {
    if !path.exists() {
        anyhow::bail!("Context file not found: {}", path.display());
    }
    let content = std::fs::read_to_string(path)?;
    let config: ContextConfig = serde_json::from_str(&content)?;
    Ok(config)
}

pub fn parse_artifact_path(
    raw: &str,
    current_scope: &str,
    git_root: &Path,
    dir_name: &str,
) -> anyhow::Result<PathBuf> {
    let (scope, rest) = if let Some(stripped) = raw.strip_prefix('@') {
        // Cross-context reference
        let (s, p) = match stripped.split_once(':') {
            Some((scope, path)) => (scope, path),
            None => (stripped, ""),
        };

        (s.to_string(), p.to_string())
    } else {
        // Local artifact. Defaults to current scope.
        // We optionally strip a leading "./" for cleaner aesthetics.
        let p = raw.strip_prefix("./").unwrap_or(raw);
        (current_scope.to_string(), p.to_string())
    };

    let rest_path = Path::new(&rest);

    // Prevent base path overwrite via `join`
    if rest_path.has_root() {
        anyhow::bail!(
            "Absolute or root paths are not allowed in artifact paths: {}",
            raw
        );
    }

    let full_path = git_root.join(dir_name).join(scope).join(rest_path);

    Ok(full_path)
}

pub fn resolve_profile(
    scope: &str,
    profile_name: &str,
    git_root: &Path,
    dir_name: &str,
    visited: &mut HashSet<(String, String)>,
) -> anyhow::Result<Vec<PathBuf>> {
    let key = (scope.to_string(), profile_name.to_string());
    if visited.contains(&key) {
        anyhow::bail!(
            "Cycle detected in context profile includes: {}:{}",
            scope,
            profile_name
        );
    }
    visited.insert(key.clone());

    let config_path = context_json_path(git_root, scope, dir_name);
    let config = match load_context_config(&config_path) {
        Ok(c) => c,
        Err(_) => {
            eprintln!(
                "Warning: Could not load context for scope {}, skipping",
                scope
            );
            visited.remove(&key);
            return Ok(Vec::new());
        }
    };

    let profile = config.get(profile_name).ok_or_else(|| {
        visited.remove(&key);
        anyhow::anyhow!(
            "Profile '{}' not found in {}",
            profile_name,
            config_path.display()
        )
    })?;

    let mut accumulator = Vec::new();

    for inc in &profile.include {
        let (inc_scope, inc_profile) = if let Some(rest) = inc.strip_prefix('@') {
            match rest.split_once(':') {
                Some((s, p)) => (s.to_string(), p.to_string()),
                None => (rest.to_string(), "default".to_string()),
            }
        } else {
            match inc.split_once(':') {
                Some((s, p)) => (s.to_string(), p.to_string()),
                None => (inc.to_string(), "default".to_string()),
            }
        };

        let inc_paths = resolve_profile(&inc_scope, &inc_profile, git_root, dir_name, visited)?;
        accumulator.extend(inc_paths);
    }

    for art in &profile.artifacts {
        let path = parse_artifact_path(art, scope, git_root, dir_name)?;

        if art.contains('*') || art.contains('?') || art.contains('[') {
            let pattern = path.to_string_lossy();
            match glob(&pattern) {
                Ok(entries) => {
                    for p in entries.flatten() {
                        if p.is_file() {
                            accumulator.push(p);
                        }
                    }
                }
                Err(e) => {
                    eprintln!("Warning: Invalid glob pattern '{}': {}", art, e);
                }
            }
        } else {
            accumulator.push(path);
        }
    }

    visited.remove(&key);

    // Deduplicate: first occurrence wins
    let mut final_paths = Vec::new();
    let mut seen = HashSet::new();
    for path in accumulator {
        if seen.insert(path.clone()) {
            final_paths.push(path);
        }
    }

    Ok(final_paths)
}

pub fn gather_context(cwd: &Path, profile_name: Option<&str>) -> anyhow::Result<ResolvedContext> {
    let profile_name = profile_name.unwrap_or("default");
    let git_root = get_git_root(cwd)?;
    let canonical_git_root = git_root.canonicalize()?;
    let config = Config::load(&git_root)?;
    let dir_name = &config.dir_name;
    let cue_dir = git_root.join(dir_name);
    let scope = cuelib::head::resolve_scope(&cue_dir)?;

    let mut visited = HashSet::new();
    let paths = resolve_profile(&scope, profile_name, &git_root, dir_name, &mut visited)?;

    let mut artifacts = Vec::new();
    for path in paths {
        let canonical_path = match path.canonicalize() {
            Ok(p) => p,
            Err(_) => {
                eprintln!(
                    "Warning: Artifact not found or is not a file: {}",
                    path.display()
                );
                continue;
            }
        };

        if !canonical_path.starts_with(&canonical_git_root) {
            eprintln!("Warning: Path traversal blocked: {}", path.display());
            continue;
        }

        if !canonical_path.is_file() {
            eprintln!(
                "Warning: Artifact is not a file (skipping): {}",
                path.display()
            );
            continue;
        }

        let content = std::fs::read_to_string(&canonical_path)?;
        artifacts.push(Artifact {
            path: canonical_path,
            content,
        });
    }

    let context_path = context_json_path(&git_root, &scope, dir_name);
    let context_config = load_context_config(&context_path)?;
    let profile_obj = context_config.get(profile_name).ok_or_else(|| {
        anyhow::anyhow!(
            "Profile '{}' not found in {}",
            profile_name,
            context_path.display()
        )
    })?;

    let instructions = profile_obj.instructions.clone();

    Ok(ResolvedContext {
        artifacts,
        instructions,
    })
}

pub fn init_context(cwd: &Path, force: bool) -> anyhow::Result<PathBuf> {
    let git_root = get_git_root(cwd)?;
    let config = Config::load(&git_root)?;
    let cue_dir = git_root.join(&config.dir_name);
    let scope = cuelib::head::resolve_scope(&cue_dir)?;
    let config_path = context_json_path(&git_root, &scope, &config.dir_name);

    if config_path.exists() && !force {
        anyhow::bail!(
            "Context file already exists: {}. Use --force to overwrite.",
            config_path.display()
        );
    }

    let context_config = if !config.context.is_empty() {
        // Use template from config
        config.context.clone()
    } else {
        // No template defined: initialize with an empty default profile
        let mut map = HashMap::new();
        map.insert("default".to_string(), ContextProfile::default());
        map
    };

    let json = serde_json::to_string_pretty(&context_config)?;
    std::fs::create_dir_all(config_path.parent().unwrap())?;
    std::fs::write(&config_path, json)?;

    Ok(config_path)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_deserialize_full_schema() {
        let data = json!({
            "default": {
                "artifacts": ["./spec/index.md"],
                "include": ["@other-scope"],
                "instructions": "Go fast"
            },
            "brief": {
                "artifacts": ["./spec/index.md"]
            }
        });
        let config: ContextConfig = serde_json::from_value(data).unwrap();

        assert_eq!(config.len(), 2);
        assert_eq!(config["default"].artifacts, vec!["./spec/index.md"]);
        assert_eq!(config["default"].include, vec!["@other-scope"]);
        assert_eq!(config["default"].instructions, Some("Go fast".to_string()));
        assert_eq!(config["brief"].artifacts, vec!["./spec/index.md"]);
        assert_eq!(config["brief"].include, Vec::<String>::new());
    }

    #[test]
    fn test_deserialize_partial_schema() {
        let data = json!({
            "default": {
                "artifacts": ["./spec/index.md"]
            }
        });
        let config: ContextConfig = serde_json::from_value(data).unwrap();
        assert_eq!(config["default"].artifacts, vec!["./spec/index.md"]);
        assert_eq!(config["default"].include, Vec::<String>::new());
    }

    #[test]
    fn test_deserialize_unknown_fields_tolerated() {
        let data = json!({
            "default": {
                "artifacts": [],
                "future_field": "ignore me"
            }
        });
        let config: ContextConfig = serde_json::from_value(data).unwrap();
        assert!(config.contains_key("default"));
    }

    const DIR: &str = ".cue";

    #[test]
    fn test_parse_artifact_path() {
        let root = Path::new("/repo");
        let current = "feat-ctx";

        // Current scope with ./
        let path = parse_artifact_path("./spec/index.md", current, root, DIR).unwrap();
        assert_eq!(path, root.join(DIR).join(current).join("spec/index.md"));

        // Current scope without prefix
        let path = parse_artifact_path("spec/plan.md", current, root, DIR).unwrap();
        assert_eq!(path, root.join(DIR).join(current).join("spec/plan.md"));

        // Current scope with parent traversal (allowed)
        let path = parse_artifact_path("../master/spec/index.md", current, root, DIR).unwrap();
        assert_eq!(
            path,
            root.join(DIR).join(current).join("../master/spec/index.md")
        );

        // Cross-context reference
        let path = parse_artifact_path("@other:spec/plan.md", current, root, DIR).unwrap();
        assert_eq!(path, root.join(DIR).join("other").join("spec/plan.md"));

        // Cross-context with colon in path (split_once takes the first colon)
        let path = parse_artifact_path("@feat:context:spec/index.md", current, root, DIR).unwrap();
        assert_eq!(
            path,
            root.join(DIR).join("feat").join("context:spec/index.md")
        );

        // Cross-context without path
        let path = parse_artifact_path("@other", current, root, DIR).unwrap();
        assert_eq!(path, root.join(DIR).join("other").join(""));

        // Failures
        assert!(parse_artifact_path("/absolute.md", current, root, DIR).is_err());
        assert!(parse_artifact_path("@other:/etc/passwd", current, root, DIR).is_err());

        // Valid path containing ".." as part of filename
        assert!(parse_artifact_path("./spec/my..file.md", current, root, DIR).is_ok());
    }

    #[test]
    fn test_resolve_profile_include_formats() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path();

        let scope_a = root.join(DIR).join("A");
        let scope_b = root.join(DIR).join("B");
        let scope_feat = root.join(DIR).join("feat-test");
        std::fs::create_dir_all(&scope_a).unwrap();
        std::fs::create_dir_all(&scope_b).unwrap();
        std::fs::create_dir_all(&scope_feat).unwrap();

        std::fs::write(
            scope_a.join("context.json"),
            r#"{
                "default": { "include": ["B", "B:brief", "@B", "feat-test"] }
            }"#,
        )
        .unwrap();
        std::fs::write(
            scope_b.join("context.json"),
            r#"{
                "default": { "artifacts": ["./b-default.md"] },
                "brief": { "artifacts": ["./b-brief.md"] }
            }"#,
        )
        .unwrap();
        std::fs::write(
            scope_feat.join("context.json"),
            r#"{
                "default": { "artifacts": ["./feat.md"] }
            }"#,
        )
        .unwrap();

        // Create dummy files
        std::fs::write(scope_b.join("b-default.md"), "b-default").unwrap();
        std::fs::write(scope_b.join("b-brief.md"), "b-brief").unwrap();
        std::fs::write(scope_feat.join("feat.md"), "feat").unwrap();

        let mut visited = HashSet::new();
        let res = resolve_profile("A", "default", root, DIR, &mut visited).unwrap();

        // Accumulator: [b-default, b-brief, b-default (deduped), feat]
        // Final: [b-default, b-brief, feat]
        assert_eq!(res.len(), 3);
        assert!(res[0].to_str().unwrap().contains("b-default.md"));
        assert!(res[1].to_str().unwrap().contains("b-brief.md"));
        assert!(res[2].to_str().unwrap().contains("feat.md"));
    }

    #[test]
    fn test_resolve_profile_cycle() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path();

        // Setup Cycle: A -> B -> A
        let scope_a = root.join(DIR).join("A");
        let scope_b = root.join(DIR).join("B");
        std::fs::create_dir_all(&scope_a).unwrap();
        std::fs::create_dir_all(&scope_b).unwrap();

        std::fs::write(
            scope_a.join("context.json"),
            r#"{"default": {"include": ["@B"]}}"#,
        )
        .unwrap();
        std::fs::write(
            scope_b.join("context.json"),
            r#"{"default": {"include": ["@A"]}}"#,
        )
        .unwrap();

        let mut visited = HashSet::new();
        let res = resolve_profile("A", "default", root, DIR, &mut visited);
        assert!(res.is_err());
        assert!(res.unwrap_err().to_string().contains("Cycle detected"));
    }

    #[test]
    fn test_resolve_profile_diamond_dependency() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path();

        // Setup Diamond: A -> [B, C], B -> D, C -> D
        let scope_a = root.join(DIR).join("A");
        let scope_b = root.join(DIR).join("B");
        let scope_c = root.join(DIR).join("C");
        let scope_d = root.join(DIR).join("D");
        std::fs::create_dir_all(&scope_a).unwrap();
        std::fs::create_dir_all(&scope_b).unwrap();
        std::fs::create_dir_all(&scope_c).unwrap();
        std::fs::create_dir_all(&scope_d).unwrap();

        std::fs::write(
            scope_a.join("context.json"),
            r#"{"default": {"include": ["@B", "@C"]}}"#,
        )
        .unwrap();
        std::fs::write(
            scope_b.join("context.json"),
            r#"{"default": {"include": ["@D"], "artifacts": ["./spec/b.md"]}}"#,
        )
        .unwrap();
        std::fs::write(
            scope_c.join("context.json"),
            r#"{"default": {"include": ["@D"], "artifacts": ["./spec/c.md"]}}"#,
        )
        .unwrap();
        std::fs::write(
            scope_d.join("context.json"),
            r#"{"default": {"artifacts": ["./spec/d.md"]}}"#,
        )
        .unwrap();

        let mut visited = HashSet::new();
        let res = resolve_profile("A", "default", root, DIR, &mut visited).unwrap();

        // Deduplication should ensure D appears once, and DFS ordering
        // Accumulator: [D, B, D, C] -> Deduplicated: [D, B, C]
        assert_eq!(res.len(), 3);
        assert!(res[0].to_str().unwrap().contains("D"));
        assert!(res[1].to_str().unwrap().contains("B"));
        assert!(res[2].to_str().unwrap().contains("C"));
    }

    #[test]
    fn test_resolve_profile_with_globs() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path();

        let scope_a = root.join(DIR).join("A");
        let spec_a = scope_a.join("spec");
        std::fs::create_dir_all(&spec_a).unwrap();

        std::fs::write(spec_a.join("1.md"), "1").unwrap();
        std::fs::write(spec_a.join("2.md"), "2").unwrap();
        std::fs::write(
            scope_a.join("context.json"),
            r#"{"default": {"artifacts": ["./spec/*.md"]}}"#,
        )
        .unwrap();

        let mut visited = HashSet::new();
        let res = resolve_profile("A", "default", root, DIR, &mut visited).unwrap();

        assert_eq!(res.len(), 2);
        let mut paths: Vec<_> = res
            .iter()
            .map(|p| p.file_name().unwrap().to_str().unwrap())
            .collect();
        paths.sort();
        assert_eq!(paths, vec!["1.md", "2.md"]);
    }

    #[test]
    fn test_resolve_profile_skips_directories() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path();

        let scope_a = root.join(DIR).join("A");
        let spec_a = scope_a.join("spec");
        let sub_dir = spec_a.join("notes");
        std::fs::create_dir_all(&sub_dir).unwrap();

        std::fs::write(spec_a.join("1.md"), "1").unwrap();
        std::fs::write(sub_dir.join("2.md"), "2").unwrap();
        std::fs::write(
            scope_a.join("context.json"),
            r#"{"default": {"artifacts": ["./spec/**/*"]}}"#,
        )
        .unwrap();

        let mut visited = HashSet::new();
        let res = resolve_profile("A", "default", root, DIR, &mut visited).unwrap();

        // Should include 1.md and 2.md, but NOT the 'notes' directory
        assert_eq!(res.len(), 2);
        let mut file_names: Vec<_> = res
            .iter()
            .map(|p| p.file_name().unwrap().to_str().unwrap())
            .collect();
        file_names.sort();
        assert_eq!(file_names, vec!["1.md", "2.md"]);
    }
}
