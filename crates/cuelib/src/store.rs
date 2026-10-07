use anyhow::{Context, Result};
use std::ffi::OsStr;
use std::path::{Path, PathBuf};

/// Resolve the root of the central cue store.
///
/// Precedence: an explicit `--store` override, then `$CUE_STORE` (if set and
/// non-empty), then `~/cue`.
pub fn root(store_root: Option<&Path>) -> Result<PathBuf> {
    if let Some(path) = store_root {
        return Ok(path.to_path_buf());
    }

    if let Some(path) = std::env::var_os("CUE_STORE").filter(|path| !path.is_empty()) {
        return Ok(PathBuf::from(path));
    }

    dirs::home_dir()
        .map(|home| home.join("cue"))
        .context("Could not determine home directory for cue store")
}

/// Resolve the repository's origin to its `<org>/<repo>` store scope.
pub fn repository_scope(repo: &Path) -> Result<PathBuf> {
    let origin = crate::git::run_git(["remote", "get-url", "origin"], repo)
        .context("Git repository has no origin remote")?;
    parse_origin_scope(&origin)
        .with_context(|| format!("Could not derive repository scope from origin '{origin}'"))
}

fn parse_origin_scope(origin: &str) -> Option<PathBuf> {
    let origin = origin.trim().trim_end_matches('/').trim_end_matches(".git");
    let path = if let Some((_, path)) = origin.rsplit_once(':') {
        path
    } else if let Some((_, path)) = origin.split_once("://") {
        path.split_once('/').map(|(_, path)| path)?
    } else {
        origin
    };
    let mut components = Path::new(path).components().rev();
    let repo = components.next()?.as_os_str();
    let org = components.next()?.as_os_str();
    if repo.is_empty() || org.is_empty() || repo == OsStr::new(".") || org == OsStr::new(".") {
        return None;
    }
    Some(PathBuf::from(org).join(repo))
}
