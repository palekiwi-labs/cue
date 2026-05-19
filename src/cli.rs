use clap::{Parser, Subcommand, ValueEnum};

#[derive(Parser)]
#[command(author, version, about, long_about = None)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(ValueEnum, Clone, Copy, Debug)]
pub enum MemType {
    Spec,
    Trace,
    Tmp,
    Ref,
    Bin,
    Doc,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Initialize agent artifacts directory structure
    Init,
    /// Add a new artifact
    #[command(arg_required_else_help = true)]
    Add {
        /// Name of the artifact file
        filename: String,
        /// Initial content for the file (use "-" to read from stdin)
        #[arg(conflicts_with_all = &["file", "clipboard"])]
        content: Option<String>,
        /// Read content from a file (recommended for AI agents to avoid escaping)
        #[arg(short = 'f', long = "file", conflicts_with_all = &["content", "clipboard"])]
        file: Option<String>,
        /// Read content from system clipboard
        #[arg(short = 'c', long = "clipboard", conflicts_with_all = &["content", "file"])]
        clipboard: bool,
        /// Type of artifact
        #[arg(short = 't', long = "type", value_enum, default_value = "spec")]
        mem_type: MemType,
        /// Save artifact to a specific branch instead of current
        #[arg(short = 'b', long)]
        branch: Option<String>,
        /// Overwrite existing file
        #[arg(long)]
        force: bool,
    },

    /// List artifacts for a branch
    List {
        /// List files for a specific branch instead of current
        #[arg(long, conflicts_with = "all")]
        branch: Option<String>,
        /// List files for all branches
        #[arg(short = 'a', long)]
        all: bool,
        /// Filter by artifact type
        #[arg(short = 't', long = "type", value_enum)]
        mem_type: Option<MemType>,
        /// Include gitignored categories (tmp, ref)
        #[arg(short = 'i', long)]
        include_gitignored: bool,
        /// Output as JSON
        #[arg(short = 'j', long)]
        json: bool,
    },
    /// Manage project log (add entries)
    Log {
        #[command(subcommand)]
        command: LogCommands,
    },
    /// Manage branch-specific AI agent context
    Context {
        #[command(subcommand)]
        command: ContextCommands,
    },
}

#[derive(Subcommand)]
pub enum ContextCommands {
    /// Create context.json, auto-populated from existing spec/ files
    Init {
        /// Overwrite existing context.json
        #[arg(long)]
        force: bool,
    },
    /// Print raw context.json
    Show,
    /// List available profile names
    Profiles,
    /// Expand and stream context to stdout
    Render {
        /// Profile name to render
        #[arg(short = 'p', long, default_value = "default")]
        profile: Option<String>,
    },
    /// Print absolute path to context.json
    Path {
        /// Show paths for all branches
        #[arg(short = 'a', long)]
        all: bool,
    },
}

#[derive(Subcommand)]
pub enum LogCommands {
    /// Add a new log entry
    Add {
        /// Entry title (required unless --file is used)
        #[arg(long)]
        title: Option<String>,
        /// Entry body text
        #[arg(long)]
        body: Option<String>,
        /// Findings (can be repeated)
        #[arg(long)]
        found: Vec<String>,
        /// Decisions (can be repeated)
        #[arg(long)]
        decided: Vec<String>,
        /// Open questions (can be repeated)
        #[arg(long)]
        open: Vec<String>,
        /// Read entry data from a JSON file
        #[arg(short = 'f', long, conflicts_with_all = &["title", "body", "found", "decided", "open"])]
        file: Option<String>,
    },
    /// List log entries
    List {
        /// List log for a specific branch instead of current
        #[arg(long)]
        branch: Option<String>,
    },
}
