#[cfg(test)]
mod tests {
    use crate::commands::context::*;
    use serde_json::json;

    #[test]
    fn test_deserialize_full_schema() {
        let data = json!({
            "default": {
                "artifacts": ["./spec/index.md"],
                "diff": "main...HEAD",
                "include": ["@other-branch"]
            },
            "brief": {
                "artifacts": ["./spec/index.md"]
            }
        });
        let config: ContextConfig = serde_json::from_value(data).unwrap();

        assert_eq!(config.len(), 2);
        assert_eq!(config["default"].artifacts, vec!["./spec/index.md"]);
        assert_eq!(config["default"].diff, Some("main...HEAD".to_string()));
        assert_eq!(config["default"].include, vec!["@other-branch"]);
        assert_eq!(config["brief"].artifacts, vec!["./spec/index.md"]);
        assert_eq!(config["brief"].diff, None);
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
        assert_eq!(config["default"].diff, None);
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

    #[test]
    fn test_parse_artifact_path() {
        let root = Path::new("/repo");
        let current = "feat-ctx";

        // Current branch with ./
        let path = parse_artifact_path("./spec/index.md", current, root).unwrap();
        assert_eq!(path, root.join(".mem").join(current).join("spec/index.md"));

        // Current branch without prefix
        let path = parse_artifact_path("spec/plan.md", current, root).unwrap();
        assert_eq!(path, root.join(".mem").join(current).join("spec/plan.md"));

        // Current branch with parent traversal (allowed now)
        let path = parse_artifact_path("../master/spec/index.md", current, root).unwrap();
        assert_eq!(
            path,
            root.join(".mem")
                .join(current)
                .join("../master/spec/index.md")
        );

        // Cross branch
        let path = parse_artifact_path("@other:spec/plan.md", current, root).unwrap();
        assert_eq!(path, root.join(".mem").join("other").join("spec/plan.md"));

        // Cross branch with colon in branch name
        let path = parse_artifact_path("@feat:context:spec/index.md", current, root).unwrap();
        assert_eq!(
            path,
            root.join(".mem").join("feat:context").join("spec/index.md")
        );

        // Cross branch without path
        let path = parse_artifact_path("@other", current, root).unwrap();
        assert_eq!(path, root.join(".mem").join("other").join(""));

        // Failures
        assert!(parse_artifact_path("/absolute.md", current, root).is_err());
        assert!(parse_artifact_path("@branch_with/slash:spec.md", current, root).is_err());
        assert!(parse_artifact_path("@other:/etc/passwd", current, root).is_err());

        // Valid path containing ".." as part of filename
        assert!(parse_artifact_path("./spec/my..file.md", current, root).is_ok());
    }

    #[test]
    fn test_resolve_profile_cycle() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path();

        // Setup Cycle: A -> B -> A
        let branch_a = root.join(".mem").join("A");
        let branch_b = root.join(".mem").join("B");
        std::fs::create_dir_all(&branch_a).unwrap();
        std::fs::create_dir_all(&branch_b).unwrap();

        std::fs::write(
            branch_a.join("context.json"),
            r#"{"default": {"include": ["@B"]}}"#,
        )
        .unwrap();
        std::fs::write(
            branch_b.join("context.json"),
            r#"{"default": {"include": ["@A"]}}"#,
        )
        .unwrap();

        let mut visited = std::collections::HashSet::new();
        let res = resolve_profile("A", "default", root, &mut visited);
        assert!(res.is_err());
        assert!(res.unwrap_err().to_string().contains("Cycle detected"));
    }

    #[test]
    fn test_resolve_profile_diamond_dependency() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path();

        // Setup Diamond: A -> [B, C], B -> D, C -> D
        let branch_a = root.join(".mem").join("A");
        let branch_b = root.join(".mem").join("B");
        let branch_c = root.join(".mem").join("C");
        let branch_d = root.join(".mem").join("D");
        std::fs::create_dir_all(&branch_a).unwrap();
        std::fs::create_dir_all(&branch_b).unwrap();
        std::fs::create_dir_all(&branch_c).unwrap();
        std::fs::create_dir_all(&branch_d).unwrap();

        std::fs::write(
            branch_a.join("context.json"),
            r#"{"default": {"include": ["@B", "@C"]}}"#,
        )
        .unwrap();
        std::fs::write(
            branch_b.join("context.json"),
            r#"{"default": {"include": ["@D"], "artifacts": ["./spec/b.md"]}}"#,
        )
        .unwrap();
        std::fs::write(
            branch_c.join("context.json"),
            r#"{"default": {"include": ["@D"], "artifacts": ["./spec/c.md"]}}"#,
        )
        .unwrap();
        std::fs::write(
            branch_d.join("context.json"),
            r#"{"default": {"artifacts": ["./spec/d.md"]}}"#,
        )
        .unwrap();

        let mut visited = std::collections::HashSet::new();
        let res = resolve_profile("A", "default", root, &mut visited).unwrap();

        // Deduplication should ensure D appears once, and DFS ordering
        // Accumulator: [D, B, D, C] -> Deduplicated: [D, B, C]
        assert_eq!(res.len(), 3);
        assert!(res[0].to_str().unwrap().contains("D"));
        assert!(res[1].to_str().unwrap().contains("B"));
        assert!(res[2].to_str().unwrap().contains("C"));
    }
}
