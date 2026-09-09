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
    /// Run as if started in <PATH> instead of the current directory.
    /// Mirrors the git -C convention.
    #[arg(short = 'C', long = "dir", value_name = "PATH", global = true)]
    pub dir: Option<std::path::PathBuf>,

    /// Root of the central cue store; overrides $CUE_STORE
    #[arg(long, value_name = "PATH", global = true)]
    pub store: Option<std::path::PathBuf>,

    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Initialize agent artifacts directory structure
    Init,
    /// Print the active context
    Status {
        /// Set context; overrides $CUE_CONTEXT and branch.<name>.cue-context
        #[arg(long)]
        context: Option<String>,
        /// Output structured JSON instead of human-readable text
        #[arg(long)]
        json: bool,
    },
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
        /// Type of artifact
        #[arg(
            short = 't',
            long = "type",
            default_value = "spec",
            value_parser = ["task", "spec", "plan", "note", "trace", "bin", "tmp"]
        )]
        cue_type: String,
        /// Set context; overrides $CUE_CONTEXT and branch.<name>.cue-context
        #[arg(long)]
        context: Option<String>,
        /// Group name for tmp artifacts
        #[arg(long, value_name = "NAME")]
        group: Option<String>,
        /// Overwrite existing file
        #[arg(long)]
        force: bool,
    },

    /// List artifacts for a scope
    List {
        /// Set context; overrides $CUE_CONTEXT and branch.<name>.cue-context
        #[arg(long)]
        context: Option<String>,
        /// Filter by artifact type
        #[arg(short = 't', long = "type")]
        cue_type: Option<String>,
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
}

#[derive(Subcommand)]
pub enum ContextCommands {
    /// Create a context in the repository's central store
    Create {
        /// Immutable context slug
        name: String,
        /// Presentation name
        #[arg(long)]
        title: Option<String>,
        /// What ends this context
        #[arg(long, value_enum, default_value = "work")]
        kind: ContextKind,
        /// Advisory session mode
        #[arg(long, value_enum)]
        mode: Option<ContextMode>,
        /// One-line listing description
        #[arg(long)]
        description: Option<String>,
        /// Canonical address of the parent context
        #[arg(long)]
        parent: Option<String>,
        /// Canonical address of a related context or artifact
        #[arg(long = "ref")]
        refs: Vec<String>,
    },
    /// List contexts in the current repository scope
    List {
        /// Output structured JSON instead of context slugs
        #[arg(long)]
        json: bool,
    },
    /// Concatenate context artifacts for injection into a session
    Render {
        /// Artifact paths relative to the context directory
        #[arg(value_name = "ENTRY")]
        entries: Vec<String>,
        /// Set context; overrides $CUE_CONTEXT and branch.<name>.cue-context
        #[arg(long)]
        context: Option<String>,
    },
}

#[derive(Clone, Copy, clap::ValueEnum)]
pub enum ContextKind {
    Work,
    Coord,
    Reference,
}

impl ContextKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Work => "work",
            Self::Coord => "coord",
            Self::Reference => "reference",
        }
    }
}

#[derive(Clone, Copy, clap::ValueEnum)]
pub enum ContextMode {
    Research,
    Design,
    Build,
    Review,
    Learn,
}

impl ContextMode {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Research => "research",
            Self::Design => "design",
            Self::Build => "build",
            Self::Review => "review",
            Self::Learn => "learn",
        }
    }
}

#[derive(Subcommand)]
pub enum LogCommands {
    /// Add a new log entry
    Add {
        /// Entry title (required unless --file is used)
        #[arg(long)]
        title: Option<String>,
        /// Repository-relative or absolute reference to a trace artifact
        #[arg(long)]
        trace: Option<String>,
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
        #[arg(long, conflicts_with_all = &["title", "trace", "found", "decided", "open"])]
        file: Option<String>,
        /// Set context; overrides $CUE_CONTEXT and branch.<name>.cue-context
        #[arg(long)]
        context: Option<String>,
    },
    /// List log entries
    List {
        /// Set context; overrides $CUE_CONTEXT and branch.<name>.cue-context
        #[arg(long)]
        context: Option<String>,
        /// Output format
        #[arg(long, value_enum, default_value = "json")]
        format: LogFormat,
    },
}

#[derive(Clone, Copy, clap::ValueEnum)]
pub enum LogFormat {
    Json,
    Md,
}
