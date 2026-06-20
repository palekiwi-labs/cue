use std::path::{Path, PathBuf};
use std::process::Command;
use tempfile::TempDir;

/// The single authoritative test isolation boundary. All integration tests
/// MUST spawn the `cue` binary via `TestEnv::command()`. Never use a raw
/// `assert_cmd::Command` directly — doing so risks leaking into the
/// developer's real config and data directories.
#[allow(dead_code)]
pub struct TestEnv {
    pub temp_dir: TempDir,
    pub config_dir: PathBuf,
    pub data_dir: PathBuf,
}

impl Default for TestEnv {
    fn default() -> Self {
        Self::new()
    }
}

impl TestEnv {
    #[allow(dead_code)]
    pub fn new() -> Self {
        let temp_dir = tempfile::tempdir().expect("Failed to create temp dir");
        let config_dir = temp_dir.path().join("config");
        let data_dir = temp_dir.path().join("data");
        std::fs::create_dir_all(&config_dir).expect("Failed to create config dir");
        std::fs::create_dir_all(&data_dir).expect("Failed to create data dir");

        Self {
            temp_dir,
            config_dir,
            data_dir,
        }
    }

    /// Returns a fully isolated `cue` command. Config and data store
    /// directories are scoped to this `TestEnv`'s `TempDir` and are
    /// cleaned up automatically on drop.
    #[allow(dead_code)]
    pub fn command(&self) -> assert_cmd::Command {
        let mut cmd = assert_cmd::Command::cargo_bin("cue").expect("Failed to find cue binary");
        cmd.env("CUE_CONFIG_DIR", &self.config_dir)
            .env("CUE_DATA_DIR", &self.data_dir)
            .env_remove("CUE_ARTIFACT_TYPES")
            .env_remove("CUE_IGNORED_TYPES")
            .current_dir(self.temp_dir.path());
        cmd
    }

    #[allow(dead_code)]
    pub fn root(&self) -> &Path {
        self.temp_dir.path()
    }
}

/// Returns a `cue` command fully isolated from the host environment:
/// - `CUE_CONFIG_DIR` points to the system temp dir (no global cue.json)
/// - `CUE_ARTIFACT_TYPES` and `CUE_IGNORED_TYPES` are removed so project
///   cue.json and compiled-in defaults remain authoritative.
#[allow(dead_code)]
pub fn cue_cmd() -> assert_cmd::Command {
    let mut cmd = assert_cmd::Command::cargo_bin("cue").expect("Failed to find cue binary");
    cmd.env("CUE_CONFIG_DIR", std::env::temp_dir())
        .env_remove("CUE_ARTIFACT_TYPES")
        .env_remove("CUE_IGNORED_TYPES");
    cmd
}

pub fn setup_git_repo(dir: &Path) {
    Command::new("git")
        .args(["init", "-b", "main"])
        .current_dir(dir)
        .output()
        .expect("Failed to init git repo");

    Command::new("git")
        .args(["config", "user.email", "test@example.com"])
        .current_dir(dir)
        .output()
        .expect("Failed to config git user email");

    Command::new("git")
        .args(["config", "user.name", "Test User"])
        .current_dir(dir)
        .output()
        .expect("Failed to config git user name");

    Command::new("git")
        .args(["config", "commit.gpgsign", "false"])
        .current_dir(dir)
        .output()
        .expect("Failed to config git commit.gpgsign");

    std::fs::write(dir.join("initial.txt"), "hello").expect("Failed to write initial.txt");

    Command::new("git")
        .args(["add", "initial.txt"])
        .current_dir(dir)
        .output()
        .expect("Failed to git add");

    Command::new("git")
        .args(["commit", "-m", "initial commit"])
        .current_dir(dir)
        .output()
        .expect("Failed to git commit");
}

#[allow(dead_code)]
pub fn setup_remote(local: &Path, remote: &Path) {
    Command::new("git")
        .args(["init", "--bare"])
        .current_dir(remote)
        .output()
        .expect("Failed to init bare remote");

    Command::new("git")
        .args(["remote", "add", "origin", remote.to_str().unwrap()])
        .current_dir(local)
        .output()
        .expect("Failed to add remote origin");
}
