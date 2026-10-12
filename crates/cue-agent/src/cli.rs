use clap::{ArgGroup, Parser, Subcommand};
use std::path::PathBuf;

#[derive(Parser)]
#[command(author, version, about = "Run named cue agents as child harness processes", long_about = None)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Run one batch of named-agent tasks concurrently from a JSON
    /// specification
    #[command(arg_required_else_help = true)]
    Run(Box<RunArgs>),
    /// Inspect the agents the layered manifest defines
    Agents {
        #[command(subcommand)]
        command: AgentCommands,
    },
}

/// Exactly one specification source: literal JSON, "-" for standard input, or
/// --spec PATH. A positional value is never guessed to be a path.
#[derive(clap::Args)]
#[command(group(ArgGroup::new("source").required(true).args(["input", "spec"])))]
pub struct RunArgs {
    /// The run specification as literal JSON, or "-" to read it from standard
    /// input
    #[arg(value_name = "JSON")]
    pub input: Option<String>,

    /// Read the run specification from a JSON file
    #[arg(long, value_name = "PATH")]
    pub spec: Option<PathBuf>,

    /// Per-run wall-clock deadline in seconds, counted from each harness
    /// launch; overrides the manifest timeout, and 0 means none
    #[arg(long, value_name = "SECS")]
    pub timeout: Option<u64>,

    /// Print the batch receipt as JSON instead of a human-readable summary
    #[arg(long)]
    pub json: bool,
}

#[derive(Subcommand)]
pub enum AgentCommands {
    /// List the merged agent definitions
    List {
        /// Output structured JSON instead of a human-readable listing
        #[arg(long)]
        json: bool,
    },
}
