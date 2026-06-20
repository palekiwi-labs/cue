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
    let branch_dir = branch.replace(['/', '\\'], "-");

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

pub fn build_frontmatter_bytes(fields: &[(String, String)]) -> Result<Vec<u8>> {
    let mut map = serde_yaml::Mapping::new();
    for (k, v) in fields {
        // Parse the raw string to allow scalar type coercion (bool, int,
        // float). However, if the parse yields a collection type (Mapping,
        // Sequence, or Tagged) the user supplied a plain string that happens
        // to contain YAML collection syntax. Force it back to a String so
        // that serde_yaml will emit it as a properly quoted scalar.
        let yaml_val: serde_yaml::Value = match serde_yaml::from_str(v) {
            Ok(serde_yaml::Value::Mapping(_))
            | Ok(serde_yaml::Value::Sequence(_))
            | Ok(serde_yaml::Value::Tagged(_)) => serde_yaml::Value::String(v.clone()),
            Ok(val) => val,
            Err(_) => serde_yaml::Value::String(v.clone()),
        };
        map.insert(serde_yaml::Value::String(k.clone()), yaml_val);
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
