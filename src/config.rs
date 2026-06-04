use figment::{
    Figment,
    providers::{Env, Format, Json, Serialized},
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;

#[derive(Debug, Serialize, Deserialize, Default, Clone, PartialEq)]
pub struct ContextProfile {
    #[serde(default)]
    pub artifacts: Vec<String>,
    #[serde(default)]
    pub include: Vec<String>,
    #[serde(default)]
    pub instructions: Option<String>,
}

pub type ContextConfig = HashMap<String, ContextProfile>;

#[derive(Deserialize, Serialize, Debug)]
pub struct Config {
    pub branch_name: String,
    pub dir_name: String,
    #[serde(default)]
    pub artifact_types: Vec<String>,
    #[serde(default)]
    pub ignored_types: Vec<String>,
    #[serde(default)]
    pub context: ContextConfig,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            branch_name: "mem".into(),
            dir_name: ".mem".into(),
            artifact_types: vec!["spec".into(), "trace".into(), "tmp".into()],
            ignored_types: vec!["tmp".into()],
            context: HashMap::new(),
        }
    }
}

impl Config {
    pub fn load(project_root: &Path) -> anyhow::Result<Self> {
        let mut builder = Figment::from(Serialized::defaults(Config::default()));

        if let Ok(config_dir) = std::env::var("MEM_CONFIG_DIR") {
            let global_config = Path::new(&config_dir).join("mem.json");
            builder = builder.merge(Json::file(global_config));
        } else if let Some(home) = dirs::home_dir() {
            let global_config = home.join(".config/mem/mem.json");
            builder = builder.merge(Json::file(global_config));
        }

        let project_config = project_root.join("mem.json");
        let config = builder
            .merge(Json::file(project_config))
            .merge(Env::prefixed("MEM_").split("__"))
            .extract()?;

        Ok(config)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_artifact_types() {
        let config = Config::default();
        assert_eq!(config.artifact_types, vec!["spec", "trace", "tmp"]);
    }

    #[test]
    fn test_default_ignored_types() {
        let config = Config::default();
        assert_eq!(config.ignored_types, vec!["tmp"]);
    }

    #[test]
    fn test_artifact_types_json_override() {
        use tempfile::tempdir;
        let dir = tempdir().unwrap();

        let config_json = r#"{"artifact_types": ["spec", "trace", "tmp", "doc", "custom"]}"#;
        std::fs::write(dir.path().join("mem.json"), config_json).unwrap();

        // Unset MEM_ARTIFACT_TYPES so host environment cannot override the JSON config
        let config =
            temp_env::with_var_unset("MEM_ARTIFACT_TYPES", || Config::load(dir.path()).unwrap());
        assert_eq!(
            config.artifact_types,
            vec!["spec", "trace", "tmp", "doc", "custom"]
        );
    }

    #[test]
    fn test_ignored_types_json_override() {
        use tempfile::tempdir;
        let dir = tempdir().unwrap();

        let config_json = r#"{"ignored_types": ["tmp", "ref"]}"#;
        std::fs::write(dir.path().join("mem.json"), config_json).unwrap();

        // Unset MEM_IGNORED_TYPES so host environment cannot override the JSON config
        let config =
            temp_env::with_var_unset("MEM_IGNORED_TYPES", || Config::load(dir.path()).unwrap());
        assert_eq!(config.ignored_types, vec!["tmp", "ref"]);
    }

    #[test]
    fn test_nested_env_override() {
        use tempfile::tempdir;
        let dir = tempdir().unwrap();

        // Set a nested environment variable
        // MEM_CONTEXT__DEFAULT__INSTRUCTIONS maps to context["default"].instructions
        unsafe {
            std::env::set_var("MEM_CONTEXT__DEFAULT__INSTRUCTIONS", "env instructions");
        }

        let config = Config::load(dir.path()).unwrap();

        let default_profile = config
            .context
            .get("default")
            .expect("default profile should exist");
        assert_eq!(
            default_profile.instructions,
            Some("env instructions".into())
        );

        // Clean up
        unsafe {
            std::env::remove_var("MEM_CONTEXT__DEFAULT__INSTRUCTIONS");
        }
    }
}
