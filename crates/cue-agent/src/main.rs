use clap::{Parser, Subcommand};
use std::io::Read;
use std::path::{Path, PathBuf};

#[derive(Parser)]
#[command(
    name = "cue-agent",
    version,
    about = "Supervised launcher and batch engine for headless coding agents"
)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Execute a batch of agent runs described by a JSON spec
    Run {
        /// JSON spec string; '-' reads the spec from stdin
        #[arg(conflicts_with = "spec_file", value_name = "JSON_SPEC")]
        spec: Option<String>,

        /// Read the JSON spec from a file
        #[arg(long = "spec-file", value_name = "PATH")]
        spec_file: Option<PathBuf>,

        /// cue context receiving artifacts (default: active context
        /// from .cue/HEAD, else master)
        #[arg(long = "task", value_name = "SLUG")]
        task: Option<String>,

        /// Maximum number of simultaneous children (0 = unbounded)
        #[arg(long = "concurrency", value_name = "N", default_value = "0")]
        concurrency: u64,

        /// Overall wall-clock cap for the whole batch, in seconds
        #[arg(long = "timeout", value_name = "SECS")]
        timeout: Option<u64>,
    },
}

/// Exit with a spec/usage error: message on stderr, exit code 2.
fn die(msg: impl std::fmt::Display) -> ! {
    eprintln!("error: {msg}");
    std::process::exit(2);
}

fn main() {
    let cli = Cli::parse();
    let Commands::Run {
        spec,
        spec_file,
        task,
        concurrency: _concurrency,
        timeout,
    } = cli.command;

    if let Some(slug) = &task
        && let Err(e) = cuelib::head::validate_slug(slug)
    {
        die(format!("--task: {e}"));
    }
    if timeout == Some(0) {
        die("--timeout must be greater than zero");
    }

    let (spec_json, base_dir) = match (spec, spec_file) {
        (Some(json), None) => {
            let json = if json == "-" {
                let mut buf = String::new();
                std::io::stdin()
                    .read_to_string(&mut buf)
                    .unwrap_or_else(|e| die(format!("failed to read stdin: {e}")));
                buf
            } else {
                json
            };
            let cwd = std::env::current_dir()
                .unwrap_or_else(|e| die(format!("failed to resolve current directory: {e}")));
            (json, cwd)
        }
        (None, Some(path)) => {
            let json = std::fs::read_to_string(&path).unwrap_or_else(|e| {
                die(format!("failed to read spec file {}: {e}", path.display()))
            });
            let base = path
                .parent()
                .filter(|p| !p.as_os_str().is_empty())
                .map(Path::to_path_buf)
                .unwrap_or_else(|| Path::new(".").to_path_buf());
            (json, base)
        }
        (None, None) => {
            die("no spec provided: pass a JSON string, --spec-file PATH, or '-' for stdin")
        }
        (Some(_), Some(_)) => {
            die("conflicting spec inputs: pass only one of JSON string, --spec-file, or '-'")
        }
    };

    let _specs = cue_agent::spec::load_spec(&spec_json, &base_dir).unwrap_or_else(|e| die(e));
}
