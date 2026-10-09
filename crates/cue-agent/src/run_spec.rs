//! The run specification: admission and resolution of the tasks envelope.
//!
//! The JSON names the requested work; this module turns it into fully
//! resolved tasks before anything is launched or created. Every failure here
//! is an admission failure that rejects the whole request, so every effective
//! file reference (the winning value after defaults and overrides) is read,
//! all strings that reach argv or the environment are checked, and every
//! capture context is validated up front.

use crate::manifest::{Agent, Manifest};
use crate::string_source::{Patch, StringSource, kind};
use anyhow::{Context, Result, bail};
use serde_json::{Map, Value};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

/// The per-invocation task limit. There is no queue: a larger request is
/// rejected rather than deferred.
pub const MAX_TASKS: usize = 4;

/// Where an admitted request gets its relative paths and capture contexts.
#[derive(Debug, Clone, Copy)]
pub struct Bases<'a> {
    /// The directory relative file references resolve against: the
    /// specification file's directory, or the invocation directory for inline
    /// and standard-input specifications.
    pub references: &'a Path,
    /// The invocation directory, the effective cwd when none is given.
    pub invocation: &'a Path,
    /// The cue store root capture contexts must exist in.
    pub store: &'a Path,
}

/// An admitted request.
#[derive(Debug, Clone)]
pub struct ResolvedBatch {
    pub label: Option<String>,
    pub tasks: Vec<ResolvedTask>,
}

/// One execution, with defaults and agent overrides applied.
#[derive(Debug, Clone)]
pub struct ResolvedTask {
    /// The named agent with any run overrides applied.
    pub agent: Agent,
    pub prompt: String,
    /// Absolute: the task's or default cwd, else the invocation directory.
    pub cwd: PathBuf,
    /// Overlay on the inherited process environment; `None` removes the
    /// variable.
    pub env: BTreeMap<String, Option<String>>,
    /// Canonical `<org>/<repo>/<context>` capture destination.
    pub context: Option<String>,
    pub label: Option<String>,
    pub worktree: Option<Worktree>,
}

/// A requested new worktree. Validated here; provisioning is separate.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Worktree {
    pub base: String,
    pub ephemeral: bool,
    /// Exact destination, resolved against the task's effective cwd.
    pub path: Option<PathBuf>,
    pub branch: Option<String>,
}

const ROOT_FIELDS: [&str; 3] = ["tasks", "defaults", "label"];
const WORKTREE_FIELDS: [&str; 4] = ["base", "ephemeral", "path", "branch"];
/// Fields shared by defaults and tasks; only a task names its agent.
const LAYER_FIELDS: [&str; 11] = [
    "prompt",
    "label",
    "cwd",
    "env",
    "context",
    "worktree",
    "description",
    "model",
    "system_prompt",
    "tools",
    "thinking",
];

/// Admit and resolve a run specification.
pub fn resolve(raw: &str, bases: Bases, manifest: &Manifest) -> Result<ResolvedBatch> {
    let value: Value =
        serde_json::from_str(raw).context("The run specification is not valid JSON")?;
    let Value::Object(root) = value else {
        bail!(
            "The run specification must be a JSON object with a 'tasks' array, found {}",
            kind(&value)
        );
    };
    reject_unknown(&root, &ROOT_FIELDS, "the root")?;
    let refs = bases.references;

    let label = match root.get("label") {
        None | Some(Value::Null) => None,
        Some(value) => Some(StringSource::parse(value, refs).context("label")?),
    };

    let defaults = match root.get("defaults") {
        None => Layer::default(),
        Some(Value::Object(fields)) => {
            if fields.contains_key("agent") {
                bail!("defaults cannot select an agent; name it on every task");
            }
            reject_unknown(fields, &LAYER_FIELDS, "defaults")?;
            parse_layer(fields, refs, "defaults")?
        }
        Some(other) => bail!("defaults: expected an object, found {}", kind(other)),
    };

    let entries = match root.get("tasks") {
        Some(Value::Array(entries)) => entries,
        None => bail!("The run specification needs a 'tasks' array"),
        Some(other) => bail!("tasks: expected an array, found {}", kind(other)),
    };
    if entries.is_empty() {
        bail!("tasks: at least one task is required");
    }
    if entries.len() > MAX_TASKS {
        bail!(
            "tasks: {} requested, but at most {MAX_TASKS} run per invocation; \
             split the request, cue-agent does not queue the excess",
            entries.len()
        );
    }

    // Parse every task before reading any file, so a structural error in a
    // late task is reported without touching the filesystem.
    let mut parsed = Vec::with_capacity(entries.len());
    for (index, entry) in entries.iter().enumerate() {
        let at = format!("tasks[{index}]");
        let Value::Object(fields) = entry else {
            bail!("{at}: expected an object, found {}", kind(entry));
        };
        reject_unknown(fields, &[&["agent"][..], &LAYER_FIELDS].concat(), &at)?;
        let agent = match fields.get("agent") {
            None | Some(Value::Null) => bail!("{at}.agent: a named agent is required"),
            Some(value) => {
                StringSource::parse(value, refs).with_context(|| format!("{at}.agent"))?
            }
        };
        parsed.push((at.clone(), agent, parse_layer(fields, refs, &at)?));
    }

    let label = label
        .map(|source| read_text(&source, "label"))
        .transpose()?;
    let tasks = parsed
        .into_iter()
        .map(|(at, agent, layer)| resolve_task(&at, agent, layer.over(&defaults), bases, manifest))
        .collect::<Result<_>>()?;
    Ok(ResolvedBatch { label, tasks })
}

/// The fields one level (defaults or a task) sets, clears or leaves alone.
#[derive(Debug, Default)]
struct Layer {
    prompt: Patch<StringSource>,
    label: Patch<StringSource>,
    cwd: Patch<StringSource>,
    /// Per-variable entries; `None` removes the variable. Clearing the whole
    /// field drops any default overlay.
    env: Patch<EnvSource>,
    context: Patch<StringSource>,
    worktree: Patch<WorktreeSource>,
    description: Patch<StringSource>,
    model: Patch<StringSource>,
    system_prompt: Patch<StringSource>,
    tools: Patch<Vec<StringSource>>,
    thinking: Patch<StringSource>,
}

fn parse_layer(fields: &Map<String, Value>, refs: &Path, at: &str) -> Result<Layer> {
    let string = |field: &str| -> Result<Patch<StringSource>> {
        match fields.get(field) {
            None => Ok(Patch::Absent),
            Some(Value::Null) => Ok(Patch::Clear),
            Some(value) => StringSource::parse(value, refs)
                .map(Patch::Set)
                .with_context(|| format!("{at}.{field}")),
        }
    };
    // Arrays replace whole: a task's tools never extend the inherited list.
    let tools = match fields.get("tools") {
        None => Patch::Absent,
        Some(Value::Null) => Patch::Clear,
        Some(Value::Array(items)) => Patch::Set(
            items
                .iter()
                .enumerate()
                .map(|(index, item)| {
                    StringSource::parse(item, refs).with_context(|| format!("{at}.tools[{index}]"))
                })
                .collect::<Result<_>>()?,
        ),
        Some(other) => bail!(
            "{at}.tools: expected an array of strings or file references, found {}",
            kind(other)
        ),
    };
    Ok(Layer {
        prompt: string("prompt")?,
        label: string("label")?,
        cwd: string("cwd")?,
        env: parse_env(fields.get("env"), refs, at)?,
        context: string("context")?,
        worktree: match fields.get("worktree") {
            None => Patch::Absent,
            Some(Value::Null) => Patch::Clear,
            Some(value) => {
                Patch::Set(parse_worktree(value, refs).with_context(|| format!("{at}.worktree"))?)
            }
        },
        description: string("description")?,
        model: string("model")?,
        system_prompt: string("system_prompt")?,
        tools,
        thinking: string("thinking")?,
    })
}

impl Layer {
    /// A task's effective fields: its own where it mentions them, else the
    /// defaults. Arrays and file references replace as single values.
    fn over(self, defaults: &Layer) -> Layer {
        Layer {
            prompt: pick(self.prompt, &defaults.prompt),
            label: pick(self.label, &defaults.label),
            cwd: pick(self.cwd, &defaults.cwd),
            env: merge_env(self.env, &defaults.env),
            context: pick(self.context, &defaults.context),
            worktree: pick(self.worktree, &defaults.worktree),
            description: pick(self.description, &defaults.description),
            model: pick(self.model, &defaults.model),
            system_prompt: pick(self.system_prompt, &defaults.system_prompt),
            tools: pick(self.tools, &defaults.tools),
            thinking: pick(self.thinking, &defaults.thinking),
        }
    }
}

/// The task's own value when it mentions the field, else the default.
fn pick<T: Clone>(task: Patch<T>, default: &Patch<T>) -> Patch<T> {
    match task {
        Patch::Absent => default.clone(),
        explicit => explicit,
    }
}

/// Read the source behind a winning value; only winners are ever opened.
fn read(patch: Patch<StringSource>, at: &str, field: &str) -> Result<Patch<String>> {
    Ok(match patch {
        Patch::Absent => Patch::Absent,
        Patch::Clear => Patch::Clear,
        Patch::Set(source) => Patch::Set(read_text(&source, &format!("{at}.{field}"))?),
    })
}

/// Read one string and refuse NUL bytes, which no argv entry, environment
/// value or path can carry.
fn read_text(source: &StringSource, at: &str) -> Result<String> {
    let text = source.resolve().with_context(|| at.to_string())?;
    if text.contains('\0') {
        bail!("{at}: contains a NUL byte");
    }
    Ok(text)
}

/// Environment entries before any file is read; `None` removes a variable.
type EnvSource = BTreeMap<String, Option<StringSource>>;

/// A task's env merges over the defaults by variable. A task's whole-field
/// null discards the default overlay, leaving the inherited environment
/// unchanged; a null default is simply no overlay.
fn merge_env(task: Patch<EnvSource>, defaults: &Patch<EnvSource>) -> Patch<EnvSource> {
    match (task, defaults) {
        (Patch::Absent, defaults) => defaults.clone(),
        (Patch::Clear, _) => Patch::Clear,
        (Patch::Set(own), Patch::Set(shared)) => {
            Patch::Set(shared.clone().into_iter().chain(own).collect())
        }
        (Patch::Set(own), Patch::Absent | Patch::Clear) => Patch::Set(own),
    }
}

fn parse_env(value: Option<&Value>, refs: &Path, at: &str) -> Result<Patch<EnvSource>> {
    let entries = match value {
        None => return Ok(Patch::Absent),
        Some(Value::Null) => return Ok(Patch::Clear),
        Some(Value::Object(entries)) => entries,
        Some(other) => bail!("{at}.env: expected an object, found {}", kind(other)),
    };
    let mut env = BTreeMap::new();
    for (key, value) in entries {
        if key.is_empty() {
            bail!("{at}.env: a variable name must not be empty");
        }
        if key.contains('=') {
            bail!("{at}.env: variable name '{key}' contains '='");
        }
        if key.contains('\0') {
            bail!("{at}.env: a variable name contains a NUL byte");
        }
        let source = match value {
            Value::Null => None,
            value => {
                Some(StringSource::parse(value, refs).with_context(|| format!("{at}.env.{key}"))?)
            }
        };
        env.insert(key.clone(), source);
    }
    Ok(Patch::Set(env))
}

fn read_list(patch: Patch<Vec<StringSource>>, at: &str, field: &str) -> Result<Patch<Vec<String>>> {
    Ok(match patch {
        Patch::Absent => Patch::Absent,
        Patch::Clear => Patch::Clear,
        Patch::Set(items) => Patch::Set(
            items
                .iter()
                .enumerate()
                .map(|(index, item)| read_text(item, &format!("{at}.{field}[{index}]")))
                .collect::<Result<_>>()?,
        ),
    })
}

/// A winning optional value; absent and cleared both mean none.
fn optional<T>(patch: Patch<T>) -> Option<T> {
    match patch {
        Patch::Set(value) => Some(value),
        Patch::Absent | Patch::Clear => None,
    }
}

/// Apply one override to an agent field. An overridden field no longer comes
/// from a manifest layer, so its provenance is dropped.
fn override_field<T>(
    agent: &mut Agent,
    field: &'static str,
    patch: Patch<T>,
    apply: impl FnOnce(&mut Agent, Option<T>),
) {
    if !matches!(patch, Patch::Absent) {
        agent.field_sources.remove(field);
        apply(agent, optional(patch));
    }
}

fn resolve_task(
    at: &str,
    agent: StringSource,
    layer: Layer,
    bases: Bases,
    manifest: &Manifest,
) -> Result<ResolvedTask> {
    let name = agent.resolve().with_context(|| format!("{at}.agent"))?;
    let mut agent = manifest
        .resolve(&name)
        .with_context(|| format!("{at}.agent"))?
        .clone();

    let description = read(layer.description, at, "description")?;
    let model = read(layer.model, at, "model")?;
    let system_prompt = read(layer.system_prompt, at, "system_prompt")?;
    let thinking = read(layer.thinking, at, "thinking")?;
    let tools = read_list(layer.tools, at, "tools")?;
    override_field(&mut agent, "description", description, |a, v| {
        a.description = v
    });
    override_field(&mut agent, "model", model, |a, v| a.model = v);
    override_field(&mut agent, "system_prompt", system_prompt, |a, v| {
        a.system_prompt = v.unwrap_or_default()
    });
    override_field(&mut agent, "thinking", thinking, |a, v| a.thinking = v);
    override_field(&mut agent, "tools", tools, |a, v| a.tools = v);
    // Overrides were checked as they were read; values inherited from the
    // manifest reach argv too, so the effective agent is checked as a whole.
    let effective = [("model", &agent.model), ("thinking", &agent.thinking)];
    for (field, value) in effective {
        if value.as_deref().is_some_and(|text| text.contains('\0')) {
            bail!("{at}.{field}: contains a NUL byte");
        }
    }
    for (index, tool) in agent.tools.iter().flatten().enumerate() {
        if tool.contains('\0') {
            bail!("{at}.tools[{index}]: contains a NUL byte");
        }
    }

    let Some(prompt) = optional(read(layer.prompt, at, "prompt")?) else {
        bail!("{at}.prompt: a prompt is required");
    };
    if prompt.trim().is_empty() {
        bail!("{at}.prompt: the prompt is empty");
    }
    if prompt.starts_with(['-', '@']) {
        bail!(
            "{at}.prompt: starts with '-' or '@', which Pi interprets as an option or \
             file; prepend ordinary instruction text"
        );
    }

    let label = optional(read(layer.label, at, "label")?);

    let cwd = match optional(read(layer.cwd, at, "cwd")?) {
        None => bases.invocation.to_path_buf(),
        Some(cwd) if Path::new(&cwd).is_absolute() => PathBuf::from(cwd),
        Some(cwd) => bail!("{at}.cwd: '{cwd}' is not an absolute path"),
    };

    let worktree = match layer.worktree {
        Patch::Set(source) => {
            Some(resolve_worktree(source, &cwd).with_context(|| format!("{at}.worktree"))?)
        }
        Patch::Absent | Patch::Clear => None,
    };
    // A destination is either explicit or generated beneath the configured
    // root; with neither, the request cannot be resolved.
    if let Some(worktree) = &worktree
        && worktree.path.is_none()
        && manifest.worktree_root(&cwd).is_none()
    {
        bail!(
            "{at}.worktree: no path given and no worktree_root configured in the agent \
             manifest; set worktree.path or worktree_root"
        );
    }

    // Capture never assigns or clears CUE_CONTEXT: env stays as written.
    let context = optional(read(layer.context, at, "context")?);
    if let Some(address) = &context {
        validate_context(address, bases.store).with_context(|| format!("{at}.context"))?;
    }

    let mut env = BTreeMap::new();
    for (key, source) in optional(layer.env).unwrap_or_default() {
        let value = source
            .map(|source| read_text(&source, &format!("{at}.env.{key}")))
            .transpose()?;
        env.insert(key, value);
    }

    Ok(ResolvedTask {
        agent,
        prompt,
        cwd,
        env,
        context,
        label,
        worktree,
    })
}

/// A worktree request before its strings are read. The object replaces an
/// inherited one whole, so its optional members never fall through.
#[derive(Debug, Clone)]
struct WorktreeSource {
    base: StringSource,
    ephemeral: bool,
    path: Option<StringSource>,
    branch: Option<StringSource>,
}

fn parse_worktree(value: &Value, refs: &Path) -> Result<WorktreeSource> {
    let Value::Object(fields) = value else {
        bail!("expected an object, found {}", kind(value));
    };
    reject_unknown(fields, &WORKTREE_FIELDS, "worktree")?;
    let string = |field: &str| -> Result<Option<StringSource>> {
        match fields.get(field) {
            None | Some(Value::Null) => Ok(None),
            Some(value) => StringSource::parse(value, refs)
                .map(Some)
                .with_context(|| field.to_string()),
        }
    };
    let Some(base) = string("base")? else {
        bail!("base: a base revision is required");
    };
    let ephemeral = match fields.get("ephemeral") {
        Some(Value::Bool(ephemeral)) => *ephemeral,
        None | Some(Value::Null) => bail!("ephemeral: a boolean is required"),
        Some(other) => bail!("ephemeral: expected a boolean, found {}", kind(other)),
    };
    Ok(WorktreeSource {
        base,
        ephemeral,
        path: string("path")?,
        branch: string("branch")?,
    })
}

/// Read a worktree request; a relative path is relative to the task's
/// effective cwd, never to the specification or the Git root.
fn resolve_worktree(source: WorktreeSource, cwd: &Path) -> Result<Worktree> {
    let nonempty = |source: &StringSource, field: &str| -> Result<String> {
        let text = read_text(source, field)?;
        if text.is_empty() {
            bail!("{field}: must not be empty");
        }
        Ok(text)
    };
    Ok(Worktree {
        base: nonempty(&source.base, "base")?,
        ephemeral: source.ephemeral,
        path: source
            .path
            .map(|path| nonempty(&path, "path").map(|path| cwd.join(path)))
            .transpose()?,
        branch: source
            .branch
            .map(|branch| nonempty(&branch, "branch"))
            .transpose()?,
    })
}

/// A capture destination is a canonical `<org>/<repo>/<context>` address of a
/// context that already exists; the store is only read, never written.
fn validate_context(address: &str, store: &Path) -> Result<()> {
    const FORM: &str = "expected a canonical context address '<org>/<repo>/<context>'";
    if address.starts_with(['/', '~']) {
        bail!("'{address}' is a path, not an address; {FORM}");
    }
    let segments: Vec<&str> = address.split('/').collect();
    if segments.len() != 3
        || segments
            .iter()
            .any(|segment| segment.trim().is_empty() || matches!(*segment, "." | ".."))
    {
        bail!("'{address}' is not a context address; {FORM}");
    }
    if !store.join(address).join("context.md").is_file() {
        bail!(
            "no context '{address}' in the cue store {}",
            store.display()
        );
    }
    Ok(())
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::manifest::{self, PROJECT_MANIFEST};

    struct Fixture {
        dir: tempfile::TempDir,
        manifest: Manifest,
    }

    impl Fixture {
        fn new() -> Self {
            Self::with_manifest(
                r#"{"agents": {
                    "scout": {"model": "m1", "system_prompt": "be brief",
                              "tools": ["read"], "thinking": "low",
                              "description": "looks around"},
                    "plain": {}
                }}"#,
            )
        }

        fn with_manifest(json: &str) -> Self {
            let dir = tempfile::tempdir().unwrap();
            for sub in ["spec", "invocation", "store", "manifest"] {
                std::fs::create_dir_all(dir.path().join(sub)).unwrap();
            }
            let path = dir.path().join("manifest").join(PROJECT_MANIFEST);
            std::fs::write(&path, json).unwrap();
            let manifest = manifest::load_from(None, Some(path)).unwrap();
            Self { dir, manifest }
        }

        fn path(&self, sub: &str) -> PathBuf {
            self.dir.path().join(sub)
        }

        fn write(&self, rel: &str, text: &str) {
            let path = self.path(rel);
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(path, text).unwrap();
        }

        fn context(&self, address: &str) {
            self.write(&format!("store/{address}/context.md"), "---\n---\n");
        }

        fn resolve(&self, json: &str) -> Result<ResolvedBatch> {
            let references = self.path("spec");
            let invocation = self.path("invocation");
            let store = self.path("store");
            resolve(
                json,
                Bases {
                    references: &references,
                    invocation: &invocation,
                    store: &store,
                },
                &self.manifest,
            )
        }

        fn ok(&self, json: &str) -> ResolvedBatch {
            match self.resolve(json) {
                Ok(batch) => batch,
                Err(err) => panic!("{json} should be admitted: {err:#}"),
            }
        }

        fn err(&self, json: &str) -> String {
            match self.resolve(json) {
                Ok(batch) => panic!("{json} should be rejected, got {batch:?}"),
                Err(err) => format!("{err:#}"),
            }
        }
    }

    #[test]
    fn a_minimal_task_takes_the_named_agent_and_the_invocation_directory() {
        let fx = Fixture::new();
        let batch = fx.ok(r#"{"tasks": [{"agent": "scout", "prompt": "look"}]}"#);
        assert_eq!(batch.label, None);
        assert_eq!(batch.tasks.len(), 1);
        let task = &batch.tasks[0];
        assert_eq!(task.agent.name, "scout");
        assert_eq!(task.agent.model.as_deref(), Some("m1"));
        assert_eq!(task.agent.system_prompt, "be brief");
        assert_eq!(task.agent.tools, Some(vec!["read".to_string()]));
        assert_eq!(task.prompt, "look");
        assert_eq!(task.cwd, fx.path("invocation"));
        assert!(task.env.is_empty());
        assert_eq!(task.context, None);
        assert_eq!(task.label, None);
        assert_eq!(task.worktree, None);
    }

    #[test]
    fn tasks_keep_specification_order_and_may_repeat_an_agent() {
        let fx = Fixture::new();
        let batch = fx.ok(r#"{"label": "sweep", "tasks": [
                {"agent": "scout", "prompt": "one", "label": "first"},
                {"agent": "plain", "prompt": "two"},
                {"agent": "scout", "prompt": "three"}
            ]}"#);
        assert_eq!(batch.label.as_deref(), Some("sweep"));
        let names: Vec<_> = batch.tasks.iter().map(|t| t.agent.name.as_str()).collect();
        assert_eq!(names, ["scout", "plain", "scout"]);
        let prompts: Vec<_> = batch.tasks.iter().map(|t| t.prompt.as_str()).collect();
        assert_eq!(prompts, ["one", "two", "three"]);
        assert_eq!(batch.tasks[0].label.as_deref(), Some("first"));
        assert_eq!(batch.tasks[1].label, None);
    }

    #[test]
    fn the_envelope_is_strict() {
        let fx = Fixture::new();
        let task = r#"{"agent": "scout", "prompt": "p"}"#;
        let five = [task; 5].join(",");
        for (json, expected) in [
            ("not json", "JSON"),
            (r#"[{"agent": "scout", "prompt": "p"}]"#, "object"),
            ("{}", "tasks"),
            (r#"{"tasks": null}"#, "tasks"),
            (r#"{"tasks": {}}"#, "tasks"),
            (r#"{"tasks": []}"#, "at least one"),
            (&format!(r#"{{"tasks": [{five}]}}"#), "4"),
            (&format!(r#"{{"tasks": [{task}], "runs": []}}"#), "runs"),
            (
                &format!(r#"{{"tasks": [{task}], "defaults": 1}}"#),
                "defaults",
            ),
            (&format!(r#"{{"tasks": [{task}], "label": 1}}"#), "label"),
            (r#"{"tasks": [1]}"#, "tasks[0]"),
        ] {
            let error = fx.err(json);
            assert!(error.contains(expected), "{json}: {error}");
        }
    }

    #[test]
    fn every_task_names_a_known_agent_and_a_prompt() {
        let fx = Fixture::new();
        for (json, expected) in [
            (r#"{"tasks": [{"prompt": "p"}]}"#, "tasks[0].agent"),
            (
                r#"{"tasks": [{"agent": null, "prompt": "p"}]}"#,
                "tasks[0].agent",
            ),
            (r#"{"tasks": [{"agent": "ghost", "prompt": "p"}]}"#, "ghost"),
            (r#"{"tasks": [{"agent": "scout"}]}"#, "tasks[0].prompt"),
            (
                r#"{"tasks": [{"agent": "scout", "prompt": null}]}"#,
                "tasks[0].prompt",
            ),
            (
                r#"{"tasks": [{"agent": "scout", "prompt": " \n"}]}"#,
                "empty",
            ),
            (r#"{"tasks": [{"agent": "scout", "prompt": "-x"}]}"#, "'-'"),
            (r#"{"tasks": [{"agent": "scout", "prompt": "@f"}]}"#, "'@'"),
            (
                r#"{"tasks": [{"agent": "scout", "prompt": "p", "timeout": 5}]}"#,
                "timeout",
            ),
            (
                r#"{"tasks": [{"agent": "scout", "prompt": "p", "timeout_secs": 5}]}"#,
                "timeout_secs",
            ),
        ] {
            let error = fx.err(json);
            assert!(error.contains(expected), "{json}: {error}");
        }
    }

    #[test]
    fn defaults_cannot_select_an_agent_or_set_a_timeout() {
        let fx = Fixture::new();
        for json in [
            r#"{"defaults": {"agent": "scout"}, "tasks": [{"prompt": "p"}]}"#,
            r#"{"defaults": {"agent": "scout"}, "tasks": [{"agent": "scout", "prompt": "p"}]}"#,
            r#"{"defaults": {"timeout": 1}, "tasks": [{"agent": "scout", "prompt": "p"}]}"#,
        ] {
            let error = fx.err(json);
            assert!(error.contains("defaults"), "{json}: {error}");
        }
    }

    #[test]
    fn task_fields_beat_defaults_and_null_clears_an_optional_default() {
        let fx = Fixture::new();
        let batch = fx.ok(
            r#"{"defaults": {"prompt": "shared", "label": "common"}, "tasks": [
                {"agent": "scout"},
                {"agent": "scout", "prompt": "own", "label": "mine"},
                {"agent": "scout", "label": null}
            ]}"#,
        );
        let prompts: Vec<_> = batch.tasks.iter().map(|t| t.prompt.as_str()).collect();
        assert_eq!(prompts, ["shared", "own", "shared"]);
        let labels: Vec<_> = batch.tasks.iter().map(|t| t.label.as_deref()).collect();
        assert_eq!(labels, [Some("common"), Some("mine"), None]);

        // Null never satisfies a required field.
        let error = fx.err(
            r#"{"defaults": {"prompt": "shared"}, "tasks": [{"agent": "scout", "prompt": null}]}"#,
        );
        assert!(error.contains("tasks[0].prompt"), "{error}");
    }

    #[test]
    fn agent_fields_are_overridden_by_defaults_then_by_tasks() {
        let fx = Fixture::new();
        let batch = fx.ok(
            r#"{"defaults": {"model": "m2", "tools": ["grep", "ls"]}, "tasks": [
                {"agent": "scout", "prompt": "p"},
                {"agent": "scout", "prompt": "p", "model": "m3", "tools": []},
                {"agent": "scout", "prompt": "p", "tools": null, "model": null,
                 "system_prompt": null, "thinking": null, "description": null},
                {"agent": "plain", "prompt": "p", "system_prompt": "extra",
                 "thinking": "high", "description": "d"}
            ]}"#,
        );
        let agents: Vec<_> = batch.tasks.iter().map(|t| &t.agent).collect();

        assert_eq!(agents[0].model.as_deref(), Some("m2"));
        assert_eq!(agents[0].tools, Some(vec!["grep".into(), "ls".into()]));
        assert_eq!(agents[0].system_prompt, "be brief");
        assert_eq!(agents[0].thinking.as_deref(), Some("low"));
        assert!(!agents[0].field_sources.contains_key("model"));
        assert!(agents[0].field_sources.contains_key("thinking"));

        assert_eq!(agents[1].model.as_deref(), Some("m3"));
        assert_eq!(agents[1].tools, Some(vec![]));

        assert_eq!(agents[2].model, None);
        assert_eq!(agents[2].tools, None);
        assert_eq!(agents[2].system_prompt, "");
        assert_eq!(agents[2].thinking, None);
        assert_eq!(agents[2].description, None);

        assert_eq!(agents[3].name, "plain");
        assert_eq!(agents[3].system_prompt, "extra");
        assert_eq!(agents[3].thinking.as_deref(), Some("high"));
        assert_eq!(agents[3].description.as_deref(), Some("d"));
    }

    #[test]
    fn agent_override_types_are_validated() {
        let fx = Fixture::new();
        for (json, expected) in [
            (
                r#"{"tasks": [{"agent": "scout", "prompt": "p", "model": 1}]}"#,
                "tasks[0].model",
            ),
            (
                r#"{"tasks": [{"agent": "scout", "prompt": "p", "tools": "read"}]}"#,
                "tasks[0].tools",
            ),
            (
                r#"{"tasks": [{"agent": "scout", "prompt": "p", "tools": ["a", 1]}]}"#,
                "tasks[0].tools[1]",
            ),
            (
                r#"{"defaults": {"thinking": true}, "tasks": [{"agent": "scout", "prompt": "p"}]}"#,
                "defaults.thinking",
            ),
        ] {
            let error = fx.err(json);
            assert!(error.contains(expected), "{json}: {error}");
        }
    }

    #[test]
    fn cwd_must_be_absolute_and_falls_back_to_the_invocation_directory() {
        let fx = Fixture::new();
        let batch = fx.ok(r#"{"defaults": {"cwd": "/srv/shared"}, "tasks": [
                {"agent": "scout", "prompt": "p"},
                {"agent": "scout", "prompt": "p", "cwd": "/srv/own"},
                {"agent": "scout", "prompt": "p", "cwd": null}
            ]}"#);
        let cwds: Vec<_> = batch.tasks.iter().map(|t| t.cwd.clone()).collect();
        assert_eq!(
            cwds,
            [
                PathBuf::from("/srv/shared"),
                PathBuf::from("/srv/own"),
                fx.path("invocation")
            ]
        );
        for json in [
            r#"{"tasks": [{"agent": "scout", "prompt": "p", "cwd": "rel/dir"}]}"#,
            r#"{"defaults": {"cwd": "."}, "tasks": [{"agent": "scout", "prompt": "p"}]}"#,
            r#"{"tasks": [{"agent": "scout", "prompt": "p", "cwd": 1}]}"#,
        ] {
            let error = fx.err(json);
            assert!(error.contains("cwd"), "{json}: {error}");
        }
    }

    #[test]
    fn env_merges_by_variable_and_null_removes() {
        let fx = Fixture::new();
        let batch = fx.ok(
            r#"{"defaults": {"env": {"A": "1", "B": "2", "C": null}}, "tasks": [
                {"agent": "scout", "prompt": "p", "env": {"B": "3", "C": "4", "D": null}},
                {"agent": "scout", "prompt": "p"},
                {"agent": "scout", "prompt": "p", "env": {}}
            ]}"#,
        );
        let some = |v: &str| Some(v.to_string());
        let expected_first: BTreeMap<String, Option<String>> = [
            ("A".to_string(), some("1")),
            ("B".to_string(), some("3")),
            ("C".to_string(), some("4")),
            ("D".to_string(), None),
        ]
        .into();
        let expected_default: BTreeMap<String, Option<String>> = [
            ("A".to_string(), some("1")),
            ("B".to_string(), some("2")),
            ("C".to_string(), None),
        ]
        .into();
        assert_eq!(batch.tasks[0].env, expected_first);
        assert_eq!(batch.tasks[1].env, expected_default);
        assert_eq!(batch.tasks[2].env, expected_default);
    }

    #[test]
    fn a_whole_env_null_drops_the_default_overlay() {
        let fx = Fixture::new();
        let batch = fx.ok(r#"{"defaults": {"env": {"A": "1", "B": null}}, "tasks": [
                {"agent": "scout", "prompt": "p", "env": null},
                {"agent": "scout", "prompt": "p", "env": {"C": "3"}}
            ]}"#);
        // Null returns the task to the unchanged inherited environment.
        assert!(batch.tasks[0].env.is_empty(), "{:?}", batch.tasks[0].env);
        // Per-variable merging is unaffected for the other tasks.
        let merged: BTreeMap<String, Option<String>> = [
            ("A".to_string(), Some("1".to_string())),
            ("B".to_string(), None),
            ("C".to_string(), Some("3".to_string())),
        ]
        .into();
        assert_eq!(batch.tasks[1].env, merged);

        // A null default is no overlay at all.
        let batch = fx.ok(r#"{"defaults": {"env": null}, "tasks": [
                {"agent": "scout", "prompt": "p"},
                {"agent": "scout", "prompt": "p", "env": {"A": null}}
            ]}"#);
        assert!(batch.tasks[0].env.is_empty(), "{:?}", batch.tasks[0].env);
        let removed: BTreeMap<String, Option<String>> = [("A".to_string(), None)].into();
        assert_eq!(batch.tasks[1].env, removed);
    }

    #[test]
    fn env_keys_and_values_are_validated() {
        let fx = Fixture::new();
        for (env, expected) in [
            (r#"{"": "v"}"#, "tasks[0].env"),
            (r#"{"A=B": "v"}"#, "'='"),
            (r#"{"A\u0000": "v"}"#, "NUL"),
            (r#"{"A": "v\u0000"}"#, "NUL"),
            (r#"{"A": 1}"#, "tasks[0].env.A"),
            (r#"["A"]"#, "tasks[0].env"),
        ] {
            let json =
                format!(r#"{{"tasks": [{{"agent": "scout", "prompt": "p", "env": {env}}}]}}"#);
            let error = fx.err(&json);
            assert!(error.contains(expected), "{json}: {error}");
        }
    }

    #[test]
    fn strings_that_reach_argv_reject_nul_bytes() {
        let fx = Fixture::new();
        for (json, expected) in [
            (r#""prompt": "p\u0000""#, "tasks[0].prompt"),
            (r#""prompt": "p", "model": "m\u0000""#, "tasks[0].model"),
            (
                r#""prompt": "p", "thinking": "\u0000""#,
                "tasks[0].thinking",
            ),
            (
                r#""prompt": "p", "tools": ["a\u0000"]"#,
                "tasks[0].tools[0]",
            ),
            (r#""prompt": "p", "label": "\u0000""#, "tasks[0].label"),
            (r#""prompt": "p", "cwd": "/a\u0000""#, "tasks[0].cwd"),
        ] {
            let json = format!(r#"{{"tasks": [{{"agent": "scout", {json}}}]}}"#);
            let error = fx.err(&json);
            assert!(error.contains(expected), "{json}: {error}");
            assert!(error.contains("NUL"), "{json}: {error}");
        }
    }

    #[test]
    fn file_references_resolve_against_the_specification_base_not_the_task_cwd() {
        let fx = Fixture::new();
        fx.write("spec/prompt.md", "# Look\n\n{\"file\": \"other.md\"}\n");
        fx.write("spec/agent.txt", "scout");
        fx.write("spec/label.txt", "batch label");
        fx.write("spec/tool.txt", "grep");
        fx.write("spec/value.txt", "from file");
        fx.write("cwd/prompt.md", "wrong base");
        let cwd = fx.path("cwd");
        let batch = fx.ok(&format!(
            r#"{{"label": {{"file": "label.txt"}},
                "defaults": {{"env": {{"V": {{"file": "value.txt"}}}}}},
                "tasks": [{{
                    "agent": {{"file": "agent.txt"}},
                    "prompt": {{"file": "prompt.md"}},
                    "cwd": "{}",
                    "label": {{"file": "label.txt"}},
                    "tools": ["read", {{"file": "tool.txt"}}],
                    "system_prompt": {{"file": "{}"}}
                }}]}}"#,
            cwd.display(),
            fx.path("spec/value.txt").display(),
        ));
        assert_eq!(batch.label.as_deref(), Some("batch label"));
        let task = &batch.tasks[0];
        assert_eq!(task.agent.name, "scout");
        // Verbatim, with no recursive expansion of reference-shaped text.
        assert_eq!(task.prompt, "# Look\n\n{\"file\": \"other.md\"}\n");
        assert_eq!(task.cwd, cwd);
        assert_eq!(task.label.as_deref(), Some("batch label"));
        assert_eq!(task.agent.tools, Some(vec!["read".into(), "grep".into()]));
        assert_eq!(task.agent.system_prompt, "from file");
        assert_eq!(task.env["V"].as_deref(), Some("from file"));
    }

    #[test]
    fn missing_file_references_are_admission_errors_but_overridden_ones_are_not_read() {
        let fx = Fixture::new();
        let error = fx.err(r#"{"tasks": [{"agent": "scout", "prompt": {"file": "nope.md"}}]}"#);
        assert!(error.contains("tasks[0].prompt"), "{error}");
        assert!(error.contains("nope.md"), "{error}");
        let batch = fx.ok(r#"{"defaults": {"prompt": {"file": "nope.md"}},
                "tasks": [{"agent": "scout", "prompt": "own"}]}"#);
        assert_eq!(batch.tasks[0].prompt, "own");
    }

    #[test]
    fn context_is_an_existing_canonical_address_and_never_touches_env() {
        let fx = Fixture::new();
        fx.context("acme/widgets/release");
        fx.context("other/repo/research");
        let batch = fx.ok(
            r#"{"defaults": {"context": "acme/widgets/release"}, "tasks": [
                {"agent": "scout", "prompt": "p"},
                {"agent": "scout", "prompt": "p", "context": "other/repo/research"},
                {"agent": "scout", "prompt": "p", "context": null}
            ]}"#,
        );
        let contexts: Vec<_> = batch.tasks.iter().map(|t| t.context.as_deref()).collect();
        assert_eq!(
            contexts,
            [
                Some("acme/widgets/release"),
                Some("other/repo/research"),
                None
            ]
        );
        assert!(batch.tasks.iter().all(|task| task.env.is_empty()));
    }

    #[test]
    fn invalid_or_missing_contexts_are_rejected() {
        let fx = Fixture::new();
        fx.context("acme/widgets/release");
        for context in [
            "release",
            "acme/widgets",
            "acme/widgets/release/spec/index.md",
            "/acme/widgets/release",
            "~/cue/acme/widgets/release",
            "acme/../widgets/release",
            "acme//release",
            "acme/widgets/missing",
            "",
        ] {
            let json = format!(
                r#"{{"tasks": [{{"agent": "scout", "prompt": "p"}},
                    {{"agent": "scout", "prompt": "p", "context": "{context}"}}]}}"#
            );
            let error = fx.err(&json);
            assert!(error.contains("tasks[1].context"), "{context}: {error}");
        }
    }

    #[test]
    fn only_effective_contexts_are_validated() {
        let fx = Fixture::new();
        let batch = fx.ok(r#"{"defaults": {"context": "no/such/context"},
                "tasks": [{"agent": "scout", "prompt": "p", "context": null}]}"#);
        assert_eq!(batch.tasks[0].context, None);
    }

    #[test]
    fn worktree_is_replaced_whole_and_its_path_is_relative_to_the_target() {
        let fx = Fixture::new();
        fx.write("spec/base.txt", "origin/main");
        let batch = fx.ok(
            r#"{"defaults": {"worktree": {"base": {"file": "base.txt"}, "ephemeral": true,
                                          "path": "wt/shared", "branch": "shared"}},
                "tasks": [
                {"agent": "scout", "prompt": "p"},
                {"agent": "scout", "prompt": "p", "cwd": "/repo",
                 "worktree": {"base": "dev", "ephemeral": false, "path": "wt/own"}},
                {"agent": "scout", "prompt": "p",
                 "worktree": {"base": "dev", "ephemeral": true, "path": "/abs/wt",
                              "branch": null}},
                {"agent": "scout", "prompt": "p", "worktree": null}
            ]}"#,
        );
        let worktrees: Vec<_> = batch.tasks.iter().map(|t| t.worktree.clone()).collect();
        assert_eq!(
            worktrees[0],
            Some(Worktree {
                base: "origin/main".into(),
                ephemeral: true,
                path: Some(fx.path("invocation").join("wt/shared")),
                branch: Some("shared".into()),
            })
        );
        assert_eq!(
            worktrees[1],
            Some(Worktree {
                base: "dev".into(),
                ephemeral: false,
                path: Some(PathBuf::from("/repo/wt/own")),
                branch: None,
            })
        );
        assert_eq!(
            worktrees[2],
            Some(Worktree {
                base: "dev".into(),
                ephemeral: true,
                path: Some(PathBuf::from("/abs/wt")),
                branch: None,
            })
        );
        assert_eq!(worktrees[3], None);
    }

    #[test]
    fn worktree_shape_is_validated() {
        let fx = Fixture::new();
        for (worktree, expected) in [
            (r#""main""#, "tasks[0].worktree"),
            (r#"{"ephemeral": true}"#, "base"),
            (r#"{"base": null, "ephemeral": true}"#, "base"),
            (r#"{"base": "", "ephemeral": true}"#, "base"),
            (r#"{"base": "main"}"#, "ephemeral"),
            (r#"{"base": "main", "ephemeral": "yes"}"#, "ephemeral"),
            (r#"{"base": "main", "ephemeral": null}"#, "ephemeral"),
            (r#"{"base": "main", "ephemeral": true, "path": 1}"#, "path"),
            (
                r#"{"base": "main", "ephemeral": true, "branch": ""}"#,
                "branch",
            ),
            (r#"{"base": "m\u0000", "ephemeral": true}"#, "NUL"),
            (
                r#"{"base": "main", "ephemeral": true, "adopt": true}"#,
                "adopt",
            ),
        ] {
            let json = format!(
                r#"{{"tasks": [{{"agent": "scout", "prompt": "p", "worktree": {worktree}}}]}}"#
            );
            let error = fx.err(&json);
            assert!(error.contains(expected), "{worktree}: {error}");
            assert!(error.contains("tasks[0].worktree"), "{worktree}: {error}");
        }
    }

    #[test]
    fn inherited_agent_argv_strings_with_nul_reject_the_whole_request() {
        let fx = Fixture::with_manifest(
            r#"{"agents": {
                "ok": {},
                "bad-model": {"model": "m\u0000"},
                "bad-thinking": {"thinking": "t\u0000"},
                "bad-tools": {"tools": ["read", "g\u0000"]}
            }}"#,
        );
        for (agent, expected) in [
            ("bad-model", "tasks[1].model"),
            ("bad-thinking", "tasks[1].thinking"),
            ("bad-tools", "tasks[1].tools[1]"),
        ] {
            let json = format!(
                r#"{{"tasks": [{{"agent": "ok", "prompt": "p"}},
                    {{"agent": "{agent}", "prompt": "p"}}]}}"#
            );
            let error = fx.err(&json);
            assert!(error.contains(expected), "{agent}: {error}");
            assert!(error.contains("NUL"), "{agent}: {error}");
        }
        // Only effective values matter: an override replaces the bad value.
        let batch = fx.ok(r#"{"tasks": [
                {"agent": "bad-model", "prompt": "p", "model": "m"},
                {"agent": "bad-thinking", "prompt": "p", "thinking": null},
                {"agent": "bad-tools", "prompt": "p", "tools": []}
            ]}"#);
        assert_eq!(batch.tasks.len(), 3);
    }

    #[test]
    fn a_worktree_without_a_path_needs_a_configured_root() {
        let fx = Fixture::new();
        let error = fx.err(
            r#"{"tasks": [{"agent": "scout", "prompt": "p",
                           "worktree": {"base": "main", "ephemeral": true}}]}"#,
        );
        assert!(error.contains("tasks[0].worktree"), "{error}");
        assert!(error.contains("worktree_root"), "{error}");

        let fx = Fixture::with_manifest(r#"{"worktree_root": "trees", "agents": {"scout": {}}}"#);
        let batch = fx.ok(r#"{"tasks": [{"agent": "scout", "prompt": "p",
                           "worktree": {"base": "main", "ephemeral": true}}]}"#);
        let worktree = batch.tasks[0].worktree.as_ref().unwrap();
        assert_eq!(worktree.path, None);
    }

    #[test]
    fn a_prompt_with_surrounding_whitespace_is_preserved() {
        let fx = Fixture::new();
        let batch = fx.ok(r#"{"tasks": [{"agent": "scout", "prompt": "  look\n"}]}"#);
        assert_eq!(batch.tasks[0].prompt, "  look\n");
    }
}
