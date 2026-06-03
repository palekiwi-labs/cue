use crate::config::Config;
use crate::git;
use anyhow::{bail, Context, Result};
use std::fs;
use std::path::{Component, Path};

pub fn handle(
    cwd: &Path,
    filename: &str,
    content: Vec<u8>,
    mem_type: String,
    save_at_root: bool,
    force: bool,
    branch_name: Option<String>,
) -> Result<()> {
    // 1. Verify git repo
    git::run_git(["rev-parse", "--git-dir"], cwd).context("Not in a git repository")?;

    // 2. Get git root
    let root = git::get_git_root(cwd)?;

    // 3. Load config
    let config = Config::load(&root)?;

    // 4. Check if .mem exists
    let mem_path = root.join(&config.dir_name);
    if !mem_path.exists() {
        bail!(
            "{} directory does not exist. Run `mem init` first.",
            config.dir_name
        );
    }

    // 5. Validate artifact type
    if !config.artifact_types.contains(&mem_type) {
        bail!(
            "Unknown artifact type '{}'. Valid types: {}",
            mem_type,
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
    let type_dir = mem_path.join(&branch_dir).join(&mem_type);
    let dest_dir = if save_at_root {
        type_dir
    } else {
        let ts = git::get_head_timestamp(&root)?;
        let hash = git::get_short_head_hash(&root)
            .context("Could not determine HEAD hash. Have you made your first commit yet?")?;
        type_dir.join(format!("{}-{}", ts, hash))
    };

    // 7. Validate filename for path traversal
    validate_filename(filename)?;

    let file_path = dest_dir.join(filename);

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

    // 10. Write file
    fs::write(&file_path, content)
        .with_context(|| format!("Failed to write to {}", file_path.display()))?;

    // 11. Print confirmation
    let rel_path = file_path.strip_prefix(&root).unwrap_or(&file_path);
    eprintln!("✓ Created");
    println!("{}", rel_path.to_string_lossy());

    Ok(())
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
