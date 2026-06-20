use crate::cli::ProjectCommands;
use anyhow::Result;
use cuelib::project::{derive_project_key, ProjectStore};
use std::path::{Path, PathBuf};

pub fn handle(cwd: &Path, command: ProjectCommands) -> Result<()> {
    match command {
        ProjectCommands::Add { path } => {
            let target = resolve_path(cwd, path.as_deref());
            let key = derive_project_key(&target);

            let mut store = ProjectStore::load()?;
            let paths = store.paths_for(&key).to_vec();
            if paths.contains(&target) {
                println!("already registered: {} -> {}", key, target.display());
            } else {
                store.add_path(&key, &target);
                store.save()?;
                println!("Registered: {} -> {}", key, target.display());
            }
        }

        ProjectCommands::Remove { path, key } => {
            let mut store = ProjectStore::load()?;

            if let Some(k) = key {
                if store.remove_key(&k) {
                    store.save()?;
                    println!("Removed key: {}", k);
                } else {
                    println!("Key not found: {}", k);
                }
            } else {
                let target = resolve_path(cwd, path.as_deref());
                let key = derive_project_key(&target);
                if store.remove_path(&key, &target) {
                    store.save()?;
                    println!("Removed: {} -> {}", key, target.display());
                } else {
                    println!("Path not registered: {}", target.display());
                }
            }
        }

        ProjectCommands::List => {
            let store = ProjectStore::load()?;
            for (key, paths) in store.entries() {
                for path in paths {
                    println!("{}  {}", key, path.display());
                }
            }
        }
    }

    Ok(())
}

fn resolve_path(cwd: &Path, explicit: Option<&str>) -> PathBuf {
    match explicit {
        Some(p) => {
            let p = PathBuf::from(p);
            if p.is_absolute() {
                p
            } else {
                cwd.join(p)
            }
        }
        None => cwd.to_path_buf(),
    }
}
