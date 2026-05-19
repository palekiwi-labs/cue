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
        } else if let Ok(home) = std::env::var("HOME") {
            let global_config = Path::new(&home).join(".config/mem/mem.json");
            builder = builder.merge(Json::file(global_config));
        }

        let project_config = project_root.join("mem.json");
        let config = builder
            .merge(Json::file(project_config))
            .merge(Env::prefixed("MEM_"))
            .extract()?;

        Ok(config)
    }
}
