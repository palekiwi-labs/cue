use crate::config::Config;
use crate::git;
use anyhow::{bail, Context, Result};
use serde_yaml;
use std::fs;
use std::path::{Component, Path};

pub struct AddOptions {
    pub filename: String,
    pub content: Vec<u8>,
    pub frontmatter: Vec<(String, String)>,
    pub cue_type: String,
    pub save_at_root: bool,
    pub force: bool,
    pub branch_name: Option<String>,
}

pub fn handle(cwd: &Path, opts: AddOptions) -> Result<()> {
    let AddOptions {
        filename,
        content,
        frontmatter,
        cue_type,
        save_at_root,
        force,
        branch_name,
    } = opts;
    // 1. Verify git repo
    git::run_git(["rev-parse", "--git-dir"], cwd).context("Not in a git repository")?;

    // 2. Get git root
    let root = git::get_git_root(cwd)?;

    // 3. Load config
    let config = Config::load(&root)?;

    // 4. Check if .cue exists
    let cue_path = root.join(&config.dir_name);
    if !cue_path.exists() {
        bail!(
            "{} directory does not exist. Run `cue init` first.",
            config.dir_name
        );
    }

    // 5. Validate artifact type
    if !config.artifact_types.contains(&cue_type) {
        bail!(
            "Unknown artifact type '{}'. Valid types: {}",
            cue_type,
            config.artifact_types.join(", ")
        );
    }

    // 6. Get branch (handle no-commits case if using current branch)
    let branch = if let Some(b) = branch_name {
        b
    } else {
        git::get_current_branch(&root)
            .context("Could not determine current branch. Have you made your first commit yet?")?
    };
    let branch_dir = branch.replace(['/', '\\'], "-");

    // 7. Resolve destination directory
    let type_dir = cue_path.join(&branch_dir).join(&cue_type);
    let dest_dir = if save_at_root {
        type_dir
    } else {
        let ts = git::get_head_timestamp(&root)?;
        let hash = git::get_short_head_hash(&root)
            .context("Could not determine HEAD hash. Have you made your first commit yet?")?;
        type_dir.join(format!("{}-{}", ts, hash))
    };

    // 7. Validate filename for path traversal
    validate_filename(&filename)?;

    let file_path = dest_dir.join(&filename);

    // 8. Check if exists
    if file_path.exists() && !force {
        bail!(
            "File exists: {}. Use --force to overwrite.",
            file_path.display()
        );
    }

    // 9. Create parent dirs
    if let Some(parent) = file_path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("Failed to create directory {}", parent.display()))?;
    }

    // 10. Assemble final content (prepend frontmatter if provided)
    let final_content = if frontmatter.is_empty() {
        content
    } else {
        let mut fm = build_frontmatter_bytes(&frontmatter)?;
        fm.extend_from_slice(&content);
        fm
    };

    // 11. Write file
    fs::write(&file_path, final_content)
        .with_context(|| format!("Failed to write to {}", file_path.display()))?;

    // 12. Print confirmation
    let rel_path = file_path.strip_prefix(&root).unwrap_or(&file_path);
    eprintln!("✓ Created");
    println!("{}", rel_path.to_string_lossy());

    Ok(())
}

fn build_frontmatter_bytes(fields: &[(String, String)]) -> Result<Vec<u8>> {
    let mut map = serde_yaml::Mapping::new();
    for (k, v) in fields {
        let yaml_val: serde_yaml::Value =
            serde_yaml::from_str(v).unwrap_or_else(|_| serde_yaml::Value::String(v.clone()));
        map.insert(serde_yaml::Value::String(k.clone()), yaml_val);
    }
    let yaml_str =
        serde_yaml::to_string(&map).context("Failed to serialize frontmatter to YAML")?;
    let mut out = b"---\n".to_vec();
    out.extend_from_slice(yaml_str.as_bytes());
    out.extend_from_slice(b"---\n");
    Ok(out)
}

fn validate_filename(filename: &str) -> Result<()> {
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
