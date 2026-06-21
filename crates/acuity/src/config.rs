use figment::{
    providers::{Env, Format, Json, Serialized},
    Figment,
};
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Config {
    pub gotify_host: String,
    pub port: u16,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            gotify_host: "localhost:80".into(),
            port: 33222,
        }
    }
}

impl Config {
    pub fn load() -> anyhow::Result<Self> {
        let mut builder = Figment::from(Serialized::defaults(Config::default()));

        if let Ok(config_dir) = std::env::var("ACUITY_CONFIG_DIR") {
            let path = std::path::Path::new(&config_dir).join("acuity.json");
            builder = builder.merge(Json::file(path));
        } else if let Some(home) = dirs::home_dir() {
            let path = home.join(".config/acuity/acuity.json");
            builder = builder.merge(Json::file(path));
        }

        let config = builder
            .merge(Env::prefixed("ACUITY_").split("__"))
            .extract()?;

        Ok(config)
    }
}
