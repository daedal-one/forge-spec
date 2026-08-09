use std::path::PathBuf;

use clap::{Parser, Subcommand, ValueEnum};

#[derive(Parser)]
#[command(name = "spec", about = "Specs format toolchain", version)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,

    /// Path to .specs/ directory (default: auto-detect from cwd)
    #[arg(long, global = true)]
    pub specs_dir: Option<PathBuf>,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Initialize a forge-spec tree in the current project
    Init,
    /// Scaffold a new spec from a per-type template
    New {
        /// Entity type (req, inv, ifc, adr, glo, topic, scn)
        #[arg(value_parser = parse_entity_type)]
        entity_type: String,
        /// Namespace/slug (e.g. auth/session-expiry)
        slug: String,
    },
    /// Run all validation checks
    Lint {
        /// Specific paths to lint (default: entire .specs/ directory)
        paths: Vec<PathBuf>,
        /// Fail when source symbols cannot be verified by a language server
        #[arg(long)]
        require_symbols: bool,
        /// Trust custom language-server commands from _lsp.toml
        #[arg(long)]
        allow_custom_lsp: bool,
    },
    /// Produce render bundles
    Render {
        /// Spec ID or query
        id_or_query: String,
        /// Render target
        #[arg(long, default_value = "human", value_enum)]
        target: RenderTarget,
        /// Traversal depth
        #[arg(long)]
        depth: Option<usize>,
        /// Ancestor detail level
        #[arg(long, default_value = "full")]
        ancestors: String,
        /// Descendant detail level
        #[arg(long, default_value = "summary")]
        descendants: String,
        /// Include resolved source references
        #[arg(long)]
        include_source: bool,
    },
    /// Emit DOT for the requested graph
    Graph {
        /// Show the refinement graph
        #[arg(long)]
        refinement: bool,
        /// Show the categorization graph
        #[arg(long)]
        categorization: bool,
    },
    /// Regenerate or query commit history per spec
    History {
        /// Regenerate all history files
        #[arg(long)]
        update: bool,
        /// Query history for a specific spec ID
        id: Option<String>,
    },
    /// List direct refining children
    Children {
        /// Spec ID
        id: String,
    },
    /// List direct refined-by parents
    Ancestors {
        /// Spec ID
        id: String,
    },
    /// Clause-by-clause refinement-coverage report
    Coverage {
        /// Spec ID
        id: String,
    },
    /// List specs with no refinement relationships
    Orphans,
    /// Explain or apply composable format migrations and reference redirects
    Migrate {
        /// Print the composed changelog and migration instructions without changing files
        #[arg(long)]
        guide: bool,
        /// Guide render target
        #[arg(long, default_value = "human", value_enum)]
        target: RenderTarget,
        /// Override the detected source baseline
        #[arg(long)]
        from: Option<String>,
        /// Target baseline (default: latest supported by this CLI)
        #[arg(long)]
        to: Option<String>,
    },
    /// List code symbols in a repository-relative source file
    Symbols {
        /// Repository-relative source path
        path: String,
        /// Filter by symbol or qualified name
        #[arg(long)]
        query: Option<String>,
        /// Emit machine-readable JSON
        #[arg(long)]
        json: bool,
        /// Trust custom language-server commands from _lsp.toml
        #[arg(long)]
        allow_custom_lsp: bool,
    },
    /// Resolve a spec: URL to its canonical target
    Resolve {
        /// Full spec: URL
        reference: String,
        /// Emit machine-readable JSON
        #[arg(long)]
        json: bool,
        /// Trust custom language-server commands from _lsp.toml
        #[arg(long)]
        allow_custom_lsp: bool,
    },
    /// Run the forge-spec language server over standard I/O
    Lsp {
        /// Compatibility flag supplied by standard editor language clients
        #[arg(long, hide = true)]
        stdio: bool,
    },
    /// List open tasks (pending, in-progress, blocked).
    Todo {
        /// Filter by progress state (pending|in-progress|done|blocked|deferred)
        #[arg(long)]
        state: Option<String>,
        /// Filter to tasks refining the given REQ id (matches doc id, with or without #anchor)
        #[arg(long)]
        under: Option<String>,
        /// Show all tasks, including done/deferred (shorthand for `--state all`)
        #[arg(long)]
        all: bool,
    },
    /// Mark a task as in-progress.
    Start {
        /// TASK spec id (e.g. TASK:codon/session-actions)
        id: String,
    },
    /// Mark a task as done.
    Done {
        /// TASK spec id
        id: String,
    },
    /// Mark a task as blocked, optionally citing a blocker.
    Block {
        /// TASK spec id
        id: String,
        /// Optional blocker — another spec id, e.g. ADR:foo/bar
        #[arg(long)]
        on: Option<String>,
    },
    /// Mark a task as pending (clear in-progress / blocked / done).
    Reset {
        /// TASK spec id
        id: String,
    },
    /// Mark a task as deferred (out of scope for current iteration).
    Defer {
        /// TASK spec id
        id: String,
    },
    /// Mark a task as wontdo (intentionally not implemented; the parent
    /// clause stays in place for traceability but no work is planned).
    Wontdo {
        /// TASK spec id
        id: String,
    },
    /// Print a tree of all specs grouped by namespace, then type.
    Tree {
        /// Restrict to a single namespace prefix (e.g. `auth`)
        #[arg(long)]
        namespace: Option<String>,
        /// Restrict to a single entity type (e.g. `REQ`, `TASK`)
        #[arg(long, value_parser = parse_entity_type)]
        r#type: Option<String>,
        /// Disable colored output
        #[arg(long)]
        no_color: bool,
    },
    /// Interactive TUI to browse specs (ratatui).
    Explore,
    /// Print a shell completion script (bash, zsh, fish).
    Completions {
        /// Shell to generate completions for
        #[arg(value_enum)]
        shell: Shell,
    },
    /// Internal: emit machine-readable data for shell completion.
    #[command(hide = true)]
    #[command(name = "__complete")]
    Complete {
        /// What to list (currently only `ids`)
        what: String,
    },
}

#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum Shell {
    Bash,
    Zsh,
    Fish,
}

#[derive(Debug, Clone, ValueEnum)]
pub enum RenderTarget {
    Human,
    Agent,
}

pub fn parse_entity_type(s: &str) -> Result<String, String> {
    let normalized = s.to_lowercase();
    match normalized.as_str() {
        "req" | "requirement" => Ok("REQ".to_string()),
        "inv" | "invariant" => Ok("INV".to_string()),
        "ifc" | "interface" => Ok("IFC".to_string()),
        "adr" => Ok("ADR".to_string()),
        "glo" | "glossary" => Ok("GLO".to_string()),
        "topic" => Ok("TOPIC".to_string()),
        "scn" | "scenario" => Ok("SCN".to_string()),
        "task" => Ok("TASK".to_string()),
        _ => Err(format!("unknown entity type: {s}")),
    }
}
