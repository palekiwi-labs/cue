use crate::cli::{ContextCommands, QueryScope};
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
    path: PathBuf,
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
        ContextCommands::List { json } => handle_list(cwd, json, store_root),
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
fn pins_dir(store_root: Option<&Path>) -> anyhow::Result<PathBuf> {
    Ok(store::root(store_root)?.join(".state").join("pins"))
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
    let marker = pins_dir(store_root)?.join(&address);
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
    let marker = pins_dir(store_root)?.join(&address);

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
    let pins_dir = pins_dir(store_root)?;
    let mut addresses = Vec::new();
    match scope {
        QueryScope::Store => {
            // Joining the scope directories and the filename is what yields an
            // address, so only entries with that shape are pins.
            for org in read_state_dir(&pins_dir)? {
                if !org.is_dir() {
                    continue;
                }
                let org_name = state_entry_name(&org)?;
                for repo in read_state_dir(&org)? {
                    if !repo.is_dir() {
                        continue;
                    }
                    let repo_name = state_entry_name(&repo)?;
                    collect_scope_pins(&repo, &format!("{org_name}/{repo_name}"), &mut addresses)?;
                }
            }
        }
        QueryScope::Repo => {
            let scope = scope_prefix(cwd)?;
            collect_scope_pins(&pins_dir.join(&scope), &scope, &mut addresses)?;
        }
    }

    // Alphabetical order by canonical address is deterministic and stable;
    // there is no recency or operator-chosen ordering.
    addresses.sort();
    for address in addresses {
        println!("{address}");
    }
    Ok(())
}

/// Append the pins held directly in one `<org>/<repo>` directory.
fn collect_scope_pins(
    scope_dir: &Path,
    scope: &str,
    addresses: &mut Vec<String>,
) -> anyhow::Result<()> {
    for marker in read_state_dir(scope_dir)? {
        if !marker.is_file() {
            continue;
        }
        addresses.push(format!("{scope}/{}", state_entry_name(&marker)?));
    }
    Ok(())
}

/// Read a pin state directory, treating a missing one as empty.
///
/// A missing pins directory is an empty working set rather than an error, and
/// scope directories left behind by the last unpin in a scope are retained
/// rather than pruned, so an empty read is the ordinary case.
fn read_state_dir(dir: &Path) -> anyhow::Result<Vec<PathBuf>> {
    let entries = match std::fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(error) => {
            return Err(error)
                .with_context(|| format!("could not read pin state at {}", dir.display()));
        }
    };
    let mut paths = Vec::new();
    for entry in entries {
        paths.push(entry?.path());
    }
    Ok(paths)
}

fn state_entry_name(entry: &Path) -> anyhow::Result<&str> {
    entry
        .file_name()
        .and_then(|name| name.to_str())
        .with_context(|| {
            format!(
                "pin state entry name is not valid UTF-8: {}",
                entry.display()
            )
        })
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

fn handle_list(cwd: &Path, json: bool, store_root: Option<&Path>) -> anyhow::Result<()> {
    let scope = store::repository_scope(cwd)?;
    let repository_dir = store::root(store_root)?.join(&scope);

    let mut contexts = Vec::new();
    if repository_dir.is_dir() {
        for entry in std::fs::read_dir(&repository_dir)? {
            let context_dir = entry?.path();
            let context_path = context_dir.join("context.md");
            if !context_path.is_file() {
                continue;
            }
            let context = context_dir
                .file_name()
                .and_then(|name| name.to_str())
                .context("context directory name is not valid UTF-8")?
                .to_string();
            let frontmatter = extract_frontmatter_yaml(&context_path).with_context(|| {
                format!(
                    "could not read context metadata at {}",
                    context_path.display()
                )
            })?;
            let metadata: StoredContextMetadata =
                serde_yaml::from_str(&frontmatter).with_context(|| {
                    format!("invalid context metadata at {}", context_path.display())
                })?;
            contexts.push(ContextListEntry {
                context,
                scope: scope.display().to_string(),
                title: metadata.title,
                kind: metadata.kind,
                mode: metadata.mode,
                description: metadata.description,
                created_at: metadata.created_at,
                parent: metadata.parent,
                refs: metadata.refs,
                path: context_path,
            });
        }
    }
    contexts.sort_by(|left, right| left.context.cmp(&right.context));

    if json {
        println!("{}", serde_json::to_string_pretty(&contexts)?);
    } else {
        for context in contexts {
            println!("{}", context.context);
        }
    }
    Ok(())
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
