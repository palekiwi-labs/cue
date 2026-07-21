pub use crate::list::ListOptions;

use crate::config::Config;
use crate::git;
use crate::list;
use anyhow::{Context, Result};
use cuelib::store;
use std::path::Path;

pub fn handle(cwd: &Path, opts: ListOptions) -> Result<()> {
    let json_output = opts.json || opts.frontmatter;
    let include_frontmatter = opts.frontmatter;

    // 1. Verify git repo
    git::run_git(["rev-parse", "--git-dir"], cwd).context("Not in a git repository")?;

    // 2. Get git root
    let root = git::get_git_root(cwd)?;

    // 3. Load config
    let config = Config::load(&root)?;

    // 4. Resolve store so to_cue_file can use store_dir
    let cue_dir = root.join(&config.dir_name);
    let resolved = store::resolve_store(cue_dir)?;

    // 5. Delegate to domain module
    let filtered = list::list(&root, &config, opts)?;

    // 6. Output
    if !json_output {
        for (path, _) in filtered {
            let rel_path = path
                .strip_prefix(&resolved.store_dir)
                .or_else(|_| path.strip_prefix(&root))
                .unwrap_or(&path);
            println!("{}", rel_path.display());
        }
    } else {
        let cue_files: Vec<list::CueFile> = filtered
            .into_iter()
            .filter_map(|(path, cached_fm)| {
                let mut mf = list::to_cue_file(&path, &resolved.store_dir, &root)?;
                if include_frontmatter {
                    mf.frontmatter =
                        cached_fm.and_then(|v| if v.is_null() { None } else { Some(v) });
                }
                Some(mf)
            })
            .collect();
        println!("{}", serde_json::to_string_pretty(&cue_files)?);
    }

    Ok(())
}
