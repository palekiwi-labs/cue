use crate::config::Config;
use crate::git::{get_git_root, list_worktrees};
use anyhow::{bail, Context, Result};
use std::path::{Path, PathBuf};

/// The result of resolving a cue store directory.
///
/// `head_dir` is the local checkout's `.cue/` directory (where HEAD is read
/// from and written to). `store_dir` is the store-owning git root's `.cue/`
/// directory (where artifacts and task directories live).
#[derive(Debug)]
pub struct ResolvedStore {
    /// Directory to read/write HEAD from. Always the local `.cue/`.
    pub head_dir: PathBuf,
    /// Directory to read/write artifacts from.
    pub store_dir: PathBuf,
}

/// Resolve the store-owning git root for a repository containing `start`.
///
/// The git root is the **main worktree**: the first entry of
/// `git worktree list --porcelain`, normalized through
/// `git rev-parse --show-toplevel`. The normalization is idempotent for
/// normal repositories, resolves a submodule to its own toplevel (no
/// inheritance from the parent repository), and fails loudly for a bare
/// main worktree.
///
/// Deliberately NOT `git rev-parse --git-common-dir`: that is
/// cwd-relative and unreliable for submodules and
/// `--separate-git-dir` checkouts.
pub fn git_root(start: &Path) -> Result<PathBuf> {
    let worktrees = list_worktrees(start)
        .with_context(|| format!("Failed to list git worktrees from {}", start.display()))?;
    let entry0 = worktrees.first().ok_or_else(|| {
        anyhow::anyhow!(
            "`git worktree list` returned no worktrees for {}",
            start.display()
        )
    })?;
    get_git_root(entry0).with_context(|| {
        format!(
            "Failed to resolve main worktree {} to a toplevel",
            entry0.display()
        )
    })
}

/// Open the cue store for the repository containing `root`.
///
/// `root` may be any directory inside a worktree of the repository
/// (usually the current working directory). The store is always the
/// `<git-root>/<dir_name>` directory, where the git root is the **main
/// worktree** (see [`git_root`]); `head_dir` stays the local checkout's
/// `<toplevel>/<dir_name>`.
///
/// The caller should load `config` from [`git_root`] (the store owner),
/// not from the current worktree.
pub fn open(root: &Path, config: &Config) -> Result<ResolvedStore> {
    let store_dir = git_root(root)?.join(&config.dir_name);
    if !store_dir.join("master").is_dir() {
        bail!(
            "no cue store at {} (missing `master/`); \
             run `cue init` in the main repository to create it",
            store_dir.display()
        );
    }
    let head_dir = get_git_root(root)?.join(&config.dir_name);
    Ok(ResolvedStore {
        head_dir,
        store_dir,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    // Helper: create a minimal valid cue store (contains master/ subdir).
    fn make_store(path: &Path) {
        fs::create_dir_all(path.join("master")).unwrap();
    }

    // Helper: run git in `dir`, panicking on failure.
    fn git(dir: &Path, args: &[&str]) {
        let out = std::process::Command::new("git")
            .args(args)
            .current_dir(dir)
            .output()
            .expect("failed to spawn git");
        assert!(
            out.status.success(),
            "git {:?} in {} failed: {}",
            args,
            dir.display(),
            String::from_utf8_lossy(&out.stderr)
        );
    }

    // Helper: init a normal git repo with one commit.
    fn init_repo(dir: &Path) {
        fs::create_dir_all(dir).unwrap();
        git(dir, &["init", "-b", "main"]);
        git(dir, &["config", "user.email", "test@example.com"]);
        git(dir, &["config", "user.name", "Test"]);
        git(dir, &["config", "commit.gpgsign", "false"]);
        fs::write(dir.join("initial.txt"), "hello").unwrap();
        git(dir, &["add", "."]);
        git(dir, &["commit", "-m", "initial"]);
    }

    #[test]
    fn open_plain_repo_resolves_root_cue_dir() {
        let tmp = tempdir().unwrap();
        let main = tmp.path().join("main");
        init_repo(&main);
        make_store(&main.join(".cue"));

        let resolved = open(&main, &Config::default()).unwrap();

        let root = get_git_root(&main).unwrap();
        assert_eq!(resolved.head_dir, root.join(".cue"));
        assert_eq!(resolved.store_dir, root.join(".cue"));
    }

    #[test]
    fn open_missing_master_errors_with_init_hint() {
        let tmp = tempdir().unwrap();
        let main = tmp.path().join("main");
        init_repo(&main);

        let err = open(&main, &Config::default()).unwrap_err();

        let msg = format!("{err:#}");
        assert!(msg.contains("cue init"), "unexpected error: {msg}");
        let root = get_git_root(&main).unwrap();
        assert!(
            msg.contains(&root.join(".cue").display().to_string()),
            "unexpected error: {msg}"
        );
    }

    #[test]
    fn open_worktree_splits_head_and_store_dirs() {
        let tmp = tempdir().unwrap();
        let main = tmp.path().join("main");
        let wt = tmp.path().join("wt");
        init_repo(&main);
        make_store(&main.join(".cue"));
        git(
            &main,
            &["worktree", "add", wt.to_str().unwrap(), "-b", "topic"],
        );

        let resolved = open(&wt, &Config::default()).unwrap();

        // Store dir is the main worktree's .cue; head dir is the local one.
        let main_root = get_git_root(&main).unwrap();
        let wt_root = get_git_root(&wt).unwrap();
        assert_eq!(resolved.store_dir, main_root.join(".cue"));
        assert_eq!(resolved.head_dir, wt_root.join(".cue"));
    }

    #[test]
    fn open_bare_main_worktree_fails_loudly() {
        let tmp = tempdir().unwrap();
        let src = tmp.path().join("src");
        init_repo(&src);
        let bare = tmp.path().join("bare.git");
        git(
            &src,
            &[
                "clone",
                "--bare",
                src.to_str().unwrap(),
                bare.to_str().unwrap(),
            ],
        );
        let wt = tmp.path().join("wt");
        git(
            &bare,
            &["worktree", "add", wt.to_str().unwrap(), "-b", "topic"],
        );

        let err = open(&wt, &Config::default()).unwrap_err();

        let msg = format!("{err:#}");
        assert!(
            msg.contains("Failed to resolve main worktree"),
            "unexpected error: {msg}"
        );
        assert!(msg.contains("bare.git"), "unexpected error: {msg}");
    }

    #[test]
    fn open_submodule_resolves_own_toplevel_only() {
        let tmp = tempdir().unwrap();
        let sub_src = tmp.path().join("subsrc");
        init_repo(&sub_src);
        let parent = tmp.path().join("parent");
        init_repo(&parent);
        // Parent has a store; the submodule must NOT inherit it.
        make_store(&parent.join(".cue"));
        // git >= 2.38.1 denies the file transport for submodule clones;
        // repo-local config is ignored for this security setting, so it
        // must be passed inline.
        git(
            &parent,
            &[
                "-c",
                "protocol.file.allow=always",
                "submodule",
                "add",
                sub_src.to_str().unwrap(),
                "sub",
            ],
        );
        git(&parent, &["commit", "-m", "add submodule"]);
        let sub = parent.join("sub");
        make_store(&sub.join(".cue"));

        let resolved = open(&sub, &Config::default()).unwrap();

        let sub_root = get_git_root(&sub).unwrap();
        assert_eq!(resolved.store_dir, sub_root.join(".cue"));
        assert_eq!(resolved.head_dir, sub_root.join(".cue"));
    }

    #[test]
    fn open_ignores_stray_local_store_in_worktree() {
        let tmp = tempdir().unwrap();
        let main = tmp.path().join("main");
        let wt = tmp.path().join("wt");
        init_repo(&main);
        make_store(&main.join(".cue"));
        git(
            &main,
            &["worktree", "add", wt.to_str().unwrap(), "-b", "topic"],
        );
        // Stray local store content in the worktree must not win.
        make_store(&wt.join(".cue"));

        let resolved = open(&wt, &Config::default()).unwrap();

        let main_root = get_git_root(&main).unwrap();
        let wt_root = get_git_root(&wt).unwrap();
        assert_eq!(resolved.store_dir, main_root.join(".cue"));
        assert_ne!(resolved.store_dir, wt_root.join(".cue"));
        assert_eq!(resolved.head_dir, wt_root.join(".cue"));
    }

    #[test]
    fn open_stray_local_master_does_not_satisfy_store() {
        let tmp = tempdir().unwrap();
        let main = tmp.path().join("main");
        let wt = tmp.path().join("wt");
        init_repo(&main);
        // Main has no store; only the worktree has a stray .cue/master.
        git(
            &main,
            &["worktree", "add", wt.to_str().unwrap(), "-b", "topic"],
        );
        make_store(&wt.join(".cue"));

        let err = open(&wt, &Config::default()).unwrap_err();

        let msg = format!("{err:#}");
        assert!(msg.contains("cue init"), "unexpected error: {msg}");
    }

    #[test]
    fn open_honors_custom_dir_name_at_git_root() {
        let tmp = tempdir().unwrap();
        let main = tmp.path().join("main");
        let wt = tmp.path().join("wt");
        init_repo(&main);
        make_store(&main.join(".memory"));
        git(
            &main,
            &["worktree", "add", wt.to_str().unwrap(), "-b", "topic"],
        );

        let config = Config {
            dir_name: ".memory".into(),
            ..Config::default()
        };
        let resolved = open(&wt, &config).unwrap();

        let main_root = get_git_root(&main).unwrap();
        let wt_root = get_git_root(&wt).unwrap();
        assert_eq!(resolved.store_dir, main_root.join(".memory"));
        assert_eq!(resolved.head_dir, wt_root.join(".memory"));
    }

    #[test]
    fn git_root_returns_main_worktree_from_linked_worktree() {
        let tmp = tempdir().unwrap();
        let main = tmp.path().join("main");
        let wt = tmp.path().join("wt");
        init_repo(&main);
        git(
            &main,
            &["worktree", "add", wt.to_str().unwrap(), "-b", "topic"],
        );

        // From a subdirectory of the linked worktree, too.
        let sub = wt.join("nested");
        fs::create_dir_all(&sub).unwrap();

        let main_root = get_git_root(&main).unwrap();
        assert_eq!(git_root(&wt).unwrap(), main_root);
        assert_eq!(git_root(&sub).unwrap(), main_root);
    }
}
