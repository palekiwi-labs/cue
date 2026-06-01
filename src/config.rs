use figment::{
    providers::{Env, Format, Json, Serialized},
    Figment,
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
    pub context: ContextConfig,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            branch_name: "mem".into(),
            dir_name: ".mem".into(),
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
