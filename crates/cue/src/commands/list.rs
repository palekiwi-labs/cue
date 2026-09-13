pub use crate::list::ListOptions;

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

    // 2. Resolve the repository's directory in the central store.
    let store_dir = store::root(opts.store_root.as_deref())?.join(store::repository_scope(cwd)?);

    // 3. Delegate to domain module
    let filtered = list::list(cwd, opts)?;

    // 4. Output
    if !json_output {
        for (path, _) in filtered {
            println!("{}", path.display());
        }
    } else {
        let cue_files: Vec<list::CueFile> = filtered
            .into_iter()
            .filter_map(|(path, cached_fm)| {
                let mut mf = list::to_cue_file(&path, &store_dir)?;
                if include_frontmatter {
                    mf.frontmatter = cached_fm.filter(|v| !v.is_null());
                }
                Some(mf)
            })
            .collect();
        println!("{}", serde_json::to_string_pretty(&cue_files)?);
    }

    Ok(())
}
