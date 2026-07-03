use crate::config::Config;
use crate::git;
use anyhow::{bail, Context, Result};
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
    pub branch_name: Option<String>,
}

pub fn add(root: &Path, config: &Config, opts: AddOptions) -> Result<PathBuf> {
    let AddOptions {
        filename,
        content,
        frontmatter,
        cue_type,
        save_at_root,
        force,
        branch_name,
    } = opts;

    // 1. Check if .cue exists
    let cue_path = root.join(&config.dir_name);
    if !cue_path.exists() {
        bail!(
            "{} directory does not exist. Run `cue init` first.",
            config.dir_name
        );
    }

    // 2. Validate artifact type
    if !config.artifact_types.contains(&cue_type) {
        bail!(
            "Unknown artifact type '{}'. Valid types: {}",
            cue_type,
            config.artifact_types.join(", ")
        );
    }

    // 3. Get branch
    let branch = if let Some(b) = branch_name {
        b
    } else {
        git::get_current_branch(root)
            .context("Could not determine current branch. Have you made your first commit yet?")?
    };
    if branch.trim().is_empty() {
        bail!("Branch name cannot be empty.");
    }
    let branch_dir = git::sanitize_branch_name(&branch);

    // 4. Resolve destination directory
    let type_dir = cue_path.join(&branch_dir).join(&cue_type);
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

    let file_path = dest_dir.join(&filename);

    // 6. Check if exists
    if file_path.exists() && !force {
        bail!(
            "File exists: {}. Use --force to overwrite.",
            file_path.display()
        );
    }

    // 7. Create parent dirs
    if let Some(parent) = file_path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("Failed to create directory {}", parent.display()))?;
    }

    // 8. Assemble final content (prepend frontmatter if provided)
    let final_content = if frontmatter.is_empty() {
        content
    } else {
        let mut fm = build_frontmatter_bytes(&frontmatter)?;
        fm.extend_from_slice(&content);
        fm
    };

    // 9. Write file
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
                // append, preserving encounter order within the key.
                if let serde_yaml::Value::Sequence(seq) = existing {
                    seq.push(elem);
                } else {
                    let first = std::mem::replace(
                        existing,
                        serde_yaml::Value::Sequence(Vec::with_capacity(2)),
                    );
                    if let serde_yaml::Value::Sequence(seq) = existing {
                        seq.push(first);
                        seq.push(elem);
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

pub fn validate_filename(filename: &str) -> Result<()> {
    for component in Path::new(filename).components() {
        match component {
            Component::Normal(_) | Component::CurDir => {}
            Component::ParentDir => {
                bail!("Invalid filename '{}': '..' is not allowed", filename)
            }
            Component::RootDir | Component::Prefix(_) => {
                bail!(
                    "Invalid filename '{}': absolute paths are not allowed",
                    filename
                )
            }
        }
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
