use anyhow::Context;
use figment::{
    Figment,
    providers::{Format, Json, Serialized},
};
use serde::{Deserialize, Serialize};
use std::{collections::BTreeMap, path::Path, process::Command};

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Config {
    pub scopes: BTreeMap<String, Vec<String>>,
    pub default_scope: String,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            scopes: BTreeMap::new(),
            default_scope: "repo".into(),
        }
    }
}

impl Config {
    pub fn load(cwd: &Path) -> anyhow::Result<Self> {
        let global = match std::env::var_os("CUE_CONFIG_DIR") {
            Some(path) => std::path::PathBuf::from(path),
            None => dirs::home_dir()
                .context("could not determine home directory")?
                .join(".config/cue"),
        };
        let mut config = Figment::from(Serialized::defaults(Self::default()))
            .merge(Json::file(global.join("cue.json")));
        let output = Command::new("git")
            .args(["rev-parse", "--show-toplevel"])
            .current_dir(cwd)
            .output()
            .context("could not locate project config")?;
        if output.status.success() {
            let root = String::from_utf8(output.stdout)?;
            config = config.merge(Json::file(Path::new(root.trim_end()).join("cue.json")));
        }
        Ok(config.extract()?)
    }
}
