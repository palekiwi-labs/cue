use figment::{
    providers::{Env, Format, Json, Serialized},
    Figment,
};
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Config {
    pub gotify_url: String,
    pub port: u16,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            gotify_url: "http://localhost".into(),
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

        // NOTE: ACUITY_GOTIFY_TOKEN is intentionally NOT a Config field.
        // It is read manually in main() via env::var. The figment
        // Env::prefixed("ACUITY_") layer below will encounter the var
        // but silently ignore it because there is no matching field.
        // If gotify_token is ever added to Config, the two reads would
        // silently diverge. Keep the token out of Config by design.
        let mut config: Config = builder
            .merge(Env::prefixed("ACUITY_").split("__"))
            .extract()?;

        // Normalize: strip any trailing slash so both
        // "http://localhost" and "http://localhost/" produce
        // "{gotify_url}/message" without a double slash.
        while config.gotify_url.ends_with('/') {
            config.gotify_url.pop();
        }

        Ok(config)
    }
}
