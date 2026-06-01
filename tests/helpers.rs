use std::path::{Path, PathBuf};
use std::process::Command;
use tempfile::TempDir;

#[allow(dead_code)]
pub struct TestEnv {
    pub temp_dir: TempDir,
    pub config_dir: PathBuf,
}

impl TestEnv {
    #[allow(dead_code)]
    pub fn new() -> Self {
        let temp_dir = tempfile::tempdir().expect("Failed to create temp dir");
        let config_dir = temp_dir.path().join("config");
        std::fs::create_dir(&config_dir).expect("Failed to create config dir");

        Self {
            temp_dir,
            config_dir,
        }
    }

    #[allow(dead_code)]
    pub fn command(&self) -> assert_cmd::Command {
        let mut cmd = assert_cmd::Command::cargo_bin("mem").expect("Failed to find mem binary");
        cmd.env("MEM_CONFIG_DIR", &self.config_dir);
        // Isolate from user's git config if necessary, but for now we focus on mem config
        cmd.current_dir(self.temp_dir.path());
        cmd
    }

    #[allow(dead_code)]
    pub fn root(&self) -> &Path {
        self.temp_dir.path()
    }
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
