use clap::{Parser, Subcommand};
use std::path::PathBuf;

#[derive(Parser)]
#[command(author, version, about = "Run named cue agents as child harness processes", long_about = None)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Run one batch of named agents concurrently and print a JSON receipt
    #[command(arg_required_else_help = true)]
    Run(Box<RunArgs>),
    /// Inspect the agents the layered manifest defines
    Agents {
        #[command(subcommand)]
        command: AgentCommands,
    },
}

#[derive(clap::Args)]
pub struct RunArgs {
    /// Named agents to run, one run each; repeat a name to run it twice
    #[arg(value_name = "AGENT")]
    pub agents: Vec<String>,

    /// Prompt sent to every named agent
    #[arg(short = 'p', long, value_name = "TEXT", conflicts_with_all = &["prompt_file", "batch"])]
    pub prompt: Option<String>,

    /// Read the prompt from a file; "-" reads standard input
    #[arg(long, value_name = "PATH", conflicts_with_all = &["prompt", "batch"])]
    pub prompt_file: Option<String>,

    /// Read per-agent runs from a JSON batch; "-" reads standard input
    #[arg(long, value_name = "PATH", conflicts_with_all = &["prompt", "prompt_file"])]
    pub batch: Option<String>,

    /// Caller's cue context: selects the trace destination and is exported to
    /// the child as CUE_CONTEXT
    #[arg(long, value_name = "SLUG")]
    pub context: Option<String>,

    /// Short label recorded as the trace description and used in its filename
    #[arg(long, value_name = "TEXT")]
    pub label: Option<String>,

    /// Per-run wall-clock deadline in seconds
    #[arg(long, value_name = "SECS")]
    pub timeout: Option<u64>,

    /// Working directory for the harness; defaults to the current directory
    #[arg(long, value_name = "PATH")]
    pub cwd: Option<PathBuf>,

    /// Harness executable; defaults to $CUE_AGENT_HARNESS, then `pi`
    #[arg(long, value_name = "PATH")]
    pub harness: Option<PathBuf>,
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
