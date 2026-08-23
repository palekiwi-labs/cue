use clap::{Parser, Subcommand};
use std::io::Read;
use std::path::PathBuf;

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
    },
}

/// Exit with a spec/usage error: message on stderr, exit code 2.
fn die(msg: impl std::fmt::Display) -> ! {
    eprintln!("error: {msg}");
    std::process::exit(2);
}

fn main() {
    let cli = Cli::parse();
    let Commands::Run { spec, spec_file } = cli.command;

    let _spec_json: String = match (spec, spec_file) {
        (Some(json), None) => {
            if json == "-" {
                let mut buf = String::new();
                std::io::stdin()
                    .read_to_string(&mut buf)
                    .unwrap_or_else(|e| die(format!("failed to read stdin: {e}")));
                buf
            } else {
                json
            }
        }
        (None, Some(path)) => std::fs::read_to_string(&path)
            .unwrap_or_else(|e| die(format!("failed to read spec file {}: {e}", path.display()))),
        (None, None) => {
            die("no spec provided: pass a JSON string, --spec-file PATH, or '-' for stdin")
        }
        (Some(_), Some(_)) => {
            die("conflicting spec inputs: pass only one of JSON string, --spec-file, or '-'")
        }
    };
}
