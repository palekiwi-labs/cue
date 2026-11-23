//! Validated, atomic edits of the frontmatter of an artifact or a context.
//!
//! An update names its target by canonical address, checks every requested
//! change against the field schema of the target type, and writes all of
//! them in one atomic replacement of the file. Only frontmatter entries that
//! change are touched; every other byte, the body included, is preserved.

use anyhow::{Context as _, Result, bail};
use serde::Serialize;
use serde_yaml::{Mapping, Value};
use std::collections::HashSet;
use std::fmt;
use std::fs::{File, OpenOptions};
use std::path::{Path, PathBuf};

use crate::artifact::FRONTMATTER_MAX_LINES;
use crate::frontmatter;

/// Values accepted for `status`.
pub const STATUSES: &[&str] = &["open", "in-progress", "complete", "closed"];
/// Values accepted for `priority`.
pub const PRIORITIES: &[&str] = &["critical", "high", "normal", "low"];
/// Values accepted for a context's `kind`.
pub const KINDS: &[&str] = &["work", "coord", "reference"];
/// Values accepted for a context's `mode`.
pub const MODES: &[&str] = &["research", "design", "build", "review", "learn"];

const ADDRESS_FORM: &str = "(<org>/<repo>/<context> or <org>/<repo>/<context>/<type>/<name>)";

/// A frontmatter field an update can change.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Field {
    Title,
    Status,
    Priority,
    Description,
    Kind,
    Mode,
    Parent,
    Refs,
}

impl Field {
    /// The frontmatter key of the field.
    pub fn key(self) -> &'static str {
        match self {
            Self::Title => "title",
            Self::Status => "status",
            Self::Priority => "priority",
            Self::Description => "description",
            Self::Kind => "kind",
            Self::Mode => "mode",
            Self::Parent => "parent",
            Self::Refs => "refs",
        }
    }

    /// The closed set of values the field accepts, if it has one.
    pub fn values(self) -> Option<&'static [&'static str]> {
        match self {
            Self::Status => Some(STATUSES),
            Self::Priority => Some(PRIORITIES),
            Self::Kind => Some(KINDS),
            Self::Mode => Some(MODES),
            _ => None,
        }
    }
}

/// The kinds of target an update can edit.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Target {
    Context,
    Task,
    Plan,
    Note,
}

impl Target {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Context => "context",
            Self::Task => "task",
            Self::Plan => "plan",
            Self::Note => "note",
        }
    }

    /// The fields an update may change on this target.
    pub fn fields(self) -> &'static [Field] {
        use Field::*;
        match self {
            Self::Context => &[Title, Description, Kind, Mode, Parent, Refs],
            Self::Task => &[Title, Status, Priority, Parent, Refs],
            Self::Plan | Self::Note => &[Title, Status, Parent, Refs],
        }
    }

    pub fn allows(self, field: Field) -> bool {
        self.fields().contains(&field)
    }

    /// The target kinds a parent of this target may be.
    pub fn parents(self) -> &'static [Target] {
        match self {
            Self::Context => &[Target::Context],
            Self::Task | Self::Plan | Self::Note => &[Target::Task, Target::Context],
        }
    }
}

impl fmt::Display for Target {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// A syntactically valid canonical address.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Address {
    text: String,
    segments: usize,
}

impl Address {
    /// Parse a canonical address: `<org>/<repo>/<context>` or
    /// `<org>/<repo>/<context>/<type>/<name>`, where the name may itself be
    /// nested. Partial, relative and absolute forms are rejected, so the
    /// address always resolves inside the store.
    pub fn parse(value: &str) -> Result<Self> {
        let expected = || anyhow::anyhow!("expected a canonical address\n  {ADDRESS_FORM}");
        if value.starts_with(['/', '~']) || value.contains(['\n', '\r', '\\']) {
            return Err(expected());
        }
        let segments: Vec<&str> = value.split('/').collect();
        let valid = segments
            .iter()
            .all(|segment| !segment.trim().is_empty() && *segment != "." && *segment != "..");
        if !valid || segments.len() < 3 || segments.len() == 4 {
            return Err(expected());
        }
        Ok(Self {
            text: value.to_string(),
            segments: segments.len(),
        })
    }

    pub fn as_str(&self) -> &str {
        &self.text
    }

    pub fn is_context(&self) -> bool {
        self.segments == 3
    }

    /// The artifact type segment, `None` for a context address.
    pub fn artifact_type(&self) -> Option<&str> {
        (!self.is_context()).then(|| self.text.split('/').nth(3).unwrap_or_default())
    }

    /// The file holding this address's metadata: `context.md` for a
    /// context, the artifact file otherwise.
    pub fn path(&self, store: &Path) -> PathBuf {
        let path = store.join(&self.text);
        if self.is_context() {
            path.join("context.md")
        } else {
            path
        }
    }

    pub fn exists(&self, store: &Path) -> bool {
        self.path(store).is_file()
    }

    /// The update target this address names.
    pub fn target(&self) -> Result<Target> {
        match self.artifact_type() {
            None => Ok(Target::Context),
            Some("task") => Ok(Target::Task),
            Some("plan") => Ok(Target::Plan),
            Some("note") => Ok(Target::Note),
            Some(kind @ ("spec" | "trace" | "review" | "bin" | "tmp")) => {
                bail!("{kind} artifacts cannot be updated")
            }
            Some(other) => bail!("unknown artifact type '{other}'"),
        }
    }
}

/// A change to the `parent` field.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Parent {
    Set(String),
    Clear,
}

/// A change to the `refs` list.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Refs {
    /// Add and remove individual entries, keeping the rest.
    Edit {
        add: Vec<String>,
        remove: Vec<String>,
    },
    /// Replace the whole list.
    Set(Vec<String>),
    /// Empty the list.
    Clear,
}

impl Default for Refs {
    fn default() -> Self {
        Self::Edit {
            add: Vec::new(),
            remove: Vec::new(),
        }
    }
}

/// The changes requested by one update. Unset fields are left alone.
#[derive(Clone, Debug, Default)]
pub struct Request {
    pub title: Option<String>,
    pub status: Option<String>,
    pub priority: Option<String>,
    pub description: Option<String>,
    pub kind: Option<String>,
    pub mode: Option<String>,
    pub parent: Option<Parent>,
    pub refs: Refs,
}

impl Request {
    fn scalars(&self) -> Vec<(Field, &str)> {
        [
            (Field::Title, &self.title),
            (Field::Status, &self.status),
            (Field::Priority, &self.priority),
            (Field::Description, &self.description),
            (Field::Kind, &self.kind),
            (Field::Mode, &self.mode),
        ]
        .into_iter()
        .filter_map(|(field, value)| value.as_deref().map(|value| (field, value)))
        .collect()
    }

    /// The fields this request asks to change.
    pub fn fields(&self) -> Vec<Field> {
        let mut fields: Vec<Field> = self.scalars().into_iter().map(|(field, _)| field).collect();
        if self.parent.is_some() {
            fields.push(Field::Parent);
        }
        if self.refs != Refs::default() {
            fields.push(Field::Refs);
        }
        fields
    }
}

/// The outcome of an update: every field whose value changed.
#[derive(Debug, Serialize)]
pub struct Mutation {
    pub mutation_id: String,
    pub address: String,
    pub changes: Vec<Change>,
}

/// One changed field. An absent value is `None`.
#[derive(Debug, Serialize, PartialEq)]
pub struct Change {
    pub field: &'static str,
    pub old: Option<Value>,
    pub new: Option<Value>,
}

/// Apply `request` to the target at `address` in the store at `store`.
///
/// Every change is validated before anything is written, and all of them
/// are written in one atomic replacement of the file. Changes that leave a
/// field as it is are dropped; when none remain, or with `dry_run`, nothing
/// is written. A store-wide lock is held from reading the target to writing
/// it, so concurrent updates, and the parent checks that read other files,
/// see a consistent store. A dry run takes no lock and creates no files.
pub fn update(store: &Path, address: &str, request: &Request, dry_run: bool) -> Result<Mutation> {
    let address = Address::parse(address)?;
    let target = address.target()?;
    for field in request.fields() {
        if !target.allows(field) {
            bail!("--{} is not a field of {target}", field.key());
        }
    }
    check_values(request)?;

    let _lock = if dry_run { None } else { Some(lock(store)?) };
    let path = address.path(store);
    if !path.is_file() {
        bail!("no {target} at {}", address.as_str());
    }
    let text = std::fs::read_to_string(&path)
        .with_context(|| format!("could not read {}", path.display()))?;
    let current = frontmatter::validate(&text)
        .with_context(|| format!("cannot update {}", address.as_str()))?;

    let mut edit = Edit {
        text: text.clone(),
        current: &current,
        changes: Vec::new(),
    };
    for (field, value) in request.scalars() {
        edit.set(field.key(), Some(Value::from(value)))?;
    }
    match &request.parent {
        Some(Parent::Set(parent)) => {
            check_parent(store, &address, target, parent)?;
            edit.set("parent", Some(Value::from(parent.as_str())))?;
        }
        Some(Parent::Clear) => edit.set("parent", None)?,
        None => {}
    }
    if request.refs != Refs::default() {
        let list = new_refs(store, &current, &request.refs)?;
        if !old_refs(&current).is_ok_and(|old| old == list) {
            let value = (!list.is_empty())
                .then(|| Value::Sequence(list.into_iter().map(Value::from).collect()));
            edit.set("refs", value)?;
        }
    }

    let max_lines = FRONTMATTER_MAX_LINES - 1;
    if !edit.changes.is_empty() && frontmatter_lines(&edit.text) > max_lines {
        bail!("frontmatter would exceed {max_lines} lines; artifact would be hidden from listings");
    }
    if !edit.changes.is_empty() && !dry_run {
        write_atomic(&path, &edit.text)?;
    }
    Ok(Mutation {
        mutation_id: uuid::Uuid::new_v4().to_string(),
        address: address.text,
        changes: edit.changes,
    })
}

/// Set or clear a context's archive stamp under the mutation lock.
/// Repeated archival keeps the existing stamp and produces no changes.
pub fn archive(store: &Path, address: &str, archived: bool) -> Result<Mutation> {
    let address = Address::parse(address)?;
    if !address.is_context() {
        bail!("expected a context address");
    }
    let _lock = lock(store)?;
    let path = address.path(store);
    if !path.is_file() {
        bail!("no context at {}", address.as_str());
    }
    let text = std::fs::read_to_string(&path)
        .with_context(|| format!("could not read {}", path.display()))?;
    let current = frontmatter::validate(&text)?;
    let old = current.get("archived_at").filter(|v| !v.is_null());
    let new = if archived {
        Some(match old {
            Some(value) => value.clone(),
            None => Value::from(
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)?
                    .as_secs(),
            ),
        })
    } else {
        None
    };
    let mut edit = Edit {
        text,
        current: &current,
        changes: Vec::new(),
    };
    edit.set("archived_at", new)?;
    if !edit.changes.is_empty() {
        write_atomic(&path, &edit.text)?;
    }
    Ok(Mutation {
        mutation_id: uuid::Uuid::new_v4().to_string(),
        address: address.text,
        changes: edit.changes,
    })
}

fn frontmatter_lines(text: &str) -> usize {
    text.lines()
        .skip(1)
        .take_while(|line| line.trim() != "---")
        .count()
}

struct Edit<'a> {
    text: String,
    current: &'a Mapping,
    changes: Vec<Change>,
}

impl Edit<'_> {
    fn set(&mut self, key: &'static str, new: Option<Value>) -> Result<()> {
        let old = self
            .current
            .get(key)
            .filter(|value| !value.is_null())
            .cloned();
        if old == new {
            return Ok(());
        }
        match &new {
            Some(value) => self.text = frontmatter::set(&self.text, key, value)?,
            None => {
                if let Some(text) = frontmatter::unset(&self.text, key)? {
                    self.text = text;
                }
            }
        }
        self.changes.push(Change {
            field: key,
            old,
            new,
        });
        Ok(())
    }
}

fn check_values(request: &Request) -> Result<()> {
    for (field, value) in request.scalars() {
        if let Some(values) = field.values() {
            if !values.contains(&value) {
                bail!(
                    "invalid value '{value}' for --{}\n  [possible values: {}]",
                    field.key(),
                    values.join(", ")
                );
            }
        } else if value.trim().is_empty() {
            bail!("--{} must not be empty", field.key());
        } else if value.contains(['\n', '\r']) {
            bail!("--{} must be a single line", field.key());
        }
    }
    if let Refs::Edit { add, remove } = &request.refs
        && let Some(both) = add.iter().find(|item| remove.contains(item))
    {
        bail!("--add-ref and --remove-ref both name {both}");
    }
    Ok(())
}

fn check_parent(
    store: &Path,
    target_address: &Address,
    target: Target,
    parent: &str,
) -> Result<()> {
    let parent_address =
        Address::parse(parent).with_context(|| format!("invalid parent '{parent}'"))?;
    let allowed = target.parents();
    let parent_target = parent_address.target().ok();
    if !parent_target.is_some_and(|kind| allowed.contains(&kind)) {
        let names: Vec<&str> = allowed.iter().map(|kind| kind.as_str()).collect();
        bail!(
            "{} {target}'s parent must be {}",
            article(target),
            names
                .iter()
                .map(|name| format!("{} {name}", article_str(name)))
                .collect::<Vec<_>>()
                .join(" or ")
        );
    }
    if !parent_address.exists(store) {
        bail!("parent not found: {parent}");
    }
    if parent_address == *target_address {
        bail!("cycle: {parent} cannot be its own parent");
    }

    let mut seen = HashSet::new();
    let mut current = parent_address;
    // An artifact's parent chain ends at a context, and contexts only parent
    // to contexts, so once the walk crosses from artifacts to contexts it
    // can never lead back to the artifact being updated.
    while current.is_context() == target_address.is_context() && seen.insert(current.clone()) {
        if current == *target_address {
            bail!("cycle: {parent} is below {}", target_address.as_str());
        }
        let Some(next) = read_parent(store, &current) else {
            break;
        };
        let Ok(next) = Address::parse(&next) else {
            break;
        };
        current = next;
    }
    Ok(())
}

fn article(target: Target) -> &'static str {
    article_str(target.as_str())
}

fn article_str(name: &str) -> &'static str {
    if name.starts_with(['a', 'e', 'i', 'o', 'u']) {
        "an"
    } else {
        "a"
    }
}

fn read_parent(store: &Path, address: &Address) -> Option<String> {
    let yaml = crate::artifact::extract_frontmatter_yaml(&address.path(store))?;
    let mapping: Mapping = serde_yaml::from_str(&yaml).ok()?;
    mapping.get("parent")?.as_str().map(str::to_string)
}

fn old_refs(current: &Mapping) -> Result<Vec<String>> {
    match current.get("refs") {
        None | Some(Value::Null) => Ok(Vec::new()),
        Some(Value::Sequence(items)) => items
            .iter()
            .map(|item| {
                item.as_str()
                    .map(str::to_string)
                    .context("refs holds an entry that is not an address")
            })
            .collect(),
        Some(_) => bail!("refs is not a list of addresses"),
    }
}

fn new_refs(store: &Path, current: &Mapping, refs: &Refs) -> Result<Vec<String>> {
    let check = |reference: &String| -> Result<()> {
        let address =
            Address::parse(reference).with_context(|| format!("invalid ref '{reference}'"))?;
        if !address.exists(store) {
            bail!("ref not found: {reference}");
        }
        Ok(())
    };
    let mut list = match refs {
        Refs::Edit { add, remove } => {
            add.iter().try_for_each(check)?;
            let mut list = old_refs(current)?;
            list.retain(|item| !remove.contains(item));
            list.extend(add.iter().cloned());
            list
        }
        Refs::Set(items) => {
            items.iter().try_for_each(check)?;
            items.clone()
        }
        Refs::Clear => Vec::new(),
    };
    let mut seen = HashSet::new();
    list.retain(|item| seen.insert(item.clone()));
    Ok(list)
}

pub(crate) fn lock(store: &Path) -> Result<File> {
    let dir = store.join(".state");
    std::fs::create_dir_all(&dir).with_context(|| format!("could not create {}", dir.display()))?;
    let path = dir.join("mutation.lock");
    let file = OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(false)
        .open(&path)
        .with_context(|| format!("could not open {}", path.display()))?;
    file.lock()
        .with_context(|| format!("could not lock {}", path.display()))?;
    Ok(file)
}

fn write_atomic(path: &Path, text: &str) -> Result<()> {
    let dir = path.parent().context("target has no parent directory")?;
    let mut file = tempfile::NamedTempFile::new_in(dir)?;
    file.as_file()
        .set_permissions(std::fs::metadata(path)?.permissions())?;
    std::io::Write::write_all(&mut file, text.as_bytes())?;
    file.as_file().sync_all()?;
    file.persist(path)
        .with_context(|| format!("could not write {}", path.display()))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    struct Store(TempDir);

    impl Store {
        fn new() -> Self {
            let store = Self(TempDir::new().unwrap());
            store.write(
                "a/b/ctx/context.md",
                "---\nkind: work\ncreated_at: 1\n---\n",
            );
            store.write(
                "a/b/other/context.md",
                "---\nkind: work\nparent: a/b/ctx\n---\n",
            );
            store.write(
                "a/b/ctx/task/foo.md",
                "---\ntitle: Foo\nstatus: open\nrefs:\n- a/b/other\n- a/b/gone\ncreated_at: 1\n---\n# Foo\n\nbody\n",
            );
            store.write(
                "a/b/ctx/task/sub.md",
                "---\nstatus: open\nparent: a/b/ctx/task/foo.md\n---\n",
            );
            store.write("a/b/ctx/note/idea.md", "---\nstatus: open\n---\n");
            store.write("a/b/ctx/plan/impl.md", "---\nstatus: open\n---\n");
            store.write("a/b/ctx/trace/handoff.md", "---\nrepo_id: x\n---\n");
            store
        }

        fn path(&self) -> &Path {
            self.0.path()
        }

        fn write(&self, address: &str, text: &str) {
            let path = self.0.path().join(address);
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(path, text).unwrap();
        }

        fn read(&self, address: &str) -> String {
            std::fs::read_to_string(self.0.path().join(address)).unwrap()
        }

        fn update(&self, address: &str, request: Request) -> Result<Mutation> {
            update(self.path(), address, &request, false)
        }

        fn error(&self, address: &str, request: Request) -> String {
            format!("{:#}", self.update(address, request).unwrap_err())
        }
    }

    const FOO: &str = "a/b/ctx/task/foo.md";

    fn change(field: &'static str, old: Option<&str>, new: Option<&str>) -> Change {
        Change {
            field,
            old: old.map(Value::from),
            new: new.map(Value::from),
        }
    }

    #[test]
    fn addresses_are_canonical_contexts_or_artifacts() {
        assert!(Address::parse("a/b/c").unwrap().is_context());
        assert_eq!(
            Address::parse("a/b/c/note/ideas/x.md")
                .unwrap()
                .artifact_type(),
            Some("note")
        );
        for invalid in [
            "task/foo.md",
            "a/b",
            "a/b/c/task",
            "/a/b/c",
            "~/a/b/c",
            "a/../c",
            "a//c/d/e",
            "",
        ] {
            let error = Address::parse(invalid).unwrap_err().to_string();
            assert!(
                error.starts_with("expected a canonical address"),
                "{invalid}: {error}"
            );
        }
    }

    #[test]
    fn status_change_writes_only_that_line() {
        let store = Store::new();
        let before = store.read(FOO);
        let mutation = store
            .update(
                FOO,
                Request {
                    status: Some("complete".into()),
                    ..Default::default()
                },
            )
            .unwrap();
        assert_eq!(mutation.address, FOO);
        assert_eq!(
            mutation.changes,
            vec![change("status", Some("open"), Some("complete"))]
        );
        assert_eq!(
            store.read(FOO),
            before.replace("status: open", "status: complete")
        );
    }

    #[test]
    fn unchanged_values_write_nothing() {
        let store = Store::new();
        let path = store.path().join(FOO);
        let modified = std::fs::metadata(&path).unwrap().modified().unwrap();
        let mutation = store
            .update(
                FOO,
                Request {
                    status: Some("open".into()),
                    refs: Refs::Edit {
                        add: vec!["a/b/other".into()],
                        remove: vec!["a/b/none".into()],
                    },
                    ..Default::default()
                },
            )
            .unwrap();
        assert!(mutation.changes.is_empty());
        assert_eq!(
            std::fs::metadata(&path).unwrap().modified().unwrap(),
            modified
        );
    }

    #[test]
    fn dry_run_reports_without_writing() {
        let store = Store::new();
        let before = store.read(FOO);
        let request = Request {
            status: Some("closed".into()),
            ..Default::default()
        };
        let mutation = update(store.path(), FOO, &request, true).unwrap();
        assert_eq!(mutation.changes.len(), 1);
        assert_eq!(store.read(FOO), before);
        assert!(!store.path().join(".state").exists());
    }

    #[test]
    fn refs_edits_keep_other_entries_and_allow_removing_dangling_ones() {
        let store = Store::new();
        store
            .update(
                FOO,
                Request {
                    refs: Refs::Edit {
                        add: vec!["a/b/ctx/note/idea.md".into()],
                        remove: vec!["a/b/gone".into()],
                    },
                    ..Default::default()
                },
            )
            .unwrap();
        assert!(
            store
                .read(FOO)
                .contains("refs:\n- a/b/other\n- a/b/ctx/note/idea.md\ncreated_at: 1\n")
        );
    }

    #[test]
    fn clearing_refs_removes_the_key() {
        let store = Store::new();
        let mutation = store
            .update(
                FOO,
                Request {
                    refs: Refs::Clear,
                    ..Default::default()
                },
            )
            .unwrap();
        assert_eq!(mutation.changes[0].new, None);
        assert!(!store.read(FOO).contains("refs"));
        assert!(store.read(FOO).ends_with("---\n# Foo\n\nbody\n"));
    }

    #[test]
    fn replacing_refs_repairs_a_value_that_is_not_a_list() {
        let store = Store::new();
        let scalar = "a/b/ctx/note/scalar.md";
        store.write(scalar, "---\nstatus: open\nrefs: a/b/other\n---\n");
        let error = store.error(
            scalar,
            Request {
                refs: Refs::Edit {
                    add: vec!["a/b/ctx".into()],
                    remove: Vec::new(),
                },
                ..Default::default()
            },
        );
        assert_eq!(error, "refs is not a list of addresses");

        let mutation = store
            .update(
                scalar,
                Request {
                    refs: Refs::Set(vec!["a/b/other".into()]),
                    ..Default::default()
                },
            )
            .unwrap();
        assert_eq!(mutation.changes[0].old, Some(Value::from("a/b/other")));
        assert_eq!(
            store.read(scalar),
            "---\nstatus: open\nrefs:\n- a/b/other\n---\n"
        );

        store.write(scalar, "---\nstatus: open\nrefs: a/b/other\n---\n");
        let mutation = store
            .update(
                scalar,
                Request {
                    refs: Refs::Clear,
                    ..Default::default()
                },
            )
            .unwrap();
        assert_eq!(
            mutation.changes,
            vec![change("refs", Some("a/b/other"), None)]
        );
        assert_eq!(store.read(scalar), "---\nstatus: open\n---\n");
    }

    #[test]
    fn added_refs_must_exist() {
        let store = Store::new();
        let error = store.error(
            FOO,
            Request {
                refs: Refs::Set(vec!["a/b/ctx/note/missing.md".into()]),
                ..Default::default()
            },
        );
        assert_eq!(error, "ref not found: a/b/ctx/note/missing.md");
    }

    #[test]
    fn fields_outside_the_target_schema_are_refused() {
        let store = Store::new();
        let error = store.error(
            "a/b/ctx/plan/impl.md",
            Request {
                priority: Some("high".into()),
                ..Default::default()
            },
        );
        assert_eq!(error, "--priority is not a field of plan");
        let error = store.error(
            "a/b/ctx",
            Request {
                status: Some("complete".into()),
                ..Default::default()
            },
        );
        assert_eq!(error, "--status is not a field of context");
    }

    #[test]
    fn enum_values_are_checked() {
        let store = Store::new();
        let error = store.error(
            FOO,
            Request {
                status: Some("done".into()),
                ..Default::default()
            },
        );
        assert_eq!(
            error,
            "invalid value 'done' for --status\n  [possible values: open, in-progress, complete, closed]"
        );
    }

    #[test]
    fn unsupported_types_are_refused() {
        let store = Store::new();
        let error = store.error(
            "a/b/ctx/trace/handoff.md",
            Request {
                status: Some("open".into()),
                ..Default::default()
            },
        );
        assert_eq!(error, "trace artifacts cannot be updated");
    }

    #[test]
    fn parent_rules_are_enforced() {
        let store = Store::new();
        let parent = |value: &str| Request {
            parent: Some(Parent::Set(value.into())),
            ..Default::default()
        };
        assert_eq!(
            store.error("a/b/ctx/plan/impl.md", parent("a/b/ctx/note/idea.md")),
            "a plan's parent must be a task or a context"
        );
        assert_eq!(
            store.error("a/b/ctx", parent(FOO)),
            "a context's parent must be a context"
        );
        assert_eq!(
            store.error(FOO, parent("a/b/ctx/task/missing.md")),
            "parent not found: a/b/ctx/task/missing.md"
        );
        assert_eq!(
            store.error(FOO, parent(FOO)),
            format!("cycle: {FOO} cannot be its own parent")
        );
        assert_eq!(
            store.error(FOO, parent("a/b/ctx/task/sub.md")),
            format!("cycle: a/b/ctx/task/sub.md is below {FOO}")
        );
        assert_eq!(
            store.error("a/b/ctx", parent("a/b/other")),
            "cycle: a/b/other is below a/b/ctx"
        );
    }

    #[test]
    fn parent_can_be_set_and_cleared() {
        let store = Store::new();
        let mutation = store
            .update(
                "a/b/ctx/plan/impl.md",
                Request {
                    parent: Some(Parent::Set(FOO.into())),
                    ..Default::default()
                },
            )
            .unwrap();
        assert_eq!(mutation.changes, vec![change("parent", None, Some(FOO))]);
        let mutation = store
            .update(
                "a/b/other",
                Request {
                    parent: Some(Parent::Clear),
                    ..Default::default()
                },
            )
            .unwrap();
        assert_eq!(
            mutation.changes,
            vec![change("parent", Some("a/b/ctx"), None)]
        );
        assert_eq!(store.read("a/b/other/context.md"), "---\nkind: work\n---\n");
    }

    #[test]
    fn frontmatter_the_editor_cannot_handle_is_refused() {
        let store = Store::new();
        store.write(
            "a/b/ctx/task/dup.md",
            "---\nstatus: open\nstatus: closed\n---\n",
        );
        let error = store.error(
            "a/b/ctx/task/dup.md",
            Request {
                title: Some("x".into()),
                ..Default::default()
            },
        );
        assert!(
            error.contains("appears more than once") || error.contains("duplicate"),
            "{error}"
        );
    }

    #[test]
    fn missing_targets_are_reported() {
        let store = Store::new();
        let error = store.error(
            "a/b/ctx/task/missing.md",
            Request {
                status: Some("open".into()),
                ..Default::default()
            },
        );
        assert_eq!(error, "no task at a/b/ctx/task/missing.md");
    }

    #[test]
    fn frontmatter_is_kept_within_the_read_cap() {
        let store = Store::new();
        let note = |lines: usize| {
            let keys: String = (1..lines).map(|i| format!("key{i}: x\n")).collect();
            format!("---\nstatus: open\n{keys}---\n# Note\n")
        };
        let request = || Request {
            refs: Refs::Edit {
                add: vec!["a/b/other".into()],
                remove: vec![],
            },
            ..Default::default()
        };
        let idea = "a/b/ctx/note/idea.md";

        store.write(idea, &note(FRONTMATTER_MAX_LINES - 2));
        let before = store.read(idea);
        for dry_run in [true, false] {
            let error = format!(
                "{:#}",
                update(store.path(), idea, &request(), dry_run).unwrap_err()
            );
            assert_eq!(
                error,
                format!(
                    "frontmatter would exceed {} lines; artifact would be hidden from listings",
                    FRONTMATTER_MAX_LINES - 1
                )
            );
            assert_eq!(store.read(idea), before);
        }

        store.write(idea, &note(FRONTMATTER_MAX_LINES - 3));
        store.update(idea, request()).unwrap();
        let path = store.path().join(idea);
        assert!(crate::artifact::extract_frontmatter_yaml(&path).is_some());
    }
}
