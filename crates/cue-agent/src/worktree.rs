//! New-worktree preparation and cleanup for one task.
//!
//! A task's worktree is always new: a branch created at the requested base and
//! a checkout of it at a destination that did not exist. Ownership is recorded
//! the moment each resource is acquired, so a preparation that fails part way
//! removes exactly what it made, and cleanup never touches anything it did not
//! create: pre-existing branches, paths and the source checkout stay as found.
//!
//! The checkout directory itself is the ownership point: it is created with
//! an exclusive `mkdir` before `git worktree add` runs, so a directory that
//! appears from elsewhere is a collision, never adopted. A registration found
//! after a failed add is claimed only when it checks out this task's own new
//! branch. Destinations that equal or contain a registered worktree, or that
//! overlap a checkout created earlier in the batch, are refused, so removing
//! one task's checkout can never delete another's.
//!
//! The branch is created by an atomic create-only ref update carrying a
//! reflog message unique to the run. Git commits a ref before running the
//! reference-transaction `committed` hook, so a creation that fails or is
//! stopped may still have made the branch: it is then claimed only when its
//! reflog begins with this run's marker at the base commit, and a branch
//! that cannot be attributed either way is reported rather than left
//! silently.
//!
//! Git runs file-backed in its own process group, like the harness: a
//! terminal interrupt reaches cue-agent's foreground group, not a git command
//! half way through creating or removing a checkout, and a hook that leaves a
//! descendant holding stdout cannot cause a pipe-EOF wait. Every command is
//! bounded, so a hanging hook cannot hold preparation or cleanup forever.
//! Repository-local Git variables inherited from the caller are cleared, so
//! only the target directory selects the repository.

use crate::engine;
use crate::receipt::RetainedWorktree;
use crate::run_spec::Worktree;
use anyhow::{Context, Result, bail};
use std::ffi::OsStr;
use std::io::{Read, Seek, SeekFrom};
use std::path::{Component, Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

const POLL_INTERVAL: Duration = Duration::from_millis(15);
/// The longest an inspection, cleanup or trace-writing command may run
/// before its group is killed. Once the batch is interrupted, the grace
/// window applies instead when it is shorter.
pub const HELPER_LIMIT: Duration = Duration::from_secs(60);

/// Variables that select or redirect a repository, as listed by
/// `git rev-parse --local-env-vars` (Git 2.55), plus `GIT_NAMESPACE`.
/// Inherited values would make Git act on another repository, work tree,
/// index or ref namespace than the task's target.
pub const REPOSITORY_ENV: &[&str] = &[
    "GIT_ALTERNATE_OBJECT_DIRECTORIES",
    "GIT_CONFIG",
    "GIT_CONFIG_PARAMETERS",
    "GIT_CONFIG_COUNT",
    "GIT_OBJECT_DIRECTORY",
    "GIT_DIR",
    "GIT_WORK_TREE",
    "GIT_IMPLICIT_WORK_TREE",
    "GIT_GRAFT_FILE",
    "GIT_INDEX_FILE",
    "GIT_NO_REPLACE_OBJECTS",
    "GIT_REPLACE_REF_BASE",
    "GIT_PREFIX",
    "GIT_SHALLOW_FILE",
    "GIT_COMMON_DIR",
    "GIT_NAMESPACE",
];

/// Resources created for one task, recorded as they were acquired.
#[derive(Debug)]
pub struct Owned {
    /// The task's target directory, inside the source repository; Git
    /// commands for this worktree run from here.
    repo: PathBuf,
    base: String,
    pub base_commit: String,
    /// The checkout, absolute, with its parent's symlinks resolved.
    pub path: PathBuf,
    pub branch: String,
    pub ephemeral: bool,
    path_generated: bool,
    branch_generated: bool,
    branch_created: bool,
    /// The checkout directory was made by this task's exclusive `mkdir`.
    leaf_created: bool,
    /// Git registered the checkout on this task's branch.
    checkout_created: bool,
    /// Missing ancestors of the checkout created for it, outermost first.
    dirs_created: Vec<PathBuf>,
    /// Resources a failed creation may have left that cannot be attributed
    /// to this task (a branch, a ref lock), so they are neither removed nor
    /// silently left: reported.
    unattributed: Vec<String>,
}

/// A preparation that did not produce a checkout. Whatever it had created is
/// already removed; anything that could not be is named in `cleanup_errors`.
#[derive(Debug)]
pub struct Failure {
    pub message: String,
    pub cleanup_errors: Vec<String>,
}

/// Create a new branch and checkout for one task.
///
/// `target` is the task's effective cwd, which selects the source repository.
/// `root` is the configured worktree root for that target, used when the
/// request names no path; `name` is a per-run unique identity used for
/// generated paths and branches. `others` are the checkouts already created
/// for other tasks in the batch; a destination overlapping one is refused.
pub fn prepare(
    request: &Worktree,
    target: &Path,
    root: Option<&Path>,
    name: &str,
    others: &[PathBuf],
) -> std::result::Result<Owned, Failure> {
    let mut owned = Owned {
        repo: target.to_path_buf(),
        base: request.base.clone(),
        base_commit: String::new(),
        path: PathBuf::new(),
        branch: String::new(),
        ephemeral: request.ephemeral,
        path_generated: false,
        branch_generated: false,
        branch_created: false,
        leaf_created: false,
        checkout_created: false,
        dirs_created: Vec::new(),
        unattributed: Vec::new(),
    };
    match acquire(&mut owned, request, root, name, others) {
        Ok(()) => Ok(owned),
        Err(err) => {
            let mut cleanup_errors = std::mem::take(&mut owned.unattributed);
            cleanup_errors.extend(remove(&owned));
            Err(Failure {
                message: format!("could not prepare the worktree: {err:#}"),
                cleanup_errors,
            })
        }
    }
}

fn acquire(
    owned: &mut Owned,
    request: &Worktree,
    root: Option<&Path>,
    name: &str,
    others: &[PathBuf],
) -> Result<()> {
    let target = owned.repo.clone();
    git(&target, ["rev-parse", "--git-dir"], true)
        .with_context(|| format!("{} is not inside a Git repository", target.display()))?;
    owned.base_commit = git(
        &target,
        [
            "rev-parse",
            "--verify",
            "--end-of-options",
            &format!("{}^{{commit}}", request.base),
        ],
        true,
    )
    .with_context(|| format!("base '{}' does not name a commit", request.base))?;

    let destination = match (&request.path, root) {
        (Some(path), _) => path.clone(),
        (None, Some(root)) => {
            owned.path_generated = true;
            root.join(name)
        }
        (None, None) => bail!("no path given and no worktree_root configured"),
    };
    let Some(leaf) = destination.file_name().map(OsStr::to_os_string) else {
        bail!("{} does not name a new directory", destination.display());
    };
    owned.path = destination.clone();
    if std::fs::symlink_metadata(&destination).is_ok() {
        bail!(
            "{} already exists; worktrees are only created, never adopted",
            destination.display()
        );
    }
    // Refused before anything is created. The checks are repeated against
    // the final path below, since resolving it may differ by then.
    let listed = list(&target).context("could not list registered worktrees")?;
    check_overlap(&resolve(&destination), &listed, others)?;
    owned.branch = match &request.branch {
        Some(branch) => branch.clone(),
        None => {
            owned.branch_generated = true;
            format!("cue-agent/{name}")
        }
    };

    // The name must be a branch name as `git branch` accepts it, unexpanded.
    let checked = git(
        &target,
        ["check-ref-format", "--branch", owned.branch.as_str()],
        true,
    );
    if checked.as_deref().ok() != Some(owned.branch.as_str()) {
        bail!("'{}' is not a valid branch name", owned.branch);
    }
    let reference = format!("refs/heads/{}", owned.branch);
    if ref_exists(&target, &reference)
        .with_context(|| format!("could not check for branch '{}'", owned.branch))?
    {
        bail!(
            "could not create branch '{}': it already exists; branches are only created, \
             never adopted",
            owned.branch
        );
    }
    // Create-only (an empty old value), so a branch that appeared since the
    // check above is refused, never claimed. The reflog message is unique to
    // this run and is what proves ownership if git does not report success.
    let marker = format!("cue-agent: create branch for run {name}");
    let created = git(
        &target,
        [
            "update-ref",
            "--create-reflog",
            "-m",
            marker.as_str(),
            reference.as_str(),
            owned.base_commit.as_str(),
            "",
        ],
        true,
    );
    match created {
        Ok(_) => owned.branch_created = true,
        Err(err) => {
            match created_by(&target, &reference, &owned.base_commit, &marker) {
                Ok(ours) => owned.branch_created = ours,
                Err(why) => {
                    owned.unattributed.push(format!(
                        "branch '{}' (repository {}) may survive: it could not be \
                         attributed after its creation failed: {why:#}",
                        owned.branch,
                        target.display()
                    ));
                }
            }
            owned
                .unattributed
                .extend(lock_left(&target, &reference, &owned.branch));
            return Err(err).with_context(|| format!("could not create branch '{}'", owned.branch));
        }
    }

    let parent = destination.parent().unwrap_or(Path::new("/"));
    create_missing_dirs(parent, &mut owned.dirs_created)?;
    let parent = std::fs::canonicalize(parent)
        .with_context(|| format!("could not resolve {}", parent.display()))?;
    owned.path = parent.join(leaf);
    let listed = list(&target).context("could not list registered worktrees")?;
    check_overlap(&owned.path, &listed, others)?;

    // The exclusive mkdir is the ownership point for the checkout directory:
    // one that appeared since the check above belongs to someone else.
    // `git worktree add` accepts the empty directory made here.
    match std::fs::create_dir(&owned.path) {
        Ok(()) => owned.leaf_created = true,
        Err(err) if err.kind() == std::io::ErrorKind::AlreadyExists => bail!(
            "{} already exists; worktrees are only created, never adopted",
            owned.path.display()
        ),
        Err(err) => {
            return Err(err).with_context(|| format!("could not create {}", owned.path.display()));
        }
    }

    let added = git(
        &target,
        [
            OsStr::new("worktree"),
            OsStr::new("add"),
            OsStr::new("--"),
            owned.path.as_os_str(),
            OsStr::new(&owned.branch),
        ],
        true,
    );
    // `git worktree add` can fail after registering the checkout, for
    // example when a post-checkout hook fails. A registration is this task's
    // only if it checks out the branch this task just created: Git refuses
    // a second checkout of one branch, and the path alone may be someone
    // else's (a missing or locked worktree, or a concurrent add).
    if added.is_ok() || claims(owned) {
        owned.checkout_created = true;
    }
    added.context("could not create the worktree")?;
    Ok(())
}

/// Create the missing ancestors of a destination, recording each directory
/// this call made. One that appears concurrently belongs to someone else.
fn create_missing_dirs(dir: &Path, created: &mut Vec<PathBuf>) -> Result<()> {
    let mut missing = Vec::new();
    let mut current = dir;
    loop {
        match std::fs::symlink_metadata(current) {
            Ok(_) => break,
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
                missing.push(current.to_path_buf());
                match current.parent() {
                    Some(parent) => current = parent,
                    None => break,
                }
            }
            Err(err) => {
                return Err(err)
                    .with_context(|| format!("could not inspect {}", current.display()));
            }
        }
    }
    for dir in missing.into_iter().rev() {
        match std::fs::create_dir(&dir) {
            Ok(()) => created.push(dir),
            Err(err) if err.kind() == std::io::ErrorKind::AlreadyExists => {}
            Err(err) => {
                return Err(err).with_context(|| format!("could not create {}", dir.display()));
            }
        }
    }
    Ok(())
}

/// One checkout Git has registered, missing or not.
struct Listed {
    /// Resolved as by [`resolve`], so aliases compare equal.
    path: PathBuf,
    /// The full ref checked out there, if any.
    branch: Option<String>,
}

/// The checkouts registered for the repository at `repo`.
fn list(repo: &Path) -> Result<Vec<Listed>> {
    let raw = git(repo, ["worktree", "list", "--porcelain", "-z"], false)?;
    let mut listed: Vec<Listed> = Vec::new();
    for field in raw.split('\0') {
        if let Some(path) = field.strip_prefix("worktree ") {
            listed.push(Listed {
                path: resolve(Path::new(path)),
                branch: None,
            });
        } else if let Some(branch) = field.strip_prefix("branch ")
            && let Some(last) = listed.last_mut()
        {
            last.branch = Some(branch.to_string());
        }
    }
    Ok(listed)
}

/// `path` with the symlinks of its longest existing prefix resolved and the
/// rest normalised lexically, as the missing directories will be created.
fn resolve(path: &Path) -> PathBuf {
    let components: Vec<Component> = path.components().collect();
    for split in (1..=components.len()).rev() {
        let prefix: PathBuf = components[..split].iter().collect();
        if let Ok(mut resolved) = std::fs::canonicalize(&prefix) {
            for component in &components[split..] {
                match component {
                    Component::ParentDir => {
                        resolved.pop();
                    }
                    Component::Normal(name) => resolved.push(name),
                    _ => {}
                }
            }
            return resolved;
        }
    }
    path.to_path_buf()
}

/// Refuse a destination that is, or would contain, a registered checkout,
/// or that equals, contains or lies inside a checkout created for another
/// task in this batch: removing one would delete the other.
fn check_overlap(path: &Path, listed: &[Listed], others: &[PathBuf]) -> Result<()> {
    for entry in listed {
        if entry.path == path {
            bail!(
                "{} is already registered as a worktree; worktrees are only created, \
                 never adopted",
                path.display()
            );
        }
        if entry.path.starts_with(path) {
            bail!(
                "{} would contain the registered worktree {}",
                path.display(),
                entry.path.display()
            );
        }
    }
    for other in others {
        if path.starts_with(other) || other.starts_with(path) {
            bail!(
                "{} overlaps the worktree {} created for another task in this batch",
                path.display(),
                other.display()
            );
        }
    }
    Ok(())
}

/// Whether Git has this task's checkout registered: its path, on its
/// branch.
fn claims(owned: &Owned) -> bool {
    let branch = format!("refs/heads/{}", owned.branch);
    list(&owned.repo).is_ok_and(|listed| {
        listed.iter().any(|entry| {
            entry.path == owned.path && entry.branch.as_deref() == Some(branch.as_str())
        })
    })
}

/// Whether the full ref `reference` exists. A failed check is an error, never
/// read as absence.
fn ref_exists(repo: &Path, reference: &str) -> Result<bool> {
    let ran = run_git(repo, ["show-ref", "--verify", "--quiet", reference], false)?;
    match ran.code {
        Some(0) => Ok(true),
        // `--verify --quiet` exits 1, silently, for a missing ref.
        Some(1) if ran.stderr.is_empty() => Ok(false),
        _ => bail!("git {} failed: {}", ran.shown, ran.stderr),
    }
}

/// After a creation that git did not report as successful: whether
/// `reference` exists because this task's update created it. Its reflog
/// must begin with `marker` at `commit`; a reflog that begins otherwise is
/// another actor's branch. An existing branch whose origin cannot be read is
/// an error, for the caller to report.
fn created_by(repo: &Path, reference: &str, commit: &str, marker: &str) -> Result<bool> {
    if !ref_exists(repo, reference)? {
        return Ok(false);
    }
    let log = git(
        repo,
        ["reflog", "show", "--format=%H %gs", reference, "--"],
        false,
    )?;
    // Newest first: the last entry is the one that created the ref.
    match log.lines().last() {
        Some(first) => Ok(first == format!("{commit} {marker}")),
        None => bail!("it exists without a reflog"),
    }
}

/// After a ref update for `reference` failed or was stopped: a report of
/// the ref's lock file if one is present. A killed git leaves its lock
/// behind, but a lock may also be another actor's update in progress, so it
/// is never deleted, only named by its exact path. The path comes from
/// `git rev-parse --git-path`, which places it in the common directory and
/// follows nested branch names.
fn lock_left(repo: &Path, reference: &str, branch: &str) -> Option<String> {
    let shown = repo.display();
    let lock = match git(
        repo,
        ["rev-parse", "--git-path", &format!("{reference}.lock")],
        false,
    ) {
        Ok(path) => repo.join(path),
        Err(err) => {
            return Some(format!(
                "a lock for branch '{branch}' (repository {shown}) may survive: \
                 its path could not be resolved: {err:#}"
            ));
        }
    };
    match std::fs::symlink_metadata(&lock) {
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => None,
        Ok(_) => Some(format!(
            "lock file {} for branch '{branch}' (repository {shown}) survives: it may \
             belong to another update, so it was not removed",
            lock.display()
        )),
        Err(err) => Some(format!(
            "lock file {} for branch '{branch}' (repository {shown}) may survive: {err}",
            lock.display()
        )),
    }
}

impl Owned {
    /// The identifiers a caller needs to find retained work: only those that
    /// were generated, since the caller already knows the ones it chose.
    pub fn retained(&self) -> Option<RetainedWorktree> {
        (self.path_generated || self.branch_generated).then(|| RetainedWorktree {
            path: self.path_generated.then(|| self.path.clone()),
            branch: self.branch_generated.then(|| self.branch.clone()),
        })
    }

    /// The checkout's current commit, recorded before it may be removed.
    pub fn head(&self) -> Option<String> {
        git(&self.path, ["rev-parse", "HEAD"], false).ok()
    }

    /// The local record of this worktree, without environment or contents.
    pub fn record(&self) -> serde_json::Value {
        serde_json::json!({
            "target": self.repo,
            "base": self.base,
            "base_commit": self.base_commit,
            "path": self.path,
            "branch": self.branch,
            "ephemeral": self.ephemeral,
            "path_generated": self.path_generated,
            "branch_generated": self.branch_generated,
        })
    }
}

/// Remove everything `owned` records as created, discarding modifications.
/// Each returned error names what survives and where.
pub fn remove(owned: &Owned) -> Vec<String> {
    let mut errors = Vec::new();
    let repo = owned.repo.display();
    let mut checkout_survives = false;
    if owned.checkout_created {
        // Forced twice: the checkout is disposable even if dirty or locked.
        let removed = git(
            &owned.repo,
            [
                OsStr::new("worktree"),
                OsStr::new("remove"),
                OsStr::new("--force"),
                OsStr::new("--force"),
                OsStr::new("--"),
                owned.path.as_os_str(),
            ],
            false,
        );
        if let Err(err) = removed {
            checkout_survives = true;
            errors.push(format!(
                "worktree {} (branch '{}', repository {repo}) survives: {err:#}",
                owned.path.display(),
                owned.branch
            ));
        }
    } else if owned.leaf_created {
        errors.extend(remove_unclaimed_leaf(owned));
    }
    if owned.branch_created {
        if checkout_survives {
            errors.push(format!(
                "branch '{}' (repository {repo}) survives: its worktree {} could not be removed",
                owned.branch,
                owned.path.display()
            ));
        } else if let Err(err) = git(
            &owned.repo,
            ["branch", "-D", "--", owned.branch.as_str()],
            false,
        ) {
            // A failed or stopped deletion may still have removed the ref,
            // for example before a hook it waited on was killed.
            let reference = format!("refs/heads/{}", owned.branch);
            if !matches!(ref_exists(&owned.repo, &reference), Ok(false)) {
                errors.push(format!(
                    "branch '{}' (repository {repo}) survives: {err:#}",
                    owned.branch
                ));
            }
            errors.extend(lock_left(&owned.repo, &reference, &owned.branch));
        }
    }
    if !checkout_survives {
        for dir in owned.dirs_created.iter().rev() {
            match std::fs::remove_dir(dir) {
                Ok(()) => {}
                Err(err) if err.kind() == std::io::ErrorKind::NotFound => {}
                // Something else now lives there, so it is shared, not ours.
                Err(err)
                    if matches!(
                        err.raw_os_error(),
                        Some(libc::ENOTEMPTY) | Some(libc::EEXIST)
                    ) =>
                {
                    break;
                }
                Err(err) => {
                    errors.push(format!("directory {} survives: {err}", dir.display()));
                    break;
                }
            }
        }
    }
    errors
}

/// Remove the checkout directory this task made when Git holds no checkout
/// of its branch there: what a failed or killed `git worktree add` left
/// behind. Unregistered contents were written into this task's own
/// directory and go with it; contents under a registration this task cannot
/// prove its own are kept and reported.
fn remove_unclaimed_leaf(owned: &Owned) -> Option<String> {
    let path = &owned.path;
    let survives = |why: String| {
        Some(format!(
            "directory {} (repository {}) survives: {why}",
            path.display(),
            owned.repo.display()
        ))
    };
    match std::fs::remove_dir(path) {
        Ok(()) => return None,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => return None,
        Err(err) if err.raw_os_error() == Some(libc::ENOTEMPTY) => {}
        Err(err) => return survives(err.to_string()),
    }
    let listed = match list(&owned.repo) {
        Ok(listed) => listed,
        Err(err) => return survives(format!("could not check its registration: {err:#}")),
    };
    if let Some(entry) = listed.iter().find(|entry| entry.path == *path) {
        let on = entry.branch.as_deref().unwrap_or("a detached HEAD");
        return survives(format!(
            "it holds a checkout registered on {on}, not this task's branch '{}'",
            owned.branch
        ));
    }
    match std::fs::remove_dir_all(path) {
        Ok(()) => None,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => None,
        Err(err) => survives(format!("partial checkout could not be removed: {err}")),
    }
}

/// Run one bounded, non-interruptible inspection command from `dir` with the
/// same isolation as every worktree command, returning its trimmed stdout.
pub fn inspect<I, S>(dir: &Path, args: I) -> Result<String>
where
    I: IntoIterator<Item = S>,
    S: AsRef<OsStr>,
{
    git(dir, args, false)
}

/// Run one git command from `dir`, returning its trimmed stdout.
fn git<I, S>(dir: &Path, args: I, interruptible: bool) -> Result<String>
where
    I: IntoIterator<Item = S>,
    S: AsRef<OsStr>,
{
    let ran = run_git(dir, args, interruptible)?;
    if ran.code == Some(0) {
        Ok(ran.stdout)
    } else {
        bail!("git {} failed: {}", ran.shown, ran.stderr)
    }
}

/// A git command that ran to its own exit.
struct Ran {
    shown: String,
    /// `None` when git was ended by a signal.
    code: Option<i32>,
    stdout: String,
    stderr: String,
}

/// Run one git command from `dir` to its exit, whatever its status.
///
/// Every command is bounded; an error means it could not be run or waited
/// for, or was stopped. Preparation commands are `interruptible`: once the
/// batch is interrupted they get the teardown grace window to finish before
/// their group is killed. Inspection and cleanup commands are what an
/// interrupt waits for, so they are capped at [`HELPER_LIMIT`] instead, or at
/// the grace window from when they observe the interrupt if that is sooner:
/// a hook that never exits delays finalization, but cannot stop it.
fn run_git<I, S>(dir: &Path, args: I, interruptible: bool) -> Result<Ran>
where
    I: IntoIterator<Item = S>,
    S: AsRef<OsStr>,
{
    use std::os::unix::process::CommandExt;
    let args: Vec<_> = args
        .into_iter()
        .map(|arg| arg.as_ref().to_os_string())
        .collect();
    let shown = args
        .iter()
        .map(|arg| arg.to_string_lossy())
        .collect::<Vec<_>>()
        .join(" ");
    let mut stdout = tempfile::tempfile().context("Could not create a git output file")?;
    let mut stderr = tempfile::tempfile().context("Could not create a git output file")?;
    let mut command = Command::new("git");
    for name in REPOSITORY_ENV {
        command.env_remove(name);
    }
    let mut child = command
        .args(&args)
        .current_dir(dir)
        .stdin(Stdio::null())
        .stdout(Stdio::from(stdout.try_clone()?))
        .stderr(Stdio::from(stderr.try_clone()?))
        .process_group(0)
        .spawn()
        .with_context(|| format!("could not run git {shown}"))?;
    let group = child.id() as i32;
    let kill_group = || {
        // SAFETY: `kill` is always safe; the child leads its own group.
        unsafe { libc::kill(-group, libc::SIGKILL) };
    };
    let mut kill_at = (!interruptible).then(|| Instant::now() + HELPER_LIMIT);
    let mut interrupted = false;
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) => {}
            Err(err) => {
                kill_group();
                let _ = child.wait();
                bail!("could not wait for git {shown}: {err}");
            }
        }
        if !interrupted && engine::abort_requested().is_some() {
            interrupted = true;
            let at = Instant::now() + engine::grace_window();
            kill_at = Some(kill_at.map_or(at, |cap| cap.min(at)));
        }
        if kill_at.is_some_and(|at| Instant::now() >= at) {
            kill_group();
            let _ = child.wait();
            if interrupted {
                bail!("git {shown} was stopped after the batch was interrupted");
            }
            bail!(
                "git {shown} did not finish within {}s and was stopped",
                HELPER_LIMIT.as_secs()
            );
        }
        std::thread::sleep(POLL_INTERVAL);
    };
    let read = |file: &mut std::fs::File| -> Vec<u8> {
        let mut bytes = Vec::new();
        let _ = file.seek(SeekFrom::Start(0));
        let _ = file.read_to_end(&mut bytes);
        bytes
    };
    // Stdout carries paths and refs that are compared, so it is decoded
    // strictly: a lossy decoding could make distinct paths compare equal.
    let stdout = String::from_utf8(read(&mut stdout))
        .map_err(|_| anyhow::anyhow!("git {shown} printed output that is not valid UTF-8"))?
        .trim()
        .to_string();
    Ok(Ran {
        code: status.code(),
        stdout,
        stderr: String::from_utf8_lossy(&read(&mut stderr))
            .trim()
            .to_string(),
        shown,
    })
}
