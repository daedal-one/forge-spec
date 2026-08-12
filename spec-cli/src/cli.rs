use std::path::PathBuf;

use clap::{Args, Parser, Subcommand, ValueEnum};

#[derive(Parser)]
#[command(
    name = "spec",
    about = "Repository-native specifications for humans and coding agents",
    version,
    after_help = "Agent workflow:\n  spec impact --base HEAD~1 --target agent\n  spec render REQ:auth/session --target agent --include-source\n  spec change batch --from changes.json --dry-run\n  spec task start TASK:auth/update-session\n  spec lint"
)]
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
    /// Scaffold a new specification
    New {
        #[arg(value_parser = parse_entity_type)]
        entity_type: String,
        slug: String,
    },
    /// Run all validation checks
    Lint {
        paths: Vec<PathBuf>,
        #[arg(long)]
        require_symbols: bool,
        #[arg(long)]
        allow_custom_lsp: bool,
    },
    /// Produce human or agent render bundles
    Render {
        id_or_query: String,
        #[arg(long, default_value = "human", value_enum)]
        target: RenderTarget,
        #[arg(long)]
        depth: Option<usize>,
        #[arg(long, default_value = "full")]
        ancestors: String,
        #[arg(long, default_value = "summary")]
        descendants: String,
        #[arg(long)]
        include_source: bool,
    },
    /// Measure cascading specification and implementation impact
    Impact {
        subject: Option<String>,
        #[arg(long)]
        base: Option<String>,
        #[arg(long)]
        head: Option<String>,
        #[arg(long, default_value = "human", value_enum)]
        target: RenderTarget,
    },
    /// Interactive terminal explorer
    Explore,
    /// Inspect workspace structure and references
    Inspect(InspectArgs),
    /// Apply typed, validated document changes
    Change(ChangeArgs),
    /// Rename a specification and all incoming references
    Rename { id: String, new_id: String },
    /// Change document lifecycle state
    Lifecycle(LifecycleArgs),
    /// Manage structural and informational relationships
    Relation(RelationArgs),
    /// Query and update implementation tasks
    Task(TaskArgs),
    /// Query or rebuild derived Git history
    History(HistoryArgs),
    /// Plan or apply document-format migrations
    Migrate(MigrateArgs),
    /// Run the forge-spec language server over standard I/O
    Lsp {
        #[arg(long, hide = true)]
        stdio: bool,
    },
    /// Generate shell completion scripts
    Completions {
        #[arg(value_enum)]
        shell: Shell,
    },
    /// Internal machine-readable completion provider
    #[command(hide = true, name = "__complete")]
    Complete {
        what: String,
        #[arg(allow_hyphen_values = true, trailing_var_arg = true)]
        context: Vec<String>,
    },
}

#[derive(Args)]
#[command(arg_required_else_help = true)]
pub struct InspectArgs {
    #[command(subcommand)]
    pub command: InspectCommands,
}

#[derive(Subcommand)]
pub enum InspectCommands {
    /// Print the project-rooted specification tree
    Tree {
        #[arg(long)]
        namespace: Option<String>,
        #[arg(long, value_parser = parse_entity_type)]
        r#type: Option<String>,
        #[arg(long)]
        no_color: bool,
    },
    /// Emit one typed graph as DOT
    Graph {
        #[arg(value_enum, default_value = "hierarchy")]
        view: GraphView,
    },
    /// Show incoming and outgoing relationships for one specification
    Relations { id: String },
    /// Show clause-by-clause refinement coverage
    Coverage { id: String },
    /// List specifications without refinement relationships
    Orphans,
    /// Resolve a spec: URL to its canonical target
    Resolve {
        reference: String,
        #[arg(long)]
        json: bool,
        #[arg(long)]
        allow_custom_lsp: bool,
    },
    /// List symbols from a repository-relative source path
    Symbols {
        path: String,
        #[arg(long)]
        query: Option<String>,
        #[arg(long)]
        json: bool,
        #[arg(long)]
        allow_custom_lsp: bool,
    },
}

#[derive(Args)]
#[command(arg_required_else_help = true)]
pub struct ChangeArgs {
    #[command(subcommand)]
    pub command: ChangeCommands,
}

#[derive(Subcommand)]
pub enum ChangeCommands {
    /// Replace a document summary
    Summary(SummaryArgs),
    /// Add or remove an owner
    Owner(OwnerArgs),
    /// Set or clear pinned_at
    Pin(PinArgs),
    /// Change requirement-specific metadata
    Requirement(RequirementArgs),
    /// Change invariant-specific metadata
    Invariant(InvariantArgs),
    /// Change interface-specific metadata
    Interface(InterfaceArgs),
    /// Change ADR-specific metadata
    Adr(AdrArgs),
    /// Change headings, sections, typed blocks, and clauses
    Content(ContentArgs),
    /// Apply a versioned JSON change batch
    Batch {
        /// JSON request path, or - for standard input
        #[arg(long)]
        from: String,
        /// Validate and print the deterministic plan without writing
        #[arg(long)]
        dry_run: bool,
    },
}

#[derive(Args)]
#[command(arg_required_else_help = true)]
pub struct SummaryArgs {
    #[command(subcommand)]
    pub command: SummaryCommands,
}

#[derive(Subcommand)]
pub enum SummaryCommands {
    Replace { id: String, value: String },
}

#[derive(Args)]
#[command(arg_required_else_help = true)]
pub struct OwnerArgs {
    #[command(subcommand)]
    pub command: OwnerCommands,
}

#[derive(Subcommand)]
pub enum OwnerCommands {
    Add { id: String, owner: String },
    Remove { id: String, owner: String },
}

#[derive(Args)]
#[command(arg_required_else_help = true)]
pub struct PinArgs {
    #[command(subcommand)]
    pub command: PinCommands,
}

#[derive(Subcommand)]
pub enum PinCommands {
    Set { id: String, value: String },
    Clear { id: String },
}

#[derive(Args)]
#[command(arg_required_else_help = true)]
pub struct RequirementArgs {
    #[command(subcommand)]
    pub command: RequirementCommands,
}

#[derive(Subcommand)]
pub enum RequirementCommands {
    Level { id: String, level: String },
    KindSet { id: String, kind: String },
    KindClear { id: String },
    Monotonicity { id: String, value: bool },
}

#[derive(Args)]
#[command(arg_required_else_help = true)]
pub struct InvariantArgs {
    #[command(subcommand)]
    pub command: InvariantCommands,
}

#[derive(Subcommand)]
pub enum InvariantCommands {
    EnforcementAdd { id: String, value: String },
    EnforcementRemove { id: String, value: String },
    RequirementAdd { id: String, requirement: String },
    RequirementRemove { id: String, requirement: String },
}

#[derive(Args)]
#[command(arg_required_else_help = true)]
pub struct InterfaceArgs {
    #[command(subcommand)]
    pub command: InterfaceCommands,
}

#[derive(Subcommand)]
pub enum InterfaceCommands {
    Stability { id: String, stability: String },
    ConsumerAdd { id: String, consumer: String },
    ConsumerRemove { id: String, consumer: String },
    ProviderAdd { id: String, provider: String },
    ProviderRemove { id: String, provider: String },
}

#[derive(Args)]
#[command(arg_required_else_help = true)]
pub struct AdrArgs {
    #[command(subcommand)]
    pub command: AdrCommands,
}

#[derive(Subcommand)]
pub enum AdrCommands {
    DecisionDate { id: String, value: String },
    DecisionMakerAdd { id: String, owner: String },
    DecisionMakerRemove { id: String, owner: String },
}

#[derive(Args)]
#[command(arg_required_else_help = true)]
pub struct ContentArgs {
    #[command(subcommand)]
    pub command: ContentCommands,
}

#[derive(Subcommand)]
pub enum ContentCommands {
    TitleReplace {
        id: String,
        value: String,
    },
    SectionReplace {
        id: String,
        #[arg(long, required = true, num_args = 1..)]
        heading: Vec<String>,
        #[arg(long)]
        markdown: String,
    },
    BlockAdd {
        id: String,
        #[arg(long, required = true, num_args = 1..)]
        heading: Vec<String>,
        #[arg(long)]
        kind: String,
        #[arg(long)]
        block: String,
        #[arg(long)]
        level: Option<String>,
        #[arg(long)]
        markdown: String,
    },
    BlockReplace {
        id: String,
        block: String,
        #[arg(long)]
        markdown: String,
    },
    BlockRemove {
        id: String,
        block: String,
    },
    ClauseAdd {
        id: String,
        block: String,
        clause: String,
        #[arg(long)]
        markdown: String,
    },
    ClauseReplace {
        id: String,
        block: String,
        clause: String,
        #[arg(long)]
        markdown: String,
    },
    ClauseRemove {
        id: String,
        block: String,
        clause: String,
    },
}

#[derive(Args)]
#[command(arg_required_else_help = true)]
pub struct LifecycleArgs {
    #[command(subcommand)]
    pub command: LifecycleCommands,
}

#[derive(Subcommand)]
pub enum LifecycleCommands {
    Draft { id: String },
    Accept { id: String },
    Deprecate { id: String },
    Supersede { id: String, replacement: String },
}

#[derive(Args)]
#[command(arg_required_else_help = true)]
pub struct RelationArgs {
    #[command(subcommand)]
    pub command: RelationCommands,
}

#[derive(Subcommand)]
pub enum RelationCommands {
    Refine {
        id: String,
        target: String,
        #[arg(long)]
        aspect: Vec<String>,
    },
    Unrefine {
        id: String,
        target: String,
    },
    Categorize {
        id: String,
        topic: String,
    },
    Uncategorize {
        id: String,
        topic: String,
    },
    Relate {
        id: String,
        target: String,
    },
    Unrelate {
        id: String,
        target: String,
    },
}

#[derive(Args)]
#[command(arg_required_else_help = true)]
pub struct TaskArgs {
    #[command(subcommand)]
    pub command: TaskCommands,
}

#[derive(Subcommand)]
pub enum TaskCommands {
    List {
        #[arg(long, value_enum)]
        state: Option<TaskState>,
        #[arg(long)]
        under: Option<String>,
        #[arg(long)]
        all: bool,
    },
    Start {
        id: String,
    },
    Done {
        id: String,
    },
    Block {
        id: String,
        #[arg(long)]
        on: Vec<String>,
    },
    Reset {
        id: String,
    },
    Defer {
        id: String,
    },
    Wontdo {
        id: String,
    },
    Assign {
        id: String,
        assignee: String,
    },
    Unassign {
        id: String,
    },
    Schedule {
        id: String,
        eta: String,
    },
    Unschedule {
        id: String,
    },
}

#[derive(Args)]
#[command(arg_required_else_help = true)]
pub struct HistoryArgs {
    #[command(subcommand)]
    pub command: HistoryCommands,
}

#[derive(Subcommand)]
pub enum HistoryCommands {
    Show { id: Option<String> },
    Rebuild,
}

#[derive(Args)]
#[command(arg_required_else_help = true)]
pub struct MigrateArgs {
    #[command(subcommand)]
    pub command: MigrateCommands,
}

#[derive(Subcommand)]
pub enum MigrateCommands {
    /// Print a migration plan; never writes
    Plan {
        #[arg(long, default_value = "human", value_enum)]
        target: RenderTarget,
        #[arg(long)]
        from: Option<String>,
        #[arg(long)]
        to: Option<String>,
    },
    /// Apply the selected migration route
    Apply {
        #[arg(long)]
        from: Option<String>,
        #[arg(long)]
        to: Option<String>,
    },
}

#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum GraphView {
    Hierarchy,
    Refinement,
    Categorization,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum TaskState {
    Pending,
    InProgress,
    Done,
    Blocked,
    Deferred,
    Wontdo,
    All,
}

impl TaskState {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::InProgress => "in-progress",
            Self::Done => "done",
            Self::Blocked => "blocked",
            Self::Deferred => "deferred",
            Self::Wontdo => "wontdo",
            Self::All => "all",
        }
    }
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

pub fn parse_entity_type(value: &str) -> Result<String, String> {
    let normalized = value.to_lowercase();
    match normalized.as_str() {
        "project" => Err("PROJECT is the singleton root created by `spec init`".to_string()),
        "req" | "requirement" => Ok("REQ".to_string()),
        "inv" | "invariant" => Ok("INV".to_string()),
        "ifc" | "interface" => Ok("IFC".to_string()),
        "adr" => Ok("ADR".to_string()),
        "glo" | "glossary" => Ok("GLO".to_string()),
        "topic" => Ok("TOPIC".to_string()),
        "scn" | "scenario" => Ok("SCN".to_string()),
        "task" => Ok("TASK".to_string()),
        _ => Err(format!("unknown entity type: {value}")),
    }
}
