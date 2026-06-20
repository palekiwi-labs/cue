use crate::list::Filter;
use clap::{Parser, Subcommand};

fn parse_frontmatter_field(s: &str) -> Result<(String, String), String> {
    let (k, v) = s
        .split_once('=')
        .ok_or_else(|| format!("Expected key=value, got '{}'", s))?;
    if k.is_empty() {
        return Err(format!("Frontmatter key cannot be empty in '{}'", s));
    }
    Ok((k.to_string(), v.to_string()))
}

#[derive(Parser)]
#[command(author, version, about, long_about = None)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
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
        #[arg(long = "file", conflicts_with_all = &["content", "clipboard"])]
        file: Option<String>,
        /// Read content from system clipboard
        #[arg(short = 'c', long = "clipboard", conflicts_with_all = &["content", "file"])]
        clipboard: bool,
        /// Frontmatter fields to prepend to the artifact (repeatable, KEY=VALUE format)
        #[arg(short = 'f', long = "frontmatter", value_name = "KEY=VALUE", value_parser = parse_frontmatter_field)]
        frontmatter: Vec<(String, String)>,
        /// Type of artifact (must be in configured artifact_types)
        #[arg(short = 't', long = "type", default_value = "spec")]
        cue_type: String,
        /// Save artifact at the root of the type directory, not under a <timestamp>-<hash> subdir
        #[arg(long)]
        root: bool,
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
        #[arg(short = 't', long = "type")]
        cue_type: Option<String>,
        /// Include ignored artifact types (e.g. tmp)
        #[arg(short = 'i', long)]
        include_gitignored: bool,
        /// Output as JSON
        #[arg(short = 'j', long)]
        json: bool,
        /// Parse and include YAML frontmatter in output (implies --json)
        #[arg(long)]
        frontmatter: bool,
        /// Filter by frontmatter field (repeatable, ANDed)
        ///
        /// Syntax: KEY[OP]VALUE where OP is =, !=, or ~= (substring match).
        /// Dot notation is supported for nested keys: meta.status=done
        ///
        /// Examples:
        ///   --filter status=todo
        ///   --filter "status!=done"
        ///   --filter "title~=report"
        ///   --filter status=active --filter priority=high
        #[arg(long = "filter", value_name = "EXPR", verbatim_doc_comment)]
        filters: Vec<Filter>,
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
    /// Manage cue configuration
    Config {
        #[command(subcommand)]
        command: ConfigCommands,
    },
    /// Manage registered projects in the project store
    Project {
        #[command(subcommand)]
        command: ProjectCommands,
    },
}

#[derive(Subcommand)]
pub enum ConfigCommands {
    /// Show the resolved configuration as JSON
    Show,
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
        #[arg(long, conflicts_with_all = &["title", "body", "found", "decided", "open"])]
        file: Option<String>,
    },
    /// List log entries
    List {
        /// List log for a specific branch instead of current
        #[arg(long)]
        branch: Option<String>,
    },
}

#[derive(Subcommand)]
pub enum ProjectCommands {
    /// Register a path in the project store (defaults to cwd)
    Add {
        /// Path to register (defaults to current directory)
        #[arg(long)]
        path: Option<String>,
    },
    /// Remove a path or key from the project store
    Remove {
        /// Path to remove (defaults to current directory)
        #[arg(long, conflicts_with = "key")]
        path: Option<String>,
        /// Remove all paths for this project key
        #[arg(long, conflicts_with = "path")]
        key: Option<String>,
    },
    /// List all registered projects
    List,
}
