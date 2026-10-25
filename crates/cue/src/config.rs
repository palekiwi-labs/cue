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

fn valid_segment(value: &str) -> bool {
    !value.starts_with(['.', '~'])
        && !value.contains(['*', '/', '\\'])
        && cuelib::head::validate_slug(value).is_ok()
}

fn project_root(cwd: &Path) -> anyhow::Result<Option<std::path::PathBuf>> {
    let output = match Command::new("git")
        .args(["rev-parse", "--show-toplevel"])
        // Git has no separate exit code for being outside a repository.
        .env("LC_ALL", "C")
        .current_dir(cwd)
        .output()
    {
        Ok(output) => output,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error).context("could not locate project config"),
    };
    if !output.status.success() {
        let error = String::from_utf8_lossy(&output.stderr);
        if error.starts_with("fatal: not a git repository (or any") {
            return Ok(None);
        }
        anyhow::bail!(
            "could not locate project config: git rev-parse failed: {}",
            error.trim()
        );
    }
    let root = String::from_utf8(output.stdout)
        .context("could not locate project config: Git toplevel is not valid UTF-8")?;
    Ok(Some(
        Path::new(root.trim_end_matches(['\r', '\n'])).to_path_buf(),
    ))
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
        if let Some(root) = project_root(cwd)? {
            config = config.merge(Json::file(root.join("cue.json")));
        }
        let config: Self = config.extract()?;
        for (name, matchers) in &config.scopes {
            for matcher in matchers {
                let pattern = matcher.strip_prefix('!').unwrap_or(matcher);
                let valid = pattern.split_once('/').is_some_and(|(org, repo)| {
                    !org.starts_with('!')
                        && valid_segment(org)
                        && (repo == "*" || valid_segment(repo))
                });
                anyhow::ensure!(
                    valid,
                    "invalid matcher '{matcher}' in scope '{name}': expected <org>/<repo> or <org>/*, optionally prefixed with !"
                );
            }
        }
        Ok(config)
    }
}
