use crate::address;
use crate::git;
use anyhow::{Context, Result, bail};
use cuelib::store;
use std::fs;
use std::io::Cursor;
use std::path::{Component, Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

pub struct AddOptions {
    pub filename: String,
    pub content: Vec<u8>,
    pub frontmatter: Vec<(String, String)>,
    pub cue_type: String,
    pub force: bool,
    pub scope_name: Option<String>,
    pub store_root: Option<PathBuf>,
}

/// Shared inputs locating a central artifact write: the repository, the
/// requested file, and the store-root/context resolution inputs.
struct CentralWrite<'a> {
    root: &'a Path,
    filename: &'a str,
    content: &'a [u8],
    force: bool,
    context: Option<&'a str>,
    store_root: Option<&'a Path>,
}

pub fn add(root: &Path, opts: AddOptions) -> Result<PathBuf> {
    let AddOptions {
        filename,
        content,
        frontmatter,
        cue_type,
        force,
        scope_name,
        store_root,
    } = opts;

    let write = CentralWrite {
        root,
        filename: &filename,
        content: &content,
        force,
        context: scope_name.as_deref(),
        store_root: store_root.as_deref(),
    };

    if matches!(
        cue_type.as_str(),
        "task" | "spec" | "plan" | "note" | "trace"
    ) {
        return add_central_markdown(write, frontmatter, &cue_type);
    }
    if cue_type == "bin" {
        return add_central_bin(write, frontmatter);
    }
    if cue_type == "tmp" {
        return add_central_tmp(write, frontmatter);
    }
    unreachable!("artifact types are constrained by clap")
}

fn add_central_markdown(
    write: CentralWrite<'_>,
    mut frontmatter: Vec<(String, String)>,
    cue_type: &str,
) -> Result<PathBuf> {
    let CentralWrite {
        root,
        filename,
        content,
        force,
        context,
        store_root,
    } = write;
    validate_filename(filename)?;
    validate_reference_fields(&frontmatter, &store::root(store_root)?)?;

    let context_dir = central_context_dir(root, context, store_root)?;

    // A task is the only artifact that can be done, so it is the only type
    // given lifecycle defaults. A new task is untriaged (`inbox`) and
    // unranked (`normal`) until an operator decides otherwise; stamping both
    // keeps every task filterable on status and priority without forcing a
    // caller to supply them.
    if cue_type == "task" {
        for (key, default) in [("status", "inbox"), ("priority", "normal")] {
            if !frontmatter.iter().any(|(existing, _)| existing == key) {
                frontmatter.push((key.into(), default.into()));
            }
        }
    }
    // A trace is an artifact *about* a revision, so it is the only markdown
    // type carrying revision correlation. Both fields are stamped from the
    // current repository, but an explicit value wins: a coordination context
    // records evidence about a revision of some other repository.
    if cue_type == "trace" {
        if !frontmatter.iter().any(|(key, _)| key == "repo_id") {
            let scope = store::repository_scope(root)?;
            frontmatter.push(("repo_id".into(), scope.to_string_lossy().into_owned()));
        }
        if !frontmatter.iter().any(|(key, _)| key == "commit_hash") {
            let hash = git::get_short_head_hash(root)
                .context("Could not determine HEAD hash. Have you made your first commit yet?")?;
            frontmatter.push(("commit_hash".into(), hash));
        }
        // Hashes made only of decimal digits must remain strings rather than
        // being coerced into YAML numbers by the field-agnostic encoder.
        if let Some((_, hash)) = frontmatter.iter_mut().find(|(key, _)| key == "commit_hash") {
            *hash = format!("'{}'", hash.replace('\'', "''"));
        }
    }
    if !frontmatter.iter().any(|(key, _)| key == "created_at") {
        let created_at = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs();
        frontmatter.push(("created_at".into(), created_at.to_string()));
    }

    let filename = if Path::new(filename).extension().is_none() {
        format!("{filename}.md")
    } else {
        filename.to_string()
    };
    let file_path = context_dir.join(cue_type).join(filename);
    write_new_file(&file_path, force, || {
        let mut final_content = build_frontmatter_bytes(&frontmatter)?;
        final_content.extend_from_slice(content);
        Ok(final_content)
    })?;

    Ok(file_path)
}

fn add_central_bin(write: CentralWrite<'_>, metadata: Vec<(String, String)>) -> Result<PathBuf> {
    let CentralWrite {
        root,
        filename,
        content,
        force,
        context,
        store_root,
    } = write;
    validate_filename(filename)?;
    let context_dir = central_context_dir(root, context, store_root)?;
    let filename = if Path::new(filename).extension().is_none() {
        format!("{filename}.json")
    } else {
        filename.to_string()
    };
    let file_path = context_dir.join("bin").join(filename);

    write_new_file(&file_path, force, || {
        let value: serde_json::Value =
            serde_json::from_slice(content).context("Bin content must be valid JSON")?;
        let serde_json::Value::Object(mut object) = value else {
            bail!("Bin content must be a JSON object");
        };
        for (key, raw_value) in metadata {
            let value = serde_json::to_value(coerce_scalar(&raw_value))?;
            match object.get_mut(&key) {
                Some(serde_json::Value::Array(values)) => values.push(value),
                Some(existing) => {
                    let first = std::mem::take(existing);
                    *existing = serde_json::Value::Array(vec![first, value]);
                }
                None => {
                    object.insert(key, value);
                }
            }
        }
        let mut bytes = serde_json::to_vec_pretty(&object)?;
        bytes.push(b'\n');
        Ok(bytes)
    })?;

    Ok(file_path)
}

fn add_central_tmp(write: CentralWrite<'_>, metadata: Vec<(String, String)>) -> Result<PathBuf> {
    let CentralWrite {
        root,
        filename,
        content,
        force,
        context,
        store_root,
    } = write;
    if !metadata.is_empty() {
        bail!("tmp artifacts do not support metadata");
    }
    validate_filename(filename)?;

    let context_dir = central_context_dir(root, context, store_root)?;
    let commit_hash = git::get_short_head_hash(root)
        .context("Could not determine HEAD hash. Have you made your first commit yet?")?;
    let tmp_dir = context_dir.join("tmp");
    fs::create_dir_all(&tmp_dir)?;
    let mut created_at = SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos();
    let artifact_dir = loop {
        let candidate = tmp_dir.join(format!("{created_at}-{commit_hash}"));
        match fs::create_dir(&candidate) {
            Ok(()) => break candidate,
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
                created_at += 1;
            }
            Err(error) => {
                return Err(error)
                    .with_context(|| format!("Failed to create {}", candidate.display()));
            }
        }
    };
    let file_path = artifact_dir.join(filename);
    write_new_file(&file_path, force, || Ok(content.to_vec()))?;

    Ok(file_path)
}

fn central_context_dir(
    root: &Path,
    context: Option<&str>,
    store_root: Option<&Path>,
) -> Result<PathBuf> {
    let context = cuelib::head::resolve_active_context(root, context)?
        .context("No context selected; pass --context <context>")?;
    let repository_dir = store::root(store_root)?.join(store::repository_scope(root)?);
    let context_dir = repository_dir.join(&context);
    if !context_dir.join("context.md").is_file() {
        bail!("Context does not exist: {context}");
    }
    Ok(context_dir)
}

fn write_new_file<F>(file_path: &Path, force: bool, content: F) -> Result<()>
where
    F: FnOnce() -> Result<Vec<u8>>,
{
    if file_path.exists() && !force {
        bail!(
            "File exists: {}. Use --force to overwrite.",
            file_path.display()
        );
    }

    fs::create_dir_all(file_path.parent().expect("artifact path has a parent"))?;
    fs::write(file_path, content()?)
        .with_context(|| format!("Failed to write to {}", file_path.display()))?;
    Ok(())
}

/// Coerce a raw frontmatter string into a YAML scalar value.
///
/// Booleans, integers, and floats are recognized so they serialize unquoted
/// (e.g. `count=3` -> `count: 3`). Any value that would parse as a YAML
/// collection (Mapping/Sequence/Tagged) or as YAML `null` (the tokens `null`,
/// `~`, `Null`, `NULL`, a comment-only `#...`, or whitespace-only input) is
/// forced back to a plain string so that values like `title=foo: bar` and
/// `status=null` round-trip as quoted scalars instead of being re-interpreted
/// as structure or as an absent value. An empty value yields the empty string,
/// not YAML `null`.
fn coerce_scalar(v: &str) -> serde_yaml::Value {
    if v.is_empty() {
        return serde_yaml::Value::String(String::new());
    }
    match serde_yaml::from_str::<serde_yaml::Value>(v) {
        Ok(serde_yaml::Value::Mapping(_))
        | Ok(serde_yaml::Value::Sequence(_))
        | Ok(serde_yaml::Value::Tagged(_))
        | Ok(serde_yaml::Value::Null) => serde_yaml::Value::String(v.to_string()),
        Ok(val) => val,
        Err(_) => serde_yaml::Value::String(v.to_string()),
    }
}

/// Structural fields whose value is a list by definition, regardless of how
/// many values a caller supplied. `refs` is the only one: it names zero or
/// more canonical addresses, so a single entry is a one-element list rather
/// than a scalar, and a reader never has to handle two shapes.
const LIST_VALUED_FIELDS: [&str; 1] = ["refs"];

/// Structural fields that name at most one canonical address, so supplying
/// them more than once is a caller error rather than a promotion to a list.
const SINGLE_VALUED_REFERENCE_FIELDS: [&str; 1] = ["parent"];

/// Validate the structural reference fields carried by an artifact.
///
/// `parent` and `refs` are structural: cue understands them, so it checks
/// their shape and arity. This is not an exception to field-agnostic
/// conventional-metadata encoding; it is what "structural" means.
fn validate_reference_fields(fields: &[(String, String)], store_root: &Path) -> Result<()> {
    for field in SINGLE_VALUED_REFERENCE_FIELDS {
        if fields.iter().filter(|(key, _)| key == field).count() > 1 {
            bail!("Invalid {field}: {field} must be supplied at most once");
        }
    }
    for (key, value) in fields {
        if SINGLE_VALUED_REFERENCE_FIELDS.contains(&key.as_str())
            || LIST_VALUED_FIELDS.contains(&key.as_str())
        {
            address::validate_reference(key, value, store_root)?;
        }
    }
    Ok(())
}

/// Serialize frontmatter fields into a `---\n...\n---\n` byte block.
///
/// A key supplied once becomes a scalar; a key repeated two or more times
/// becomes a YAML Sequence of coerced scalars (in encounter order). Keys are
/// emitted in first-seen order (`serde_yaml::Mapping` preserves insertion
/// order). This is field-agnostic for conventional metadata: the same rule
/// applies to any key cue does not understand. The structural fields in
/// `LIST_VALUED_FIELDS` are the exception, and always serialize as a
/// Sequence.
pub fn build_frontmatter_bytes(fields: &[(String, String)]) -> Result<Vec<u8>> {
    let mut map = serde_yaml::Mapping::new();
    for (k, v) in fields {
        let key = serde_yaml::Value::String(k.clone());
        let elem = coerce_scalar(v);
        match map.get_mut(&key) {
            None => {
                // First occurrence: store as a scalar, unless the field is a
                // list by definition. Its slot is fixed here and never moves,
                // so first-seen key order is preserved.
                if LIST_VALUED_FIELDS.contains(&k.as_str()) {
                    map.insert(key, serde_yaml::Value::Sequence(vec![elem]));
                } else {
                    map.insert(key, elem);
                }
            }
            Some(existing) => {
                // Second+ occurrence: promote the scalar to a Sequence and
                // append, preserving encounter order within the key. We mutate
                // in place through the existing reference so the key's slot
                // (and thus first-seen order) never moves. The match is total:
                // each arm fully handles its case, so there is no fallible or
                // data-losing branch.
                match existing {
                    serde_yaml::Value::Sequence(seq) => seq.push(elem),
                    other => {
                        // Move the scalar out (leaving the default Null) and
                        // rebuild the slot as a fresh two-element Sequence.
                        let first = std::mem::take(other);
                        *other = serde_yaml::Value::Sequence(vec![first, elem]);
                    }
                }
            }
        }
    }
    let yaml_str =
        serde_yaml::to_string(&map).context("Failed to serialize frontmatter to YAML")?;
    let mut out = b"---\n".to_vec();
    out.extend_from_slice(yaml_str.as_bytes());
    out.extend_from_slice(b"---\n");
    Ok(out)
}

/// Validate a caller-supplied artifact filename.
///
/// Allows only `Normal` path components: subdirectory grouping like
/// `auth-redesign/index.md` is permitted. Rejects empty or whitespace-only
/// input, `.`/`..` components, leading `..` component prefixes, absolute
/// paths, trailing separators (dir-like inputs such as `dir/`, whose
/// `.md`-normalized form would create board-invisible ghost files like
/// `dir/.md`), and trailing dots (`foo.`, which would normalize to `foo..md`).
pub fn validate_filename(filename: &str) -> Result<()> {
    if filename.trim().is_empty() {
        bail!("Invalid filename '{filename}': must not be empty");
    }
    for component in Path::new(filename).components() {
        match component {
            Component::Normal(c) => {
                if c.to_str().is_some_and(|s| s.trim().is_empty()) {
                    bail!("Invalid filename '{filename}': must not be empty");
                }
                if c.to_str().is_some_and(|s| s.starts_with("..")) {
                    bail!("Invalid filename '{filename}': '..' is not allowed");
                }
            }
            Component::CurDir => {
                bail!("Invalid filename '{filename}': '.' is not allowed");
            }
            Component::ParentDir => {
                bail!("Invalid filename '{filename}': '..' is not allowed");
            }
            Component::RootDir | Component::Prefix(_) => {
                bail!("Invalid filename '{filename}': absolute paths are not allowed");
            }
        }
    }
    // Suffix checks run after the component scan so that root dirs (`/`),
    // current dirs (`.`, `./`), and parent dirs (`..`, `../`) retain their
    // dedicated error messages rather than being masked by trailing separator
    // or trailing dot checks.
    if filename.chars().last().is_some_and(std::path::is_separator) {
        bail!("Invalid filename '{filename}': trailing path separators are not allowed");
    }
    if filename.ends_with('.') {
        bail!("Invalid filename '{filename}': trailing dots are not allowed");
    }
    Ok(())
}

pub fn resolve_clipboard(filename: &str) -> anyhow::Result<Vec<u8>> {
    use arboard::Clipboard;
    use image::{ImageBuffer, ImageFormat, RgbaImage};

    let lower_filename = filename.to_lowercase();
    let is_png = lower_filename.ends_with(".png");
    let is_jpg = lower_filename.ends_with(".jpg") || lower_filename.ends_with(".jpeg");

    // Check for other image formats we don't support yet
    let other_image = [".webp", ".gif", ".bmp", ".tiff", ".tga"];
    if other_image.iter().any(|ext| lower_filename.ends_with(ext)) {
        anyhow::bail!(
            "Unsupported image format in filename '{}'. Supported formats: .png, .jpg, .jpeg",
            filename
        );
    }

    let mut ctx = Clipboard::new().context(
        "Failed to access clipboard. Ensure a display server (X11 or Wayland) is running.",
    )?;

    if is_png || is_jpg {
        let img_data = ctx
            .get_image()
            .context("Clipboard does not contain an image.")?;
        let img: RgbaImage = ImageBuffer::from_raw(
            img_data.width as u32,
            img_data.height as u32,
            img_data.bytes.into_owned(),
        )
        .context("Invalid image data in clipboard")?;

        let mut buf = Vec::new();
        let format = if is_png {
            ImageFormat::Png
        } else {
            ImageFormat::Jpeg
        };
        img.write_to(&mut Cursor::new(&mut buf), format)
            .context("Failed to encode image")?;
        Ok(buf)
    } else {
        // Assume text for any other extension
        let text = ctx.get_text().context("Clipboard does not contain text.")?;
        Ok(text.into_bytes())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validate_filename_accepts_valid_inputs() {
        for valid in [
            "card",
            "card.md",
            "auth-login",
            "auth-login.md",
            "v0.2.0-notes",
            "nested/card",
            "nested/card.md",
            "deeply/nested/dir/artifact.txt",
            ".hidden",
            ".hidden.md",
            "name with spaces",
            "name with spaces.md",
        ] {
            assert!(
                validate_filename(valid).is_ok(),
                "'{valid}' should be valid"
            );
        }
    }

    #[test]
    fn validate_filename_rejects_empty_and_whitespace() {
        for input in ["", " ", "   ", "\t", "\n", "dir/ ", " /file.md"] {
            let err = validate_filename(input).unwrap_err().to_string();
            assert!(
                err.contains("must not be empty"),
                "input '{input}' should fail with empty message, got: {err}"
            );
        }
    }

    #[test]
    fn validate_filename_rejects_curdir_dot() {
        for input in [".", "./", "./foo"] {
            let err = validate_filename(input).unwrap_err().to_string();
            assert!(
                err.contains("'.' is not allowed"),
                "input '{input}' should fail with '.' message, got: {err}"
            );
        }
    }

    #[test]
    fn validate_filename_rejects_parentdir_and_double_dot_prefix() {
        for input in [
            "..",
            "../",
            "../foo",
            "foo/..",
            "foo/../bar",
            "..foo",
            "dir/..foo",
            "...",
            "....bar",
        ] {
            let err = validate_filename(input).unwrap_err().to_string();
            assert!(
                err.contains("'..' is not allowed"),
                "input '{input}' should fail with '..' message, got: {err}"
            );
        }
    }

    #[test]
    fn validate_filename_rejects_absolute_paths() {
        for input in ["/", "///", "/abs", "/etc/passwd", "/dir/"] {
            let err = validate_filename(input).unwrap_err().to_string();
            assert!(
                err.contains("absolute paths are not allowed"),
                "input '{input}' should fail with absolute path message, got: {err}"
            );
        }
    }

    #[test]
    fn validate_filename_rejects_trailing_separators() {
        for input in ["dir/", "dir//", "nested/dir/", "a/b/c/"] {
            let err = validate_filename(input).unwrap_err().to_string();
            assert!(
                err.contains("trailing path separators are not allowed"),
                "input '{input}' should fail with trailing separator message, got: {err}"
            );
        }
    }

    #[test]
    fn validate_filename_rejects_trailing_dots() {
        for input in ["trailing.", "foo.", "nested/dir."] {
            let err = validate_filename(input).unwrap_err().to_string();
            assert!(
                err.contains("trailing dots are not allowed"),
                "input '{input}' should fail with trailing dot message, got: {err}"
            );
        }
    }
}
