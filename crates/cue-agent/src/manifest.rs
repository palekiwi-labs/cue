//! The agent manifest: one JSON file per layer, holding every agent.
//!
//! Two layers are read, global first and project-local second, and merged so
//! the local layer overrides field by field. Agents are a JSON object keyed by
//! agent name rather than an array, because a merge replaces an array wholesale
//! and a project file would then wipe every global agent instead of overriding
//! one field of one agent.

use anyhow::{bail, Context, Result};
use serde::Deserialize;
use serde_json::{Map, Value};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

/// The project-local manifest filename, looked up from the working directory
/// upwards. Deliberately not `.cue/`: that name belonged to the abolished
/// per-repository store and reusing it would be actively confusing.
pub const PROJECT_MANIFEST: &str = "cue-agent.json";

/// The global manifest, relative to the config home.
const GLOBAL_MANIFEST: &str = "cue/cue-agent.json";

/// Which layer last defined an agent.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Source {
    User,
    Project,
}

impl Source {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::User => "user",
            Self::Project => "project",
        }
    }
}

/// One agent as the manifest defines it, after layering.
#[derive(Debug, Clone)]
pub struct Agent {
    pub name: String,
    pub description: Option<String>,
    pub model: Option<String>,
    pub system_prompt: String,
    pub tools: Vec<String>,
    pub thinking: Option<String>,
    pub timeout_secs: Option<u64>,
    pub source: Source,
}

/// The merged manifest and where its layers came from.
#[derive(Debug, Clone, Default)]
pub struct Manifest {
    agents: BTreeMap<String, Agent>,
    pub global_path: Option<PathBuf>,
    pub project_path: Option<PathBuf>,
}

impl Manifest {
    pub fn agents(&self) -> impl Iterator<Item = &Agent> {
        self.agents.values()
    }

    pub fn get(&self, name: &str) -> Option<&Agent> {
        self.agents.get(name)
    }

    /// Resolve an agent by name, naming the alternatives when it is unknown.
    ///
    /// Exact names only: alias and prefix matching would have to decide what an
    /// ambiguous abbreviation means, and that behaviour is not yet designed.
    pub fn resolve(&self, name: &str) -> Result<&Agent> {
        self.get(name).ok_or_else(|| {
            let available: Vec<&str> = self.agents.keys().map(String::as_str).collect();
            let available = if available.is_empty() {
                "none".to_string()
            } else {
                available.join(", ")
            };
            anyhow::anyhow!("Unknown agent '{name}'. Available agents: {available}")
        })
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawAgent {
    #[serde(default)]
    description: Option<String>,
    #[serde(default)]
    model: Option<String>,
    #[serde(default)]
    system_prompt: Option<String>,
    #[serde(default)]
    system_prompt_file: Option<String>,
    #[serde(default)]
    tools: Option<Vec<String>>,
    #[serde(default)]
    thinking: Option<String>,
    #[serde(default)]
    timeout_secs: Option<u64>,
}

/// Load and merge both manifest layers for a working directory.
pub fn load(cwd: &Path) -> Result<Manifest> {
    let global_path = global_manifest_path();
    let project_path = find_project_manifest(cwd);

    let mut merged = Map::new();
    let mut sources: BTreeMap<String, Source> = BTreeMap::new();

    for (path, source) in [
        (global_path.as_ref(), Source::User),
        (project_path.as_ref(), Source::Project),
    ] {
        let Some(path) = path else { continue };
        if !path.is_file() {
            continue;
        }
        let layer = read_layer(path)?;
        for name in layer.keys() {
            sources.insert(name.clone(), source);
        }
        merge_objects(&mut merged, layer);
    }

    let mut agents = BTreeMap::new();
    for (name, value) in merged {
        let raw: RawAgent = serde_json::from_value(value)
            .with_context(|| format!("Invalid definition for agent '{name}'"))?;
        let system_prompt = match (&raw.system_prompt, &raw.system_prompt_file) {
            (Some(_), Some(_)) => bail!(
                "Agent '{name}' sets both system_prompt and system_prompt_file; keep one of them"
            ),
            (Some(text), None) => text.clone(),
            (None, Some(file)) => std::fs::read_to_string(file)
                .with_context(|| format!("Agent '{name}': could not read system prompt {file}"))?,
            (None, None) => String::new(),
        };
        let source = sources.get(&name).copied().unwrap_or(Source::User);
        agents.insert(
            name.clone(),
            Agent {
                name,
                description: raw.description,
                model: raw.model,
                system_prompt,
                tools: raw.tools.unwrap_or_default(),
                thinking: raw.thinking,
                timeout_secs: raw.timeout_secs,
                source,
            },
        );
    }

    Ok(Manifest {
        agents,
        global_path,
        project_path,
    })
}

/// Read one layer into its `agents` object, with `system_prompt_file` rewritten
/// to an absolute path.
///
/// The rewrite happens per layer because merging erases which file supplied a
/// field, and a relative prompt path only means something next to the manifest
/// that wrote it.
fn read_layer(path: &Path) -> Result<Map<String, Value>> {
    let raw = std::fs::read_to_string(path)
        .with_context(|| format!("Could not read agent manifest {}", path.display()))?;
    let value: Value = serde_json::from_str(&raw)
        .with_context(|| format!("Invalid JSON in agent manifest {}", path.display()))?;
    let Value::Object(root) = value else {
        bail!(
            "Agent manifest {} must be a JSON object with an 'agents' key",
            path.display()
        );
    };
    let Some(agents) = root.get("agents") else {
        return Ok(Map::new());
    };
    let Value::Object(agents) = agents else {
        bail!(
            "Agent manifest {}: 'agents' must be a JSON object keyed by agent name, not an array",
            path.display()
        );
    };

    let dir = path.parent().unwrap_or(Path::new("."));
    let mut agents = agents.clone();
    for (name, definition) in &mut agents {
        let Some(entry) = definition.as_object_mut() else {
            continue;
        };
        let inline = entry.get("system_prompt").is_some_and(|v| !v.is_null());
        let file = entry
            .get("system_prompt_file")
            .is_some_and(|v| !v.is_null());
        if inline && file {
            bail!(
                "Agent '{name}' in {} sets both system_prompt and system_prompt_file; keep one of them",
                path.display()
            );
        }
        // A prompt source is one logical field, regardless of its representation.
        // Clear the inherited alternative without reading an overridden file.
        if inline {
            entry.insert("system_prompt_file".into(), Value::Null);
        } else if file {
            entry.insert("system_prompt".into(), Value::Null);
        }
        if let Some(Value::String(file)) = entry.get("system_prompt_file") {
            let resolved = dir.join(file);
            entry.insert(
                "system_prompt_file".to_string(),
                Value::String(resolved.to_string_lossy().into_owned()),
            );
        }
    }
    Ok(agents)
}

/// Recursively merge `overlay` into `base`: objects merge key by key, every
/// other value (arrays included) is replaced outright.
fn merge_objects(base: &mut Map<String, Value>, overlay: Map<String, Value>) {
    for (key, value) in overlay {
        match (base.get_mut(&key), value) {
            (Some(Value::Object(existing)), Value::Object(incoming)) => {
                merge_objects(existing, incoming);
            }
            (_, value) => {
                base.insert(key, value);
            }
        }
    }
}

fn global_manifest_path() -> Option<PathBuf> {
    config_home().map(|home| home.join(GLOBAL_MANIFEST))
}

fn config_home() -> Option<PathBuf> {
    if let Some(value) = std::env::var_os("XDG_CONFIG_HOME").filter(|value| !value.is_empty()) {
        return Some(PathBuf::from(value));
    }
    dirs::home_dir().map(|home| home.join(".config"))
}

/// Find the nearest project manifest, walking upwards and stopping at the
/// repository root so a manifest above an unrelated checkout is never adopted.
fn find_project_manifest(cwd: &Path) -> Option<PathBuf> {
    let mut dir = cwd;
    loop {
        let candidate = dir.join(PROJECT_MANIFEST);
        if candidate.is_file() {
            return Some(candidate);
        }
        if dir.join(".git").exists() {
            return None;
        }
        dir = dir.parent()?;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn object(json: &str) -> Map<String, Value> {
        match serde_json::from_str(json).unwrap() {
            Value::Object(map) => map,
            other => panic!("not an object: {other}"),
        }
    }

    #[test]
    fn merging_overrides_fields_rather_than_filling_gaps() {
        let mut base = object(r#"{"explore": {"model": "sonnet", "system_prompt": "p"}}"#);
        merge_objects(&mut base, object(r#"{"explore": {"model": "haiku"}}"#));

        assert_eq!(base["explore"]["model"], "haiku");
        assert_eq!(base["explore"]["system_prompt"], "p");
    }

    #[test]
    fn merging_replaces_arrays_wholesale() {
        let mut base = object(r#"{"explore": {"tools": ["read", "bash"]}}"#);
        merge_objects(&mut base, object(r#"{"explore": {"tools": ["read"]}}"#));

        assert_eq!(base["explore"]["tools"], serde_json::json!(["read"]));
    }

    #[test]
    fn merging_keeps_agents_the_overlay_never_mentions() {
        let mut base = object(r#"{"explore": {}, "consultant": {}}"#);
        merge_objects(&mut base, object(r#"{"explore": {"model": "haiku"}}"#));

        assert_eq!(base.len(), 2);
        assert!(base.contains_key("consultant"));
    }
}
