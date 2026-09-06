use std::path::{Path, PathBuf};
use std::process::Command;
use tempfile::TempDir;

#[allow(dead_code)]
pub const TEST_ORIGIN_URL: &str = "https://github.com/acme/widgets.git";

/// The single authoritative test isolation boundary. All integration tests
/// MUST spawn the `cue` binary via `TestEnv::command()`. Never use a raw
/// `assert_cmd::Command` directly — doing so risks leaking into the
/// developer's real config and data directories.
#[allow(dead_code)]
pub struct TestEnv {
    pub temp_dir: TempDir,
    pub config_dir: PathBuf,
    pub data_dir: PathBuf,
    pub cue_home: PathBuf,
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
        let cue_home = temp_dir.path().join("cue-home");
        std::fs::create_dir_all(&config_dir).expect("Failed to create config dir");
        std::fs::create_dir_all(&data_dir).expect("Failed to create data dir");
        std::fs::create_dir_all(&cue_home).expect("Failed to create CUE_HOME");

        Self {
            temp_dir,
            config_dir,
            data_dir,
            cue_home,
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
            .env("CUE_HOME", &self.cue_home)
            .env_remove("CUE_ARTIFACT_TYPES")
            .env_remove("CUE_IGNORED_TYPES")
            .env_remove("CUE_TASK")
            .current_dir(self.temp_dir.path());
        cmd
    }

    #[allow(dead_code)]
    pub fn root(&self) -> &Path {
        self.temp_dir.path()
    }

    #[allow(dead_code)]
    pub fn cue_home(&self) -> &Path {
        &self.cue_home
    }

    #[allow(dead_code)]
    pub fn setup_repo_with_origin(&self) {
        setup_git_repo(self.root());
        setup_origin(self.root(), TEST_ORIGIN_URL);
    }
}

#[allow(dead_code)]
pub fn setup_origin(dir: &Path, url: &str) {
    let output = Command::new("git")
        .args(["remote", "add", "origin", url])
        .current_dir(dir)
        .output()
        .expect("Failed to add remote origin");

    assert!(
        output.status.success(),
        "Failed to add remote origin: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[allow(dead_code)]
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
