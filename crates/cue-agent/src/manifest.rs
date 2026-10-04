//! The agent manifest: one JSON file per layer, holding supervisor settings
//! and every named agent.
//!
//! Two layers are read, global first and project-local second, and merged so
//! the local layer overrides field by field. Agents are a JSON object keyed by
//! agent name rather than an array, because a merge replaces an array wholesale
//! and a project file would then wipe every global agent instead of overriding
//! one field of one agent.
//!
//! Each layer is validated in full before merging, including fields a later
//! layer overrides. String fields may be file references; those are located
//! against the declaring manifest while parsing, and only the winning value
//! of each field is read once the layers are merged.

use crate::string_source::{Patch, StringSource, kind};
use anyhow::{Context, Result, bail};
use serde_json::{Map, Value};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

/// The project-local manifest filename, looked up from the working directory
/// upwards. Deliberately not `.cue/`: that name belonged to the abolished
/// per-repository store and reusing it would be actively confusing.
pub const PROJECT_MANIFEST: &str = "cue-agent.json";

/// The global manifest, relative to the config home.
const GLOBAL_MANIFEST: &str = "cue/cue-agent.json";

const ROOT_FIELDS: [&str; 3] = ["timeout", "worktree_root", "agents"];
const AGENT_FIELDS: [&str; 5] = ["description", "model", "system_prompt", "tools", "thinking"];

/// Which layer supplied an agent or one of its fields.
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

/// One agent as the manifest defines it, after layering and file resolution.
#[derive(Debug, Clone)]
pub struct Agent {
    pub name: String,
    pub description: Option<String>,
    pub model: Option<String>,
    /// Supplementary instructions appended to the harness's own prompt; empty
    /// when unset or cleared.
    pub system_prompt: String,
    /// `None` leaves the harness defaults alone; `Some(vec![])` means no tools.
    pub tools: Option<Vec<String>>,
    pub thinking: Option<String>,
    /// The last layer that mentioned the agent.
    pub source: Source,
    /// The layer that supplied each field still set after merging.
    pub field_sources: BTreeMap<&'static str, Source>,
}

/// The merged manifest and where its layers came from.
#[derive(Debug, Clone, Default)]
pub struct Manifest {
    agents: BTreeMap<String, Agent>,
    /// Per-run timeout in seconds; 0 means unlimited.
    pub timeout: u64,
    worktree_root: Option<String>,
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

    /// The configured worktree root for one execution target.
    ///
    /// A relative root resolves against the target directory (the task's cwd
    /// or the invocation directory), never against the manifest.
    #[allow(dead_code)] // consumed once worktree creation lands
    pub fn worktree_root(&self, target: &Path) -> Option<PathBuf> {
        self.worktree_root.as_ref().map(|root| target.join(root))
    }
}

/// One manifest layer, validated but with no referenced file read yet.
#[derive(Debug, Default)]
struct Layer {
    timeout: Option<u64>,
    worktree_root: Patch<StringSource>,
    agents: BTreeMap<String, AgentLayer>,
}

#[derive(Debug, Default)]
struct AgentLayer {
    description: Patch<StringSource>,
    model: Patch<StringSource>,
    system_prompt: Patch<StringSource>,
    tools: Patch<Vec<StringSource>>,
    thinking: Patch<StringSource>,
}

/// A merged field: its unread value and the layer that set it.
type Slot<T> = Option<(T, Source)>;

#[derive(Debug)]
struct MergedAgent {
    description: Slot<StringSource>,
    model: Slot<StringSource>,
    system_prompt: Slot<StringSource>,
    tools: Slot<Vec<StringSource>>,
    thinking: Slot<StringSource>,
    source: Source,
}

fn apply<T>(slot: &mut Slot<T>, patch: Patch<T>, source: Source) {
    match patch {
        Patch::Absent => {}
        Patch::Clear => *slot = None,
        Patch::Set(value) => *slot = Some((value, source)),
    }
}

/// Load and merge both manifest layers for a working directory.
pub fn load(cwd: &Path) -> Result<Manifest> {
    load_from(global_manifest_path(), find_project_manifest(cwd))
}

pub(crate) fn load_from(
    global_path: Option<PathBuf>,
    project_path: Option<PathBuf>,
) -> Result<Manifest> {
    let mut timeout = 0;
    let mut worktree_root: Slot<StringSource> = None;
    let mut merged: BTreeMap<String, MergedAgent> = BTreeMap::new();

    for (path, source) in [
        (global_path.as_ref(), Source::User),
        (project_path.as_ref(), Source::Project),
    ] {
        let Some(path) = path else { continue };
        if !path.is_file() {
            continue;
        }
        let layer = read_layer(path)?;
        if let Some(value) = layer.timeout {
            timeout = value;
        }
        apply(&mut worktree_root, layer.worktree_root, source);
        for (name, patch) in layer.agents {
            let agent = merged.entry(name).or_insert(MergedAgent {
                description: None,
                model: None,
                system_prompt: None,
                tools: None,
                thinking: None,
                source,
            });
            agent.source = source;
            apply(&mut agent.description, patch.description, source);
            apply(&mut agent.model, patch.model, source);
            apply(&mut agent.system_prompt, patch.system_prompt, source);
            apply(&mut agent.tools, patch.tools, source);
            apply(&mut agent.thinking, patch.thinking, source);
        }
    }

    let worktree_root = worktree_root
        .map(|(value, source)| {
            let declared = match source {
                Source::User => global_path.as_deref(),
                Source::Project => project_path.as_deref(),
            };
            value.resolve().with_context(|| {
                format!(
                    "Agent manifest {}: worktree_root",
                    declared.unwrap_or(Path::new("?")).display()
                )
            })
        })
        .transpose()?;

    let mut agents = BTreeMap::new();
    for (name, merged) in merged {
        let agent = resolve_agent(&name, merged).with_context(|| format!("Agent '{name}'"))?;
        agents.insert(name, agent);
    }

    Ok(Manifest {
        agents,
        timeout,
        worktree_root,
        global_path,
        project_path,
    })
}

/// Read the winning value of every field, recording where each came from.
fn resolve_agent(name: &str, merged: MergedAgent) -> Result<Agent> {
    let mut field_sources = BTreeMap::new();
    let mut text = |field: &'static str, slot: Slot<StringSource>| -> Result<Option<String>> {
        let Some((value, source)) = slot else {
            return Ok(None);
        };
        field_sources.insert(field, source);
        value.resolve().context(field).map(Some)
    };
    let description = text("description", merged.description)?;
    let model = text("model", merged.model)?;
    let system_prompt = text("system_prompt", merged.system_prompt)?.unwrap_or_default();
    let thinking = text("thinking", merged.thinking)?;
    let tools = match merged.tools {
        None => None,
        Some((items, source)) => {
            field_sources.insert("tools", source);
            let resolved = items
                .iter()
                .enumerate()
                .map(|(index, item)| item.resolve().with_context(|| format!("tools[{index}]")))
                .collect::<Result<Vec<_>>>()?;
            Some(resolved)
        }
    };
    Ok(Agent {
        name: name.to_string(),
        description,
        model,
        system_prompt,
        tools,
        thinking,
        source: merged.source,
        field_sources,
    })
}

/// Read and validate one layer. Relative file references are located against
/// the manifest's own directory here, because merging erases which file
/// declared a value.
fn read_layer(path: &Path) -> Result<Layer> {
    let raw = std::fs::read_to_string(path)
        .with_context(|| format!("Could not read agent manifest {}", path.display()))?;
    let value: Value = serde_json::from_str(&raw)
        .with_context(|| format!("Invalid JSON in agent manifest {}", path.display()))?;
    let base = path.parent().unwrap_or(Path::new("."));
    parse_layer(&value, base).with_context(|| format!("Agent manifest {}", path.display()))
}

fn parse_layer(value: &Value, base: &Path) -> Result<Layer> {
    let Value::Object(root) = value else {
        bail!("must be a JSON object with an 'agents' key");
    };
    reject_unknown(root, &ROOT_FIELDS, "the root")?;

    let mut layer = Layer::default();
    if let Some(value) = root.get("timeout") {
        let Some(secs) = value.as_u64() else {
            bail!(
                "timeout: expected a non-negative integer number of seconds, found {}",
                kind(value)
            );
        };
        layer.timeout = Some(secs);
    }
    if let Some(value) = root.get("worktree_root") {
        layer.worktree_root = optional_string(value, base, "worktree_root")?;
    }
    match root.get("agents") {
        None => {}
        Some(Value::Object(agents)) => {
            for (name, definition) in agents {
                let agent = parse_agent(definition, base, &format!("agents.{name}"))?;
                layer.agents.insert(name.clone(), agent);
            }
        }
        Some(Value::Array(_)) => {
            bail!("'agents' must be a JSON object keyed by agent name, not an array")
        }
        Some(other) => bail!(
            "'agents' must be a JSON object keyed by agent name, found {}",
            kind(other)
        ),
    }
    Ok(layer)
}

fn parse_agent(value: &Value, base: &Path, path: &str) -> Result<AgentLayer> {
    let Value::Object(fields) = value else {
        bail!(
            "{path}: an agent definition must be a JSON object, found {}",
            kind(value)
        );
    };
    reject_unknown(fields, &AGENT_FIELDS, path)?;

    let mut agent = AgentLayer::default();
    let string = |field: &str| -> Result<Patch<StringSource>> {
        match fields.get(field) {
            None => Ok(Patch::Absent),
            Some(value) => optional_string(value, base, &format!("{path}.{field}")),
        }
    };
    agent.description = string("description")?;
    agent.model = string("model")?;
    agent.system_prompt = string("system_prompt")?;
    agent.thinking = string("thinking")?;
    agent.tools = match fields.get("tools") {
        None => Patch::Absent,
        Some(Value::Null) => Patch::Clear,
        Some(Value::Array(items)) => Patch::Set(
            items
                .iter()
                .enumerate()
                .map(|(index, item)| {
                    StringSource::parse(item, base)
                        .with_context(|| format!("{path}.tools[{index}]"))
                })
                .collect::<Result<_>>()?,
        ),
        Some(other) => bail!(
            "{path}.tools: expected an array of strings or file references, found {}",
            kind(other)
        ),
    };
    Ok(agent)
}

/// A string field where null clears an inherited value.
fn optional_string(value: &Value, base: &Path, path: &str) -> Result<Patch<StringSource>> {
    if value.is_null() {
        return Ok(Patch::Clear);
    }
    StringSource::parse(value, base)
        .map(Patch::Set)
        .with_context(|| path.to_string())
}

fn reject_unknown(object: &Map<String, Value>, allowed: &[&str], at: &str) -> Result<()> {
    if let Some(key) = object.keys().find(|key| !allowed.contains(&key.as_str())) {
        bail!(
            "unknown field '{key}' in {at}; expected one of: {}",
            allowed.join(", ")
        );
    }
    Ok(())
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

    fn layers(global: &str, project: &str) -> (tempfile::TempDir, Result<Manifest>) {
        let dir = tempfile::tempdir().unwrap();
        let global_dir = dir.path().join("global");
        let project_dir = dir.path().join("project");
        std::fs::create_dir_all(&global_dir).unwrap();
        std::fs::create_dir_all(&project_dir).unwrap();
        let global_path = global_dir.join(PROJECT_MANIFEST);
        let project_path = project_dir.join(PROJECT_MANIFEST);
        std::fs::write(&global_path, global).unwrap();
        std::fs::write(&project_path, project).unwrap();
        std::fs::write(project_dir.join("root.txt"), "trees").unwrap();
        let manifest = load_from(Some(global_path), Some(project_path));
        (dir, manifest)
    }

    #[test]
    fn root_settings_default_to_no_timeout_and_no_worktree_root() {
        let manifest = load_from(None, None).unwrap();
        assert_eq!(manifest.timeout, 0);
        assert_eq!(manifest.worktree_root(Path::new("/target")), None);
        let (_dir, manifest) = layers("{}", "{}");
        let manifest = manifest.unwrap();
        assert_eq!(manifest.timeout, 0);
        assert_eq!(manifest.worktree_root(Path::new("/target")), None);
    }

    #[test]
    fn the_project_timeout_overrides_the_global_one() {
        let (_dir, manifest) = layers(r#"{"timeout": 30}"#, r#"{"timeout": 0}"#);
        assert_eq!(manifest.unwrap().timeout, 0);
        let (_dir, manifest) = layers(r#"{"timeout": 30}"#, "{}");
        assert_eq!(manifest.unwrap().timeout, 30);
    }

    #[test]
    fn a_relative_worktree_root_resolves_against_the_target_not_the_manifest() {
        let (_dir, manifest) = layers("{}", r#"{"worktree_root": "../trees"}"#);
        assert_eq!(
            manifest.unwrap().worktree_root(Path::new("/repo")),
            Some(PathBuf::from("/repo/../trees"))
        );
        let (_dir, manifest) = layers("{}", r#"{"worktree_root": "/abs/trees"}"#);
        assert_eq!(
            manifest.unwrap().worktree_root(Path::new("/repo")),
            Some(PathBuf::from("/abs/trees"))
        );
    }

    #[test]
    fn a_file_sourced_worktree_root_is_still_relative_to_the_target() {
        let (_dir, manifest) = layers("{}", r#"{"worktree_root": {"file": "root.txt"}}"#);
        assert_eq!(
            manifest.unwrap().worktree_root(Path::new("/repo")),
            Some(PathBuf::from("/repo/trees"))
        );
    }

    #[test]
    fn null_clears_an_inherited_worktree_root() {
        let (_dir, manifest) = layers(
            r#"{"worktree_root": {"file": "missing.txt"}}"#,
            r#"{"worktree_root": null}"#,
        );
        assert_eq!(manifest.unwrap().worktree_root(Path::new("/repo")), None);
    }
}
