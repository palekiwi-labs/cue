//! Execution spec model: parsing, validation, normalization, and
//! `{file}` interpolation.
//!
//! The wire format is a JSON array of run specs (see the cue-agent
//! specification, section 3). Parsing yields normalized [`RunSpec`]
//! values with all defaults applied and file references resolved.

use serde::Deserialize;
use serde::de::{self, MapAccess, SeqAccess, Visitor};
use std::collections::BTreeMap;
use std::fmt;
use std::path::{Path, PathBuf};

// ---------------------------------------------------------------------------
// Resolvable string values (string | {"file": PATH})
// ---------------------------------------------------------------------------

/// A literal string or a reference to a file whose content is loaded
/// during interpolation.
#[derive(Debug, Clone)]
pub enum StrOrFile {
    Str(String),
    File(PathBuf),
}

impl<'de> Deserialize<'de> for StrOrFile {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        struct V;
        impl<'de> Visitor<'de> for V {
            type Value = StrOrFile;
            fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str(r#"a string or a {"file": PATH} object"#)
            }
            fn visit_str<E: de::Error>(self, v: &str) -> Result<StrOrFile, E> {
                Ok(StrOrFile::Str(v.to_owned()))
            }
            fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<StrOrFile, A::Error> {
                read_file_ref(&mut map).map(StrOrFile::File)
            }
        }
        deserializer.deserialize_any(V)
    }
}

/// `append-system-prompt` accepts a string, an array of strings, or a
/// single `{"file": PATH}` object.
#[derive(Debug, Clone)]
pub enum StrListOrFile {
    Str(String),
    List(Vec<String>),
    File(PathBuf),
}

impl<'de> Deserialize<'de> for StrListOrFile {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        struct V;
        impl<'de> Visitor<'de> for V {
            type Value = StrListOrFile;
            fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str(r#"a string, an array of strings, or a {"file": PATH} object"#)
            }
            fn visit_str<E: de::Error>(self, v: &str) -> Result<StrListOrFile, E> {
                Ok(StrListOrFile::Str(v.to_owned()))
            }
            fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<StrListOrFile, A::Error> {
                let mut out = Vec::new();
                while let Some(s) = seq.next_element::<String>()? {
                    out.push(s);
                }
                Ok(StrListOrFile::List(out))
            }
            fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<StrListOrFile, A::Error> {
                read_file_ref(&mut map).map(StrListOrFile::File)
            }
        }
        deserializer.deserialize_any(V)
    }
}

/// Read exactly one `{"file": PATH}` entry from a map, rejecting
/// empty maps, wrong keys, and extra fields with actionable messages.
fn read_file_ref<'de, A: MapAccess<'de>>(map: &mut A) -> Result<PathBuf, A::Error> {
    let expect =
        || -> String { r#"expected an object with a single "file": PATH field"#.to_string() };
    let (key, value) = match map.next_entry::<String, String>()? {
        None => return Err(de::Error::custom(format!("empty object; {}", expect()))),
        Some(kv) => kv,
    };
    if key != "file" {
        return Err(de::Error::custom(format!(
            "unknown field '{key}'; {}",
            expect()
        )));
    }
    if map.next_key::<String>()?.is_some() {
        return Err(de::Error::custom(format!(
            "unexpected extra field; {}",
            expect()
        )));
    }
    Ok(PathBuf::from(value))
}

// ---------------------------------------------------------------------------
// Wire format (raw serde structs)
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "kebab-case")]
struct RawRunSpec {
    id: Option<String>,
    harness: Option<String>,
    model: Option<String>,
    provider: Option<String>,
    system_prompt: Option<StrOrFile>,
    append_system_prompt: Option<StrListOrFile>,
    prompt: Option<StrOrFile>,
    tools: Option<Vec<String>>,
    exclude_tools: Option<Vec<String>>,
    no_tools: Option<bool>,
    no_builtin_tools: Option<bool>,
    thinking: Option<Thinking>,
    approve: Option<bool>,
    session: Option<RawSession>,
    background: Option<bool>,
    env: Option<BTreeMap<String, String>>,
    worktree: Option<RawWorktree>,
    timeout: Option<u64>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawSession {
    persist: Option<bool>,
    id: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawWorktree {
    mode: Option<WorktreeMode>,
    base: Option<String>,
    name: Option<String>,
}

/// pi thinking-level vocabulary (pi 0.84.2).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Thinking {
    Off,
    Minimal,
    Low,
    Medium,
    High,
    Xhigh,
    Max,
}

/// Worktree placement mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum WorktreeMode {
    Cwd,
    Ephemeral,
    Named,
}

// ---------------------------------------------------------------------------
// Normalized model
// ---------------------------------------------------------------------------

/// A validated, normalized run spec with defaults applied and file
/// references resolved against the spec's base directory.
#[derive(Debug, Clone)]
pub struct RunSpec {
    /// Run id; `None` means a default (`run-0`, `run-1`, ...) is
    /// minted at execution time.
    pub id: Option<String>,
    /// Always `"pi"` in the MVP; kept for future harnesses.
    pub harness: String,
    pub model: Option<String>,
    pub provider: Option<String>,
    pub system_prompt: Option<String>,
    pub append_system_prompt: Vec<String>,
    pub prompt: String,
    pub tools: Option<Vec<String>>,
    pub exclude_tools: Option<Vec<String>>,
    pub no_tools: bool,
    pub no_builtin_tools: bool,
    pub thinking: Option<Thinking>,
    pub approve: bool,
    pub session: Session,
    pub background: bool,
    pub env: BTreeMap<String, String>,
    pub worktree: Worktree,
    /// Per-run wall-clock cap in seconds.
    pub timeout: Option<u64>,
}

/// Session persistence settings, normalized.
#[derive(Debug, Clone)]
pub struct Session {
    pub persist: bool,
    pub id: Option<String>,
}

/// Worktree settings, normalized with defaults applied.
#[derive(Debug, Clone)]
pub struct Worktree {
    pub mode: WorktreeMode,
    pub base: String,
    pub name: Option<String>,
}

// ---------------------------------------------------------------------------
// Parsing, validation, interpolation
// ---------------------------------------------------------------------------

/// Parse and validate a spec array, interpolating `{file}` references
/// relative to `base_dir` (the spec file's directory, or the cwd for
/// stdin/argv input).
pub fn load_spec(json: &str, base_dir: &Path) -> Result<Vec<RunSpec>, String> {
    let raw: Vec<RawRunSpec> =
        serde_json::from_str(json).map_err(|e| format!("invalid spec: {e}"))?;
    if raw.is_empty() {
        return Err("spec array is empty; expected at least one run".to_string());
    }

    let mut seen_ids: BTreeMap<String, usize> = BTreeMap::new();
    let mut out = Vec::with_capacity(raw.len());
    for (idx, spec) in raw.into_iter().enumerate() {
        let normalized = validate_and_normalize(idx, spec, &mut seen_ids)?;
        out.push(interpolate(normalized, base_dir, idx)?);
    }
    Ok(out)
}

/// Validate one raw spec element and apply defaults. On success,
/// returns the normalized spec with file references still unresolved.
fn validate_and_normalize(
    idx: usize,
    raw: RawRunSpec,
    seen_ids: &mut BTreeMap<String, usize>,
) -> Result<UnresolvedRunSpec, String> {
    let at = |msg: String| format!("spec[{idx}]: {msg}");

    if let Some(h) = &raw.harness
        && h != "pi"
    {
        return Err(at(format!(
            "unsupported harness '{h}' (only 'pi' is supported)"
        )));
    }
    if raw.background == Some(true) {
        return Err(at(
            "background execution is not supported yet (reserved for a future release)".to_string(),
        ));
    }
    if let Some(id) = &raw.id {
        if id.is_empty() {
            return Err(at("id must not be empty".to_string()));
        }
        if let Some(first) = seen_ids.get(id) {
            return Err(at(format!(
                "duplicate run id '{id}' (first seen at spec[{first}])"
            )));
        }
        seen_ids.insert(id.clone(), idx);
    }
    let prompt = raw
        .prompt
        .ok_or_else(|| at("prompt is required".to_string()))?;

    let session = normalize_session(idx, raw.session)?;
    let worktree = normalize_worktree(idx, raw.worktree)?;

    if raw.timeout == Some(0) {
        return Err(at("timeout must be greater than zero".to_string()));
    }
    if let Some(env) = &raw.env {
        let bad = env
            .keys()
            .find(|k| k.is_empty() || k.contains('=') || k.contains('\0'));
        if let Some(k) = bad {
            return Err(at(format!("invalid env key '{k}'")));
        }
    }

    Ok(UnresolvedRunSpec {
        id: raw.id,
        harness: "pi".to_string(),
        model: raw.model,
        provider: raw.provider,
        system_prompt: raw.system_prompt,
        append_system_prompt: raw.append_system_prompt,
        prompt,
        tools: raw.tools,
        exclude_tools: raw.exclude_tools,
        no_tools: raw.no_tools.unwrap_or(false),
        no_builtin_tools: raw.no_builtin_tools.unwrap_or(false),
        thinking: raw.thinking,
        approve: raw.approve.unwrap_or(false),
        session,
        background: false,
        env: raw.env.unwrap_or_default(),
        worktree,
        timeout: raw.timeout,
    })
}

/// Intermediate form: normalized but with file refs unresolved.
#[derive(Debug)]
struct UnresolvedRunSpec {
    id: Option<String>,
    harness: String,
    model: Option<String>,
    provider: Option<String>,
    system_prompt: Option<StrOrFile>,
    append_system_prompt: Option<StrListOrFile>,
    prompt: StrOrFile,
    tools: Option<Vec<String>>,
    exclude_tools: Option<Vec<String>>,
    no_tools: bool,
    no_builtin_tools: bool,
    thinking: Option<Thinking>,
    approve: bool,
    session: Session,
    background: bool,
    env: BTreeMap<String, String>,
    worktree: Worktree,
    timeout: Option<u64>,
}

fn normalize_session(idx: usize, raw: Option<RawSession>) -> Result<Session, String> {
    let at = |msg: String| format!("spec[{idx}]: {msg}");
    let Some(raw) = raw else {
        return Ok(Session {
            persist: false,
            id: None,
        });
    };
    let persist = raw.persist.unwrap_or(false);
    match (persist, raw.id) {
        (false, Some(_)) => Err(at(
            "session.id requires session.persist: true (a non-persisted session is never saved)"
                .to_string(),
        )),
        (false, None) => Ok(Session { persist, id: None }),
        (true, Some(id)) if id.is_empty() => Err(at("session.id must not be empty".to_string())),
        (true, id) => Ok(Session { persist, id }),
    }
}

fn normalize_worktree(idx: usize, raw: Option<RawWorktree>) -> Result<Worktree, String> {
    let at = |msg: String| format!("spec[{idx}]: {msg}");
    let Some(raw) = raw else {
        return Ok(Worktree {
            mode: WorktreeMode::Cwd,
            base: "HEAD".to_string(),
            name: None,
        });
    };
    let mode = raw.mode.unwrap_or(WorktreeMode::Cwd);
    match mode {
        WorktreeMode::Cwd => {
            if raw.base.is_some() {
                return Err(at(
                    "worktree 'base' requires mode 'ephemeral' or 'named'".to_string()
                ));
            }
            if raw.name.is_some() {
                return Err(at("worktree 'name' requires mode 'named'".to_string()));
            }
            Ok(Worktree {
                mode,
                base: "HEAD".to_string(),
                name: None,
            })
        }
        WorktreeMode::Ephemeral => {
            if let Some(name) = raw.name {
                if name.is_empty() {
                    return Err(at("worktree 'name' must not be empty".to_string()));
                }
                return Err(at("worktree 'name' requires mode 'named'".to_string()));
            }
            let base = raw.base.unwrap_or_else(|| "HEAD".to_string());
            if base.is_empty() {
                return Err(at("worktree 'base' must not be empty".to_string()));
            }
            Ok(Worktree {
                mode,
                base,
                name: None,
            })
        }
        WorktreeMode::Named => {
            let name = raw
                .name
                .ok_or_else(|| at("worktree mode 'named' requires a 'name'".to_string()))?;
            if name.is_empty() {
                return Err(at("worktree 'name' must not be empty".to_string()));
            }
            let base = raw.base.unwrap_or_else(|| "HEAD".to_string());
            if base.is_empty() {
                return Err(at("worktree 'base' must not be empty".to_string()));
            }
            Ok(Worktree {
                mode,
                base,
                name: Some(name),
            })
        }
    }
}

/// Resolve `{file}` references relative to `base_dir`.
fn interpolate(spec: UnresolvedRunSpec, base_dir: &Path, idx: usize) -> Result<RunSpec, String> {
    let at = |msg: String| format!("spec[{idx}]: {msg}");

    let prompt =
        resolve_str_or_file(&spec.prompt, base_dir).map_err(|e| at(format!("prompt: {e}")))?;
    let system_prompt = spec
        .system_prompt
        .as_ref()
        .map(|v| resolve_str_or_file(v, base_dir))
        .transpose()
        .map_err(|e| at(format!("system-prompt: {e}")))?;
    let append_system_prompt = match &spec.append_system_prompt {
        None => Vec::new(),
        Some(StrListOrFile::Str(s)) => vec![s.clone()],
        Some(StrListOrFile::List(items)) => items.clone(),
        Some(StrListOrFile::File(path)) => vec![
            read_spec_file(path, base_dir).map_err(|e| at(format!("append-system-prompt: {e}")))?,
        ],
    };

    Ok(RunSpec {
        id: spec.id,
        harness: spec.harness,
        model: spec.model,
        provider: spec.provider,
        system_prompt,
        append_system_prompt,
        prompt,
        tools: spec.tools,
        exclude_tools: spec.exclude_tools,
        no_tools: spec.no_tools,
        no_builtin_tools: spec.no_builtin_tools,
        thinking: spec.thinking,
        approve: spec.approve,
        session: spec.session,
        background: spec.background,
        env: spec.env,
        worktree: spec.worktree,
        timeout: spec.timeout,
    })
}

fn resolve_str_or_file(v: &StrOrFile, base_dir: &Path) -> Result<String, String> {
    match v {
        StrOrFile::Str(s) => Ok(s.clone()),
        StrOrFile::File(path) => read_spec_file(path, base_dir),
    }
}

fn read_spec_file(path: &Path, base_dir: &Path) -> Result<String, String> {
    let resolved = if path.is_absolute() {
        path.to_path_buf()
    } else {
        base_dir.join(path)
    };
    std::fs::read_to_string(&resolved)
        .map_err(|e| format!("failed to read file {}: {e}", resolved.display()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn load(json: &str) -> Result<Vec<RunSpec>, String> {
        load_spec(json, Path::new("/base"))
    }

    #[test]
    fn minimal_spec_applies_defaults() {
        let specs = load(r#"[{"prompt": "hello"}]"#).expect("parses");
        assert_eq!(specs.len(), 1);
        let s = &specs[0];
        assert_eq!(s.id, None);
        assert_eq!(s.harness, "pi");
        assert_eq!(s.model, None);
        assert_eq!(s.provider, None);
        assert_eq!(s.system_prompt, None);
        assert!(s.append_system_prompt.is_empty());
        assert_eq!(s.prompt, "hello");
        assert_eq!(s.tools, None);
        assert_eq!(s.exclude_tools, None);
        assert!(!s.no_tools);
        assert!(!s.no_builtin_tools);
        assert_eq!(s.thinking, None);
        assert!(!s.approve);
        assert!(!s.session.persist);
        assert_eq!(s.session.id, None);
        assert!(!s.background);
        assert!(s.env.is_empty());
        assert_eq!(s.worktree.mode, WorktreeMode::Cwd);
        assert_eq!(s.worktree.base, "HEAD");
        assert_eq!(s.worktree.name, None);
        assert_eq!(s.timeout, None);
    }

    #[test]
    fn not_an_array_is_rejected() {
        let err = load(r#"{"prompt": "hello"}"#).unwrap_err();
        assert!(err.contains("invalid spec"), "got: {err}");
    }

    #[test]
    fn empty_array_is_rejected() {
        let err = load("[]").unwrap_err();
        assert!(err.contains("empty"), "got: {err}");
    }

    #[test]
    fn unknown_field_is_rejected() {
        let err = load(r#"[{"prompt": "x", "unknown-field": 1}]"#).unwrap_err();
        assert!(err.contains("unknown field"), "got: {err}");
    }

    #[test]
    fn prompt_is_required() {
        let err = load(r#"[{"model": "m"}]"#).unwrap_err();
        assert!(err.contains("spec[0]"), "got: {err}");
        assert!(err.contains("prompt is required"), "got: {err}");
    }

    #[test]
    fn thinking_vocabulary_parses() {
        for level in ["off", "minimal", "low", "medium", "high", "xhigh", "max"] {
            let json = format!(r#"[{{"prompt": "x", "thinking": "{level}"}}]"#);
            let specs = load(&json).unwrap_or_else(|e| panic!("{level}: {e}"));
            assert!(specs[0].thinking.is_some());
        }
    }

    #[test]
    fn unknown_thinking_level_is_rejected() {
        let err = load(r#"[{"prompt": "x", "thinking": "fast"}]"#).unwrap_err();
        assert!(err.contains("unknown variant"), "got: {err}");
    }

    #[test]
    fn harness_other_than_pi_is_rejected() {
        let err = load(r#"[{"prompt": "x", "harness": "claude"}]"#).unwrap_err();
        assert!(err.contains("unsupported harness 'claude'"), "got: {err}");
    }

    #[test]
    fn harness_pi_is_accepted() {
        assert!(load(r#"[{"prompt": "x", "harness": "pi"}]"#).is_ok());
    }

    #[test]
    fn background_true_is_rejected() {
        let err = load(r#"[{"prompt": "x", "background": true}]"#).unwrap_err();
        assert!(
            err.contains("background execution is not supported"),
            "got: {err}"
        );
    }

    #[test]
    fn duplicate_ids_are_rejected() {
        let err = load(
            r#"[{"id": "a", "prompt": "x"}, {"id": "b", "prompt": "y"}, {"id": "a", "prompt": "z"}]"#,
        )
        .unwrap_err();
        assert!(err.contains("spec[2]"), "got: {err}");
        assert!(err.contains("duplicate run id 'a'"), "got: {err}");
        assert!(err.contains("spec[0]"), "got: {err}");
    }

    #[test]
    fn empty_id_is_rejected() {
        let err = load(r#"[{"id": "", "prompt": "x"}]"#).unwrap_err();
        assert!(err.contains("id must not be empty"), "got: {err}");
    }

    #[test]
    fn timeout_zero_is_rejected() {
        let err = load(r#"[{"prompt": "x", "timeout": 0}]"#).unwrap_err();
        assert!(
            err.contains("timeout must be greater than zero"),
            "got: {err}"
        );
    }

    #[test]
    fn timeout_negative_is_rejected_at_parse() {
        let err = load(r#"[{"prompt": "x", "timeout": -5}]"#).unwrap_err();
        assert!(err.contains("invalid spec"), "got: {err}");
    }

    #[test]
    fn env_keys_are_validated() {
        let err = load(r#"[{"prompt": "x", "env": {"": "v"}}]"#).unwrap_err();
        assert!(err.contains("invalid env key"), "got: {err}");
        let err = load(r#"[{"prompt": "x", "env": {"BAD=KEY": "v"}}]"#).unwrap_err();
        assert!(err.contains("invalid env key 'BAD=KEY'"), "got: {err}");
    }

    #[test]
    fn session_id_without_persist_is_rejected() {
        let err = load(r#"[{"prompt": "x", "session": {"id": "s1"}}]"#).unwrap_err();
        assert!(
            err.contains("session.id requires session.persist"),
            "got: {err}"
        );
    }

    #[test]
    fn session_empty_id_is_rejected() {
        let err = load(r#"[{"prompt": "x", "session": {"persist": true, "id": ""}}]"#).unwrap_err();
        assert!(err.contains("session.id must not be empty"), "got: {err}");
    }

    #[test]
    fn session_persist_defaults_to_false() {
        let specs = load(r#"[{"prompt": "x", "session": {}}]"#).expect("parses");
        assert!(!specs[0].session.persist);
        assert_eq!(specs[0].session.id, None);
    }

    #[test]
    fn session_persist_with_id_is_accepted() {
        let specs =
            load(r#"[{"prompt": "x", "session": {"persist": true, "id": "s1"}}]"#).expect("parses");
        assert!(specs[0].session.persist);
        assert_eq!(specs[0].session.id.as_deref(), Some("s1"));
    }

    #[test]
    fn named_worktree_requires_name() {
        let err = load(r#"[{"prompt": "x", "worktree": {"mode": "named"}}]"#).unwrap_err();
        assert!(err.contains("requires a 'name'"), "got: {err}");
    }

    #[test]
    fn cwd_worktree_rejects_base_and_name() {
        let err =
            load(r#"[{"prompt": "x", "worktree": {"mode": "cwd", "base": "main"}}]"#).unwrap_err();
        assert!(
            err.contains("'base' requires mode 'ephemeral' or 'named'"),
            "got: {err}"
        );
        let err =
            load(r#"[{"prompt": "x", "worktree": {"mode": "cwd", "name": "w"}}]"#).unwrap_err();
        assert!(err.contains("'name' requires mode 'named'"), "got: {err}");
    }

    #[test]
    fn ephemeral_worktree_rejects_name_and_defaults_base() {
        let err = load(r#"[{"prompt": "x", "worktree": {"mode": "ephemeral", "name": "w"}}]"#)
            .unwrap_err();
        assert!(err.contains("'name' requires mode 'named'"), "got: {err}");

        let specs =
            load(r#"[{"prompt": "x", "worktree": {"mode": "ephemeral"}}]"#).expect("parses");
        assert_eq!(specs[0].worktree.mode, WorktreeMode::Ephemeral);
        assert_eq!(specs[0].worktree.base, "HEAD");
        assert_eq!(specs[0].worktree.name, None);
    }

    #[test]
    fn named_worktree_with_name_and_base_is_accepted() {
        let specs = load(
            r#"[{"prompt": "x", "worktree": {"mode": "named", "name": "wt", "base": "main"}}]"#,
        )
        .expect("parses");
        assert_eq!(specs[0].worktree.mode, WorktreeMode::Named);
        assert_eq!(specs[0].worktree.base, "main");
        assert_eq!(specs[0].worktree.name.as_deref(), Some("wt"));
    }

    #[test]
    fn file_refs_resolve_relative_to_base_dir() {
        let dir = tempfile::tempdir().expect("tempdir");
        std::fs::write(dir.path().join("prompt.txt"), "file prompt").expect("write");
        std::fs::write(dir.path().join("system.md"), "file system").expect("write");
        std::fs::write(dir.path().join("append.md"), "file append").expect("write");

        let json = r#"[{
            "prompt": {"file": "prompt.txt"},
            "system-prompt": {"file": "system.md"},
            "append-system-prompt": {"file": "append.md"}
        }]"#;
        let specs = load_spec(json, dir.path()).expect("parses");
        assert_eq!(specs[0].prompt, "file prompt");
        assert_eq!(specs[0].system_prompt.as_deref(), Some("file system"));
        assert_eq!(specs[0].append_system_prompt, vec!["file append"]);
    }

    #[test]
    fn append_system_prompt_forms_normalize() {
        let specs = load(r#"[{"prompt": "x", "append-system-prompt": "one"}]"#).expect("parses");
        assert_eq!(specs[0].append_system_prompt, vec!["one"]);

        let specs =
            load(r#"[{"prompt": "x", "append-system-prompt": ["one", "two"]}]"#).expect("parses");
        assert_eq!(specs[0].append_system_prompt, vec!["one", "two"]);
    }

    #[test]
    fn absolute_file_ref_is_not_rebased() {
        let dir = tempfile::tempdir().expect("tempdir");
        let abs = dir.path().join("prompt.txt");
        std::fs::write(&abs, "abs prompt").expect("write");

        let json = format!(
            r#"[{{"prompt": {{"file": {}}}}}]"#,
            serde_json::to_string(&abs).unwrap()
        );
        // Base dir is deliberately bogus; absolute path must still resolve.
        let specs = load_spec(&json, Path::new("/nonexistent")).expect("parses");
        assert_eq!(specs[0].prompt, "abs prompt");
    }

    #[test]
    fn missing_file_ref_names_the_resolved_path() {
        let err =
            load_spec(r#"[{"prompt": {"file": "nope.txt"}}]"#, Path::new("/base")).unwrap_err();
        assert!(err.contains("spec[0]"), "got: {err}");
        assert!(err.contains("/base/nope.txt"), "got: {err}");
    }

    #[test]
    fn file_ref_with_extra_fields_is_rejected() {
        let err = load(r#"[{"prompt": {"file": "x.txt", "extra": 1}}]"#).unwrap_err();
        assert!(err.contains("invalid spec"), "got: {err}");
        assert!(err.contains("extra"), "got: {err}");
    }

    #[test]
    fn unknown_field_inside_file_ref_is_rejected() {
        let err = load(r#"[{"prompt": {"path": "x.txt"}}]"#).unwrap_err();
        assert!(err.contains("unknown field 'path'"), "got: {err}");
    }

    #[test]
    fn kitchen_sink_spec_parses() {
        let json = r#"[{
            "id": "reviewer-a",
            "harness": "pi",
            "model": "google/gemini-3.6-flash",
            "provider": "google",
            "system-prompt": "You are an auditor.",
            "append-system-prompt": ["Focus on hot paths."],
            "prompt": "Review the diff.",
            "tools": ["read", "grep"],
            "exclude-tools": ["bash"],
            "no-tools": false,
            "no-builtin-tools": false,
            "thinking": "medium",
            "approve": true,
            "session": {"persist": true, "id": "4f9a2c1e-0000-7000-8000-000000000000"},
            "env": {"GEMINI_API_KEY": "secret"},
            "worktree": {"mode": "ephemeral", "base": "main"},
            "timeout": 300
        }]"#;
        let specs = load(json).expect("parses");
        let s = &specs[0];
        assert_eq!(s.id.as_deref(), Some("reviewer-a"));
        assert_eq!(s.model.as_deref(), Some("google/gemini-3.6-flash"));
        assert_eq!(s.provider.as_deref(), Some("google"));
        assert_eq!(s.system_prompt.as_deref(), Some("You are an auditor."));
        assert_eq!(s.append_system_prompt, vec!["Focus on hot paths."]);
        assert_eq!(s.prompt, "Review the diff.");
        assert_eq!(
            s.tools.as_deref(),
            Some(&["read".to_string(), "grep".to_string()][..])
        );
        assert_eq!(s.exclude_tools.as_deref(), Some(&["bash".to_string()][..]));
        assert_eq!(s.thinking, Some(Thinking::Medium));
        assert!(s.approve);
        assert!(s.session.persist);
        assert_eq!(
            s.env.get("GEMINI_API_KEY").map(String::as_str),
            Some("secret")
        );
        assert_eq!(s.worktree.mode, WorktreeMode::Ephemeral);
        assert_eq!(s.worktree.base, "main");
        assert_eq!(s.timeout, Some(300));
    }
}
