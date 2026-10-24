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
    /// List running batches, found without identifiers
    Ps {
        /// Include finished and crashed batches from the local records
        #[arg(long)]
        all: bool,
        /// Only batches started at or after this moment: Unix seconds, or a
        /// duration back from now (30m, 2h, 7d)
        #[arg(long, value_name = "WHEN")]
        since: Option<String>,
        /// At most this many batches, newest first
        #[arg(long, value_name = "N")]
        limit: Option<usize>,
        /// Only batches belonging to this job
        #[arg(long, value_name = "JOB_ID")]
        job: Option<String>,
        /// Output structured JSON
        #[arg(long)]
        json: bool,
    },
    /// Observe one batch or run: phase, progress and the last tool call
    Status {
        /// A batch or run identifier
        #[arg(value_name = "ID")]
        id: String,
        /// Output structured JSON
        #[arg(long)]
        json: bool,
    },
    /// Print a run's raw event records
    Logs {
        /// A run identifier
        #[arg(value_name = "RUN_ID")]
        id: String,
        /// Print the harness's stderr instead of its events
        #[arg(long)]
        stderr: bool,
        /// Keep printing new records until the run finishes
        #[arg(long, short)]
        follow: bool,
    },
    /// Ask the supervising cue-agent to stop a run or a whole batch
    Stop {
        /// A batch or run identifier
        #[arg(value_name = "ID")]
        id: String,
        /// Wait until the run or batch has finished, then report its status
        #[arg(long)]
        wait: bool,
        /// Output structured JSON
        #[arg(long)]
        json: bool,
    },
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
