use crate::config::Config;
use crate::git;
use anyhow::{Context, Result, bail};
use cuelib::store;
use std::fs;
use std::io::Cursor;
use std::path::{Component, Path, PathBuf};

pub struct AddOptions {
    pub filename: String,
    pub content: Vec<u8>,
    pub frontmatter: Vec<(String, String)>,
    pub cue_type: String,
    pub save_at_root: bool,
    pub force: bool,
    pub scope_name: Option<String>,
}

pub fn add(root: &Path, config: &Config, opts: AddOptions) -> Result<PathBuf> {
    let AddOptions {
        filename,
        content,
        frontmatter,
        cue_type,
        save_at_root,
        force,
        scope_name,
    } = opts;

    // 1. Open store
    let resolved = store::open(root, config)?;

    // 2. Validate artifact type
    if !config.artifact_types.contains(&cue_type) {
        bail!(
            "Unknown artifact type '{}'. Valid types: {}",
            cue_type,
            config.artifact_types.join(", ")
        );
    }

    // 3. Resolve scope (HEAD read from head_dir)
    let scope = cuelib::head::resolve_scope(&resolved.head_dir, scope_name.as_deref())?;
    if scope.trim().is_empty() {
        bail!("Scope name cannot be empty.");
    }

    // 4. Resolve destination directory (artifact write into store_dir)
    let type_dir = resolved.store_dir.join(&scope).join(&cue_type);
    let dest_dir = if save_at_root {
        type_dir
    } else {
        let ts = git::get_head_timestamp(root)?;
        let hash = git::get_short_head_hash(root)
            .context("Could not determine HEAD hash. Have you made your first commit yet?")?;
        type_dir.join(format!("{}-{}", ts, hash))
    };

    // 5. Validate filename for path traversal
    validate_filename(&filename)?;

    // 5b. Normalize markdown filenames: a slug-like filename for a
    // markdown artifact type gets `.md` appended so it satisfies the
    // contract expected by the board reader (`read_artifacts`). A
    // filename is slug-like when it has no extension, or when its
    // extension does not look like a real one (`looks_like_extension`)
    // — `Path::extension` splits at the last dot, so versioned slugs
    // such as `v0.2.0-notes` would otherwise masquerade as extensioned
    // files and stay board-invisible. Genuine payload extensions
    // (`.txt`, `.png`, `.md`) and non-markdown types pass through.
    let has_real_extension = Path::new(&filename)
        .extension()
        .and_then(|ext| ext.to_str())
        .is_some_and(looks_like_extension);
    let filename =
        if !has_real_extension && cuelib::artifact::MARKDOWN_TYPES.contains(&cue_type.as_str()) {
            format!("{filename}.md")
        } else {
            filename
        };

    // 6a. Reject reserved slugs for task cards.
    if cue_type == "task" {
        let stem = Path::new(&filename)
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("");
        if stem == "master" {
            bail!("'master' is a reserved slug and cannot be used as a task filename.");
        }
    }

    let file_path = dest_dir.join(&filename);

    // 7. Check if exists
    if file_path.exists() && !force {
        bail!(
            "File exists: {}. Use --force to overwrite.",
            file_path.display()
        );
    }

    // 8. Create parent dirs
    if let Some(parent) = file_path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("Failed to create directory {}", parent.display()))?;
    }

    // 9. Assemble final content (prepend frontmatter if provided)
    let final_content = if frontmatter.is_empty() {
        content
    } else {
        let mut fm = build_frontmatter_bytes(&frontmatter)?;
        fm.extend_from_slice(&content);
        fm
    };

    // 10. Write file
    fs::write(&file_path, final_content)
        .with_context(|| format!("Failed to write to {}", file_path.display()))?;

    Ok(file_path)
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

/// Serialize frontmatter fields into a `---\n...\n---\n` byte block.
///
/// A key supplied once becomes a scalar; a key repeated two or more times
/// becomes a YAML Sequence of coerced scalars (in encounter order). Keys are
/// emitted in first-seen order (`serde_yaml::Mapping` preserves insertion
/// order). This is field-agnostic: the same rule applies to any key.
pub fn build_frontmatter_bytes(fields: &[(String, String)]) -> Result<Vec<u8>> {
    let mut map = serde_yaml::Mapping::new();
    for (k, v) in fields {
        let key = serde_yaml::Value::String(k.clone());
        let elem = coerce_scalar(v);
        match map.get_mut(&key) {
            None => {
                // First occurrence: store as a scalar. Its slot is fixed here
                // and never moves, so first-seen key order is preserved.
                map.insert(key, elem);
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

/// Maximum length of a dot segment still considered a file extension.
const MAX_EXTENSION_LEN: usize = 8;

/// Returns `true` if `ext` looks like a real file extension rather
/// than the tail of a dotted slug.
///
/// `Path::extension` splits at the last dot, so it happily reports
/// `0-notes` for `v0.2.0-notes` and `2` for `v1.2`. Those are slugs,
/// not filenames, and must still be normalized to `.md`. A dot
/// segment counts as an extension only when it starts with an ASCII
/// letter, is entirely ASCII alphanumeric, and is short.
///
/// Known residual: a slug whose tail happens to look like an
/// extension (`spec.v2`) passes through unnormalized.
fn looks_like_extension(ext: &str) -> bool {
    !ext.is_empty()
        && ext.len() <= MAX_EXTENSION_LEN
        && ext.starts_with(|c: char| c.is_ascii_alphabetic())
        && ext.chars().all(|c| c.is_ascii_alphanumeric())
}

/// Validate a caller-supplied artifact filename.
///
/// Allows only `Normal` path components: subdirectory grouping like
/// `auth-redesign/index.md` is permitted. Rejects empty input,
/// `.`/`..` components, absolute paths, trailing separators
/// (dir-like inputs such as `dir/`, whose `.md`-normalized form
/// would create board-invisible ghost files like `dir/.md`), and
/// trailing dots (`foo.`, which would normalize to `foo..md`).
pub fn validate_filename(filename: &str) -> Result<()> {
    if filename.is_empty() {
        bail!("Invalid filename '{filename}': must not be empty");
    }
    if filename.chars().last().is_some_and(std::path::is_separator) {
        bail!("Invalid filename '{filename}': trailing path separators are not allowed");
    }
    for component in Path::new(filename).components() {
        match component {
            Component::Normal(_) => {}
            Component::CurDir => {
                bail!("Invalid filename '{filename}': '.' is not allowed")
            }
            Component::ParentDir => {
                bail!("Invalid filename '{filename}': '..' is not allowed")
            }
            Component::RootDir | Component::Prefix(_) => {
                bail!("Invalid filename '{filename}': absolute paths are not allowed")
            }
        }
    }
    // Checked after the component scan so `.` and `..` keep their own
    // dedicated messages.
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
    fn real_extensions_are_recognized() {
        for ext in [
            "md", "MD", "txt", "log", "sh", "png", "jpeg", "yaml", "rs", "json",
        ] {
            assert!(
                looks_like_extension(ext),
                "'{ext}' should count as a file extension"
            );
        }
    }

    #[test]
    fn dotted_slug_tails_are_not_extensions() {
        // Tails produced by `Path::extension` on versioned or dated
        // slugs: digit-leading, hyphenated, empty, or implausibly long.
        for ext in [
            "",
            "0-notes",
            "2",
            "30-standup",
            "0",
            "v2-draft",
            "verylongextension",
        ] {
            assert!(
                !looks_like_extension(ext),
                "'{ext}' should not count as a file extension"
            );
        }
    }

    #[test]
    fn extension_of_versioned_slug_is_rejected() {
        // Guards the exact reported case end to end at the predicate
        // level: `v0.2.0-notes` must be treated as extensionless.
        let ext = Path::new("v0.2.0-notes")
            .extension()
            .and_then(|e| e.to_str())
            .expect("Path::extension reports a tail for dotted slugs");
        assert_eq!(ext, "0-notes");
        assert!(!looks_like_extension(ext));
    }
}
