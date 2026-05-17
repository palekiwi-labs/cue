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
}
