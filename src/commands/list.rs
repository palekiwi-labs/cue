use crate::cli::MemType;
use crate::config::Config;
use crate::git;
use anyhow::{Context, Result};
use serde::Serialize;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Serialize)]
struct MemFile {
    path: String,
    name: String,
    branch: String,
    category: String,
    hash: Option<String>,
    commit_hash: Option<String>,
    commit_timestamp: u64,
}

pub fn handle(
    cwd: &Path,
    branch_name: Option<String>,
    all: bool,
    mem_type: Option<MemType>,
    include_gitignored: bool,
    json: bool,
) -> Result<()> {
    // 1. Verify git repo
    git::run_git(["rev-parse", "--git-dir"], cwd).context("Not in a git repository")?;

    // 2. Get git root
    let root = git::get_git_root(cwd)?;

    // 3. Load config
    let config = Config::load(&root)?;

    // 4. Check if .mem exists
    let mem_path = root.join(&config.dir_name);
    if !mem_path.is_dir() {
        anyhow::bail!(
            "{} directory does not exist. Run `mem init` first.",
            config.dir_name
        );
    }

    // 5. Determine scan directory/directories
    let mut paths = resolve_scan_paths(&root, &mem_path, all, branch_name)?;

    // 6. Sort
    paths.sort();

    // 7. Filter
    let valid_paths = paths
        .into_iter()
        .filter(|path| is_valid_mem_file(path, &mem_path, mem_type, include_gitignored));

    // 8. Process files and Output
    if !json {
        for path in valid_paths {
            let rel_path = path.strip_prefix(&root).unwrap_or(&path);
            println!("{}", rel_path.display());
        }
    } else {
        let mem_files: Vec<MemFile> = valid_paths
            .filter_map(|path| to_mem_file(&path, &mem_path, &root))
            .collect();
        println!("{}", serde_json::to_string_pretty(&mem_files)?);
    }

    Ok(())
}

fn resolve_scan_paths(
    root: &Path,
    mem_path: &Path,
    all: bool,
    branch_name: Option<String>,
) -> Result<Vec<PathBuf>> {
    if all {
        collect_files(mem_path)
    } else {
        let branch = if let Some(b) = branch_name {
            b
        } else {
            git::get_current_branch(root)?
        };
        let branch_dir = branch.replace(['/', '\\'], "-");
        let scan_dir = mem_path.join(&branch_dir);

        if scan_dir.exists() {
            collect_files(&scan_dir)
        } else {
            Ok(Vec::new())
        }
    }
}

fn is_valid_mem_file(
    path: &Path,
    mem_path: &Path,
    mem_type: Option<MemType>,
    include_gitignored: bool,
) -> bool {
    let Ok(rel_to_mem) = path.strip_prefix(mem_path) else {
        return false;
    };
    let mut components = rel_to_mem.components();

    let _branch = components.next();
    let Some(category_comp) = components.next() else {
        return false;
    };
    let Some(_name_comp) = components.next() else {
        return false; // Ensures len >= 3
    };

    let category = category_comp.as_os_str().to_string_lossy();

    if let Some(requested_type) = mem_type {
        let requested_cat = match requested_type {
            MemType::Spec => "spec",
            MemType::Trace => "trace",
            MemType::Tmp => "tmp",
            MemType::Ref => "ref",
            MemType::Bin => "bin",
            MemType::Doc => "doc",
        };
        if category != requested_cat {
            return false;
        }
    } else if !include_gitignored && (category == "tmp" || category == "ref") {
        return false;
    }

    true
}

fn to_mem_file(path: &Path, mem_path: &Path, root: &Path) -> Option<MemFile> {
    let rel_to_mem = path.strip_prefix(mem_path).ok()?;
    let mut components = rel_to_mem.components();

    let branch = components
        .next()?
        .as_os_str()
        .to_string_lossy()
        .into_owned();
    let category = components
        .next()?
        .as_os_str()
        .to_string_lossy()
        .into_owned();

    let name = path
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_default();

    let rel_path = path
        .strip_prefix(root)
        .unwrap_or(path)
        .to_string_lossy()
        .to_string();

    let mut mem_file = MemFile {
        path: rel_path,
        name: name.clone(),
        branch: branch.clone(),
        category: category.clone(),
        hash: None,
        commit_hash: None,
        commit_timestamp: 0,
    };

    // Trace/Tmp handling for hash/timestamp
    let comp_count = rel_to_mem.components().count();
    if (category == "trace" || category == "tmp") && comp_count >= 4 {
        // Need to iterate again or extract specifically the 3rd component
        let mut comps = rel_to_mem.components();
        comps.next(); // branch
        comps.next(); // category
        if let Some(ts_hash_dir) = comps.next() {
            let ts_hash_str = ts_hash_dir.as_os_str().to_string_lossy();
            if let Some((ts_str, hash_str)) = ts_hash_str.split_once('-')
                && let Ok(ts) = ts_str.parse::<u64>()
            {
                mem_file.commit_timestamp = ts;
                mem_file.hash = Some(hash_str.to_string());
                mem_file.commit_hash = Some(hash_str.to_string());

                // For trace/tmp, name is relative to ts-hash dir
                let prefix = mem_path.join(&branch).join(&category).join(ts_hash_dir);
                if let Ok(rel_name) = path.strip_prefix(&prefix) {
                    mem_file.name = rel_name.to_string_lossy().to_string();
                }
            }
        }
    } else {
        // For spec, bin, ref, name is relative to category dir
        let prefix = mem_path.join(&branch).join(&category);
        if let Ok(rel_name) = path.strip_prefix(&prefix) {
            mem_file.name = rel_name.to_string_lossy().to_string();
        }
    }

    Some(mem_file)
}

fn collect_files(dir: &Path) -> Result<Vec<PathBuf>> {
    if !dir.is_dir() {
        return Ok(vec![]);
    }

    fs::read_dir(dir)?
        .map(|entry| -> Result<Vec<PathBuf>> {
            let path = entry?.path();
            if path.is_dir() {
                collect_files(&path)
            } else {
                Ok(vec![path])
            }
        })
        .collect::<Result<Vec<_>>>()
        .map(|v| v.into_iter().flatten().collect())
}
