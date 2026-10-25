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
    /// Print the active context
    Status {
        /// Set context by slug or <org>/<repo>/<slug> address; overrides $CUE_CONTEXT
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
            value_parser = ["task", "spec", "plan", "note", "trace", "bin", "review", "tmp"]
        )]
        cue_type: String,
        /// Set context by slug or <org>/<repo>/<slug> address; overrides $CUE_CONTEXT
        #[arg(long)]
        context: Option<String>,
        /// Session that initiated this creation
        #[arg(long)]
        session_id: Option<String>,
        /// Agent harness that owns the session ID
        #[arg(long)]
        harness: Option<String>,
        /// Overwrite existing file
        #[arg(long)]
        force: bool,
    },

    /// List artifacts for a scope
    List {
        /// Set context by slug or <org>/<repo>/<slug> address; overrides $CUE_CONTEXT
        #[arg(long)]
        context: Option<String>,
        /// Filter by artifact type; repeat to match any selected type
        #[arg(short = 't', long = "type")]
        cue_types: Vec<String>,
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
    /// Render artifacts as framed blocks for injection into a session
    Render {
        /// Artifact paths; use "-" to read newline-delimited paths from stdin
        #[arg(value_name = "ENTRY")]
        entries: Vec<String>,
        /// Set context by slug or <org>/<repo>/<slug> address; overrides $CUE_CONTEXT
        #[arg(long)]
        context: Option<String>,
    },
    /// Record and inspect context activity
    Activity {
        #[command(subcommand)]
        command: ActivityCommands,
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
pub enum ActivityCommands {
    /// Record an immutable activity event
    Add {
        #[arg(long)]
        context: Option<String>,
        /// Producer-defined kind: lowercase snake_case, starting with a
        /// letter, at most 64 bytes. Kinds cue records natively are reserved.
        #[arg(long, value_parser = |s: &str| crate::activity::Kind::producer(s).map_err(|e| e.to_string()))]
        kind: crate::activity::Kind,
        #[arg(long)]
        details: String,
        #[arg(long)]
        session_id: Option<String>,
        #[arg(long)]
        harness: Option<String>,
        #[arg(long)]
        host: Option<String>,
    },
    /// List activity in chronological order
    List {
        #[arg(long)]
        context: Option<String>,
        /// Select the latest N matching events
        #[arg(long)]
        limit: Option<usize>,
        /// Include timestamps at or after these Unix milliseconds
        #[arg(long)]
        since: Option<u64>,
        #[arg(long)]
        json: bool,
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
    /// List contexts in the current repository scope, or in every scope
    List {
        /// Output structured JSON instead of canonical addresses
        #[arg(long)]
        json: bool,
        /// List only the pinned working set
        #[arg(long)]
        pinned: bool,
        /// Include archived contexts, which are otherwise left out
        #[arg(long)]
        archived: bool,
        /// Query repo, store, or a configured name (defaults to config, then repo)
        #[arg(long, value_name = "SCOPE")]
        scope: Option<String>,
        /// Order by latest log activity instead of canonical address
        #[arg(long, value_enum)]
        sort: Option<ContextSort>,
        /// Filter by context metadata field (repeatable, ANDed)
        ///
        /// Syntax: KEY[OP]VALUE where OP is =, !=, or ~= (substring match).
        /// KEY names a context.md frontmatter field, with dot notation for
        /// nested keys. Filtering is applied before --sort and --limit.
        ///
        /// Examples:
        ///   --filter kind=coord
        ///   --filter "mode!=review"
        ///   --filter "title~=release"
        ///   --filter parent=acme/widgets/program
        #[arg(long = "filter", value_name = "EXPR", verbatim_doc_comment)]
        filters: Vec<Filter>,
        /// Keep only the first N contexts of the resulting order
        #[arg(long, value_name = "N")]
        limit: Option<usize>,
    },
    /// Show contexts as a tree built from their parent edges
    Tree {
        /// Output nested JSON: { roots: [node], diagnostics: [...] }
        #[arg(long)]
        json: bool,
        /// Build the tree from the pinned working set only
        #[arg(long)]
        pinned: bool,
        /// Include archived contexts, which are otherwise left out
        #[arg(long)]
        archived: bool,
        /// Query repo, store, or a configured name (defaults to config, then repo)
        #[arg(long, value_name = "SCOPE")]
        scope: Option<String>,
        /// Show only the subtree under this context (slug or <org>/<repo>/<slug>)
        #[arg(long, value_name = "SELECTOR")]
        root: Option<String>,
        /// Order roots and siblings by the newest log activity in their subtree
        #[arg(long, value_enum)]
        sort: Option<ContextSort>,
        /// Filter by context metadata field (repeatable, ANDed), as context list
        #[arg(long = "filter", value_name = "EXPR")]
        filters: Vec<Filter>,
    },
    /// Add a context to the operator's working set
    Pin {
        /// Context slug in the current repository scope, or a full
        /// '<org>/<repo>/<slug>' address
        context: String,
    },
    /// Remove a context from the operator's working set
    Unpin {
        /// Context slug in the current repository scope, or a full
        /// '<org>/<repo>/<slug>' address
        context: String,
    },
    /// List the pinned contexts in the operator's working set
    Pins {
        /// Query repo, store, or a configured name (defaults to config, then repo)
        #[arg(long, value_name = "SCOPE")]
        scope: Option<String>,
    },
    /// Associate a context with a Git branch
    Switch {
        /// Context slug to associate with the branch
        slug: String,
        /// Branch to configure; defaults to the current branch
        #[arg(long)]
        branch: Option<String>,
    },
    /// Mark contexts archived, leaving them out of listings by default
    Archive {
        /// Context slugs in the current repository scope, or full
        /// '<org>/<repo>/<slug>' addresses
        #[arg(required = true)]
        contexts: Vec<String>,
    },
    /// Clear the archived mark from contexts
    Unarchive {
        /// Context slugs in the current repository scope, or full
        /// '<org>/<repo>/<slug>' addresses
        #[arg(required = true)]
        contexts: Vec<String>,
    },
    /// Remove a context association from a Git branch
    Unset {
        /// Branch to configure; defaults to the current branch
        #[arg(long)]
        branch: Option<String>,
    },
}

/// The orderings a context listing can be asked for, beyond the canonical
/// address order it uses when no ordering is requested.
///
/// Recency is the only member: it is the one ordering that cannot be derived
/// from what a listing already reads, because it reads each context's log.
#[derive(Clone, Copy, PartialEq, Eq, clap::ValueEnum)]
pub enum ContextSort {
    /// Newest log entry first; contexts with no log entry last
    Recency,
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
        /// Canonical address of a trace artifact in this context
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
        /// Set context by slug or <org>/<repo>/<slug> address; overrides $CUE_CONTEXT
        #[arg(long)]
        context: Option<String>,
    },
    /// List log entries
    List {
        /// Set context by slug or <org>/<repo>/<slug> address; overrides $CUE_CONTEXT
        #[arg(long)]
        context: Option<String>,
        /// Output format
        #[arg(long, value_enum, default_value = "json")]
        format: LogFormat,
        /// Keep only the newest N entries
        #[arg(long, value_name = "N")]
        limit: Option<usize>,
    },
}

#[derive(Clone, Copy, clap::ValueEnum)]
pub enum LogFormat {
    Json,
    Md,
}
