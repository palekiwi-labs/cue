use crate::cli::{ContextCommands, ContextSort, QueryScope};
use anyhow::Context as _;
use cuelib::artifact::extract_frontmatter_yaml;
use cuelib::store;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Serialize)]
struct NewContextMetadata<'a> {
    #[serde(skip_serializing_if = "Option::is_none")]
    title: Option<&'a str>,
    kind: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    mode: Option<&'static str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    description: Option<&'a str>,
    created_at: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    parent: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    refs: Option<&'a [String]>,
}

struct NewContextOptions<'a> {
    title: Option<&'a str>,
    kind: &'static str,
    mode: Option<&'static str>,
    description: Option<&'a str>,
    parent: Option<&'a str>,
    refs: &'a [String],
}

#[derive(Deserialize)]
struct StoredContextMetadata {
    title: Option<String>,
    kind: String,
    mode: Option<String>,
    description: Option<String>,
    created_at: u64,
    parent: Option<String>,
    #[serde(default)]
    refs: Vec<String>,
}

#[derive(Serialize)]
struct ContextListEntry {
    context: String,
    scope: String,
    title: Option<String>,
    kind: String,
    mode: Option<String>,
    description: Option<String>,
    created_at: u64,
    parent: Option<String>,
    refs: Vec<String>,
    /// Populated for JSON output or recency sorting. Preserve nanoseconds for
    /// ordering, but expose whole Unix seconds (or null) in JSON.
    #[serde(rename = "last_logged_at", serialize_with = "serialize_seconds")]
    last_logged_nanos: Option<u64>,
    path: PathBuf,
}

/// Report a nanosecond log stamp as whole Unix seconds.
///
/// `None` is written as null rather than omitted.
fn serialize_seconds<S: serde::Serializer>(
    nanos: &Option<u64>,
    serializer: S,
) -> Result<S::Ok, S::Error> {
    nanos
        .map(|nanos| nanos / 1_000_000_000)
        .serialize(serializer)
}

pub fn handle(
    cwd: &Path,
    command: ContextCommands,
    store_root: Option<&Path>,
) -> anyhow::Result<()> {
    match command {
        ContextCommands::Create {
            name,
            title,
            kind,
            mode,
            description,
            parent,
            refs,
        } => {
            let options = NewContextOptions {
                title: title.as_deref(),
                kind: kind.as_str(),
                mode: mode.map(|mode| mode.as_str()),
                description: description.as_deref(),
                parent: parent.as_deref(),
                refs: &refs,
            };
            handle_create(cwd, &name, options, store_root)
        }
        ContextCommands::List {
            json,
            pinned,
            scope,
            sort,
            limit,
        } => handle_list(cwd, json, pinned, scope, sort, limit, store_root),
        ContextCommands::Pin { context } => handle_pin(cwd, &context, store_root),
        ContextCommands::Unpin { context } => handle_unpin(cwd, &context, store_root),
        ContextCommands::Pins { scope } => handle_pins(cwd, scope, store_root),
        ContextCommands::Switch { slug, branch } => handle_switch(cwd, &slug, branch),
        ContextCommands::Unset { branch } => handle_unset(cwd, branch),
    }
}

/// Pin state lives outside every context, under the store root, as a directory
/// of zero-byte marker files at `<store>/.state/pins/<org>/<repo>/<slug>`. It
/// is operator state rather than metadata describing a context.
fn pins_dir(store_root: &Path) -> PathBuf {
    store_root.join(".state").join("pins")
}

/// The accepted pin argument forms, quoted back in every error so the message
/// names the shape that was expected.
const PIN_FORM: &str = "expected a context slug in the current repository scope \
     or a canonical '<org>/<repo>/<slug>' address";

/// Resolve a pin argument to a canonical context address.
///
/// Resolution is shape-only. A pin names a context that may not exist yet and
/// may outlive the context it names, so no store lookup is performed. The
/// repository scope is resolved only for the bare-slug form, so the address
/// form pins a context from anywhere.
fn resolve_pin_address(cwd: &Path, value: &str) -> anyhow::Result<String> {
    // Checked on the whole value rather than per segment, matching
    // `address::validate_shape`: `~` is a shell and path convention, and a
    // value leading with it is a path being passed where an address belongs.
    if value.starts_with('~') {
        anyhow::bail!(
            "Invalid context '{value}': home-relative paths are not addresses; {PIN_FORM}"
        );
    }

    let segments: Vec<&str> = value.split('/').collect();
    let address = match segments.as_slice() {
        [slug] => {
            validate_pin_segment(value, slug)?;
            format!("{}/{slug}", scope_prefix(cwd)?)
        }
        [org, repo, slug] => {
            for segment in [org, repo, slug] {
                validate_pin_segment(value, segment)?;
            }
            format!("{org}/{repo}/{slug}")
        }
        _ => anyhow::bail!("Invalid context '{value}': {PIN_FORM}"),
    };
    Ok(address)
}

/// Each address segment must be a single, safe path segment: a pin argument
/// becomes a filesystem path under the store's state directory.
///
/// Path semantics alone are too permissive here, because a pin is also
/// printed back as a line of `pins` output. A line break would split one
/// address across two lines, and a whitespace-only segment would print as a
/// gap that names nothing, so both are rejected on top of the path rules. An
/// ordinary internal space is left alone: it is a character of the slug.
fn validate_pin_segment(value: &str, segment: &str) -> anyhow::Result<()> {
    let invalid = || anyhow::anyhow!("Invalid context '{value}': {PIN_FORM}");
    if segment.contains(['\n', '\r']) || segment.trim().is_empty() {
        return Err(invalid());
    }
    cuelib::head::validate_slug(segment).map_err(|_| invalid())
}

/// The current repository scope, as the `<org>/<repo>` prefix of an address.
fn scope_prefix(cwd: &Path) -> anyhow::Result<String> {
    let scope = store::repository_scope(cwd)?;
    Ok(scope
        .to_str()
        .context("repository scope is not valid UTF-8")?
        .to_string())
}

fn handle_pin(cwd: &Path, context: &str, store_root: Option<&Path>) -> anyhow::Result<()> {
    let address = resolve_pin_address(cwd, context)?;
    let marker = pins_dir(&store::root(store_root)?).join(&address);
    let scope_dir = marker
        .parent()
        .context("pin marker path has no scope directory")?;
    std::fs::create_dir_all(scope_dir)
        .with_context(|| format!("could not create pin state at {}", scope_dir.display()))?;

    // An exclusive create is atomic, and treating an existing marker as
    // success makes the pin idempotent without a read-modify-write.
    match std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&marker)
    {
        Ok(_) => {}
        Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {}
        Err(error) => {
            return Err(error)
                .with_context(|| format!("could not pin context at {}", marker.display()));
        }
    }

    println!("pinned {address}");
    Ok(())
}

fn handle_unpin(cwd: &Path, context: &str, store_root: Option<&Path>) -> anyhow::Result<()> {
    let address = resolve_pin_address(cwd, context)?;
    let marker = pins_dir(&store::root(store_root)?).join(&address);

    // An unlink is atomic; an absent marker is already the desired state. The
    // emptied scope directory is deliberately left in place, because pruning
    // would reintroduce a write race.
    match std::fs::remove_file(&marker) {
        Ok(()) => {}
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => {
            return Err(error)
                .with_context(|| format!("could not unpin context at {}", marker.display()));
        }
    }

    println!("unpinned {address}");
    Ok(())
}

/// List the working set at the requested breadth.
///
/// The repository scope is resolved only for `QueryScope::Repo`, so the
/// whole-store view works from anywhere, including outside a repository.
fn handle_pins(cwd: &Path, scope: QueryScope, store_root: Option<&Path>) -> anyhow::Result<()> {
    let pins_dir = pins_dir(&store::root(store_root)?);
    let mut addresses: Vec<String> = read_pins(cwd, scope, &pins_dir)?
        .into_iter()
        .map(|(scope, context)| format!("{scope}/{context}"))
        .collect();

    // Alphabetical order by canonical address is deterministic and stable;
    // there is no recency or operator-chosen ordering.
    addresses.sort();
    for address in addresses {
        println!("{address}");
    }
    Ok(())
}

/// The pinned working set at the requested breadth, as `(scope, context)`
/// pairs in directory order.
///
/// Reading pin state costs one directory listing per scope and opens nothing,
/// because a pin is a name rather than a document. The repository scope is
/// resolved only for `QueryScope::Repo`, so the whole-store view works from
/// anywhere, including outside a repository.
fn read_pins(
    cwd: &Path,
    scope: QueryScope,
    pins_dir: &Path,
) -> anyhow::Result<Vec<(String, String)>> {
    let mut pins = Vec::new();
    match scope {
        QueryScope::Store => {
            // Joining the scope directories and the filename is what yields an
            // address, so only entries with that shape are pins.
            for (scope, scope_dir) in scope_dirs(pins_dir)? {
                collect_scope_pins(&scope_dir, &scope, &mut pins)?;
            }
        }
        QueryScope::Repo => {
            let scope = scope_prefix(cwd)?;
            collect_scope_pins(&pins_dir.join(&scope), &scope, &mut pins)?;
        }
    }
    Ok(pins)
}

/// Append the pins held directly in one `<org>/<repo>` directory.
fn collect_scope_pins(
    scope_dir: &Path,
    scope: &str,
    pins: &mut Vec<(String, String)>,
) -> anyhow::Result<()> {
    for marker in read_optional_dir(scope_dir)? {
        if !marker.is_file() {
            continue;
        }
        pins.push((scope.to_string(), entry_name(&marker)?.to_string()));
    }
    Ok(())
}

/// Every `<org>/<repo>` scope directory directly below `root`, paired with the
/// `<org>/<repo>` prefix its contents address.
///
/// Joining the two directory levels is what yields an address, so entries
/// without that shape are skipped rather than failing the walk. What counts as
/// a scope at a given root is the caller's concern: this walks shape only.
fn scope_dirs(root: &Path) -> anyhow::Result<Vec<(String, PathBuf)>> {
    let mut scopes = Vec::new();
    for org in read_optional_dir(root)? {
        if !org.is_dir() {
            continue;
        }
        let org_name = entry_name(&org)?.to_string();
        for repo in read_optional_dir(&org)? {
            if !repo.is_dir() {
                continue;
            }
            let scope = format!("{org_name}/{}", entry_name(&repo)?);
            scopes.push((scope, repo));
        }
    }
    Ok(scopes)
}

/// Read a directory, treating a missing one as empty.
///
/// Absence is the ordinary case for both callers: a store may hold no pin
/// state at all, scope directories left behind by the last unpin in a scope
/// are retained rather than pruned, and a repository scope holding no context
/// has no directory.
fn read_optional_dir(dir: &Path) -> anyhow::Result<Vec<PathBuf>> {
    let entries = match std::fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(error) => {
            return Err(error)
                .with_context(|| format!("could not read directory at {}", dir.display()));
        }
    };
    let mut paths = Vec::new();
    for entry in entries {
        paths.push(entry?.path());
    }
    Ok(paths)
}

fn entry_name(entry: &Path) -> anyhow::Result<&str> {
    entry
        .file_name()
        .and_then(|name| name.to_str())
        .with_context(|| format!("store entry name is not valid UTF-8: {}", entry.display()))
}

fn handle_switch(cwd: &Path, slug: &str, branch: Option<String>) -> anyhow::Result<()> {
    let branch = target_branch(cwd, branch, "switch")?;
    cuelib::git::set_branch_context(cwd, &branch, slug)?;
    println!("switched branch '{branch}' to context '{slug}'");
    Ok(())
}

fn handle_unset(cwd: &Path, branch: Option<String>) -> anyhow::Result<()> {
    let branch = target_branch(cwd, branch, "unset")?;
    cuelib::git::unset_branch_context(cwd, &branch)?;
    println!("unset context for branch '{branch}'");
    Ok(())
}

fn target_branch(cwd: &Path, branch: Option<String>, action: &str) -> anyhow::Result<String> {
    let branch = branch
        .or_else(|| cuelib::git::current_branch(cwd))
        .with_context(|| {
            format!(
                "cannot {action} context in detached HEAD; specify target branch with --branch <name>"
            )
        })?;
    Ok(branch)
}

/// List contexts at the requested breadth.
///
/// The repository scope is resolved only for `QueryScope::Repo`, so the
/// whole-store view works from anywhere, including outside a repository.
///
/// `pinned` selects what is enumerated rather than narrowing what was
/// enumerated. Pin state is a flat directory of markers so that the working
/// set is answerable in one directory listing; enumerating every context in
/// the store and discarding the unpinned ones would spend the cost that
/// design exists to avoid, and grows with the store rather than with the
/// working set.
fn handle_list(
    cwd: &Path,
    json: bool,
    pinned: bool,
    scope: QueryScope,
    sort: Option<ContextSort>,
    limit: Option<usize>,
    store_root: Option<&Path>,
) -> anyhow::Result<()> {
    let store_root = store::root(store_root)?;
    let mut contexts = Vec::new();
    if pinned {
        let pins = read_pins(cwd, scope, &pins_dir(&store_root))?;
        collect_pinned_contexts(&store_root, &pins, &mut contexts)?;
    } else {
        match scope {
            QueryScope::Store => {
                for (scope, scope_dir) in scope_dirs(&store_root)? {
                    // Directly below the store root, a dot-named directory is
                    // store-internal state such as `.state`, never a
                    // repository scope: an origin-derived scope cannot be
                    // dot-named.
                    if scope.starts_with('.') {
                        continue;
                    }
                    collect_scope_contexts(&scope_dir, &scope, &mut contexts)?;
                }
            }
            QueryScope::Repo => {
                let scope = scope_prefix(cwd)?;
                collect_scope_contexts(&store_root.join(&scope), &scope, &mut contexts)?;
            }
        }
    }

    // Ordering by canonical address is deterministic and stable. Within one
    // scope this is ordering by slug, so the default view is unchanged. It is
    // also the tiebreak any requested ordering falls back on, so it is
    // established first.
    contexts
        .sort_by(|left, right| (&left.scope, &left.context).cmp(&(&right.scope, &right.context)));

    // Scan filenames once per context when needed for ordering or JSON.
    // Plain default listing does not scan log directories.
    let recency = sort == Some(ContextSort::Recency);
    if recency || json {
        read_log_activity(&mut contexts)?;
    }

    if recency {
        // A context with no log entry has no activity to order by and sorts
        // last, which `None` compares as under the reversed key. The sort is
        // stable and the input is already in canonical address order, so
        // contexts sharing a stamp keep that order. The key is the nanosecond
        // stamp rather than the second that is reported, so entries written
        // within one second stay in the order they happened.
        contexts.sort_by_key(|context| std::cmp::Reverse(context.last_logged_nanos));
    }

    // Truncation is applied to the finished order, so a limit selects the
    // leading rows of what was asked for rather than an arbitrary subset that
    // is then ordered. A limit of zero selects nothing, which is a listing of
    // no contexts and not an error.
    if let Some(limit) = limit {
        contexts.truncate(limit);
    }

    if json {
        println!("{}", serde_json::to_string_pretty(&contexts)?);
        return Ok(());
    }
    for context in contexts {
        match scope {
            // A bare slug identifies a context only within one scope, and is
            // what `cue context switch` accepts.
            QueryScope::Repo => println!("{}", context.context),
            QueryScope::Store => println!("{}/{}", context.scope, context.context),
        }
    }
    Ok(())
}

/// Record each context's latest log activity, as the nanosecond stamp of its
/// newest log entry.
///
/// Log filenames are scanned before the listing is ordered or written out, so a log
/// directory that cannot be listed fails rather than silently leaving a
/// context standing as one with no activity.
fn read_log_activity(contexts: &mut [ContextListEntry]) -> anyhow::Result<()> {
    for context in contexts {
        let context_dir = context.path.parent().with_context(|| {
            format!("context path has no directory: {}", context.path.display())
        })?;
        context.last_logged_nanos = crate::log::latest_timestamp(context_dir)?;
    }
    Ok(())
}

/// Append the contexts held directly in one `<org>/<repo>` directory.
///
/// A context is a directory containing `context.md`; anything else in a scope
/// directory is skipped rather than failing the listing.
fn collect_scope_contexts(
    scope_dir: &Path,
    scope: &str,
    contexts: &mut Vec<ContextListEntry>,
) -> anyhow::Result<()> {
    for context_dir in read_optional_dir(scope_dir)? {
        let context = entry_name(&context_dir)?.to_string();
        if let Some(entry) = read_context_entry(&context_dir, scope, &context)? {
            contexts.push(entry);
        }
    }
    Ok(())
}

/// Append the contexts the working set names, resolving each pin by address.
///
/// A pin is a name, and the context it names may never have existed or may
/// have been deleted since. Such a pin has no row to report, so it is skipped:
/// the working set keeps it, and `cue context pins` is what reports it.
fn collect_pinned_contexts(
    store_root: &Path,
    pins: &[(String, String)],
    contexts: &mut Vec<ContextListEntry>,
) -> anyhow::Result<()> {
    for (scope, context) in pins {
        // Resolution is direct concatenation onto the store root, which is
        // what makes narrowing to the working set cost one read per pin.
        let context_dir = store_root.join(scope).join(context);
        if let Some(entry) = read_context_entry(&context_dir, scope, context)? {
            contexts.push(entry);
        }
    }
    Ok(())
}

/// Read one context directory into a listing row, or `None` when it holds no
/// `context.md` and so is not a context at all.
fn read_context_entry(
    context_dir: &Path,
    scope: &str,
    context: &str,
) -> anyhow::Result<Option<ContextListEntry>> {
    let context_path = context_dir.join("context.md");
    if !context_path.is_file() {
        return Ok(None);
    }
    let frontmatter = extract_frontmatter_yaml(&context_path).with_context(|| {
        format!(
            "could not read context metadata at {}",
            context_path.display()
        )
    })?;
    let metadata: StoredContextMetadata = serde_yaml::from_str(&frontmatter)
        .with_context(|| format!("invalid context metadata at {}", context_path.display()))?;
    Ok(Some(ContextListEntry {
        context: context.to_string(),
        scope: scope.to_string(),
        title: metadata.title,
        kind: metadata.kind,
        mode: metadata.mode,
        description: metadata.description,
        created_at: metadata.created_at,
        parent: metadata.parent,
        refs: metadata.refs,
        last_logged_nanos: None,
        path: context_path,
    }))
}

fn handle_create(
    cwd: &Path,
    name: &str,
    options: NewContextOptions<'_>,
    store_root: Option<&Path>,
) -> anyhow::Result<()> {
    cuelib::head::validate_slug(name)?;

    let store_root = store::root(store_root)?;
    if let Some(parent) = options.parent {
        crate::address::validate_reference("parent", parent, &store_root)?;
    }
    for reference in options.refs {
        crate::address::validate_reference("ref", reference, &store_root)?;
    }

    let repository_scope = store::repository_scope(cwd)?;
    let repository_dir = store_root.join(&repository_scope);
    let context_dir = repository_dir.join(name);
    let context_path = context_dir.join("context.md");
    if context_path.exists() {
        anyhow::bail!("Context already exists: {name}");
    }

    let metadata = NewContextMetadata {
        title: options.title,
        kind: options.kind,
        mode: options.mode,
        description: options.description,
        created_at: SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs(),
        parent: options.parent,
        refs: (!options.refs.is_empty()).then_some(options.refs),
    };
    let frontmatter = serde_yaml::to_string(&metadata)?;
    std::fs::create_dir_all(&context_dir)?;
    std::fs::write(context_path, format!("---\n{frontmatter}---\n"))?;

    println!("Created {}/{}", repository_scope.display(), name);
    Ok(())
}
