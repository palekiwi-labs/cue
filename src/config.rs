use figment::{
    providers::{Env, Format, Json, Serialized},
    Figment,
};
use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Deserialize, Serialize, Debug)]
pub struct Config {
    pub branch_name: String,
    pub dir_name: String,
    #[serde(default)]
    pub diff_exclude_paths: Vec<String>,
    #[serde(default)]
    pub base_branch_cmd: Option<String>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            branch_name: "mem".into(),
            dir_name: ".mem".into(),
            diff_exclude_paths: Vec::new(),
            base_branch_cmd: None,
        }
    }
}

impl Config {
    pub fn load(project_root: &Path) -> anyhow::Result<Self> {
        let mut builder = Figment::from(Serialized::defaults(Config::default()));

        if let Ok(home) = std::env::var("HOME") {
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
