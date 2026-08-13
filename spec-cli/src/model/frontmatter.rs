use serde::{Deserialize, Serialize};

use super::id::{EntityType, SpecId};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Status {
    Draft,
    Accepted,
    Deprecated,
    Superseded,
}

impl Status {
    pub fn from_str_val(s: &str) -> Option<Self> {
        match s {
            "draft" => Some(Self::Draft),
            "accepted" => Some(Self::Accepted),
            "deprecated" => Some(Self::Deprecated),
            "superseded" => Some(Self::Superseded),
            _ => None,
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Draft => "draft",
            Self::Accepted => "accepted",
            Self::Deprecated => "deprecated",
            Self::Superseded => "superseded",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum Level {
    Must,
    Should,
    May,
    Info,
}

impl Level {
    pub fn from_str_val(s: &str) -> Option<Self> {
        match s {
            "MUST" => Some(Self::Must),
            "SHOULD" => Some(Self::Should),
            "MAY" => Some(Self::May),
            "INFO" => Some(Self::Info),
            _ => None,
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Must => "MUST",
            Self::Should => "SHOULD",
            Self::May => "MAY",
            Self::Info => "INFO",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Stability {
    Experimental,
    Stable,
    Frozen,
}

impl Stability {
    pub fn from_str_val(s: &str) -> Option<Self> {
        match s {
            "experimental" => Some(Self::Experimental),
            "stable" => Some(Self::Stable),
            "frozen" => Some(Self::Frozen),
            _ => None,
        }
    }
}

/// Implementation-lifecycle state for TASK entities.
///
/// Distinct from [`Status`], which is the document lifecycle (draft, accepted,
/// deprecated, superseded). A task can be `accepted` as a document while still
/// being `pending` as work to do.
///
/// `Deferred` and `WontDo` look similar but differ in intent:
/// - `Deferred` means "out of scope for the current iteration — revisit later"
/// - `WontDo` means "we've decided not to implement this; the clause stays in
///   the parent REQ for traceability but the work will not happen unless
///   requirements change". Use `WontDo` when a task is rendered redundant by
///   another design choice or a working alternative.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Progress {
    Pending,
    InProgress,
    Done,
    Blocked,
    Deferred,
    WontDo,
}

impl Progress {
    pub fn from_str_val(s: &str) -> Option<Self> {
        match s {
            "pending" => Some(Self::Pending),
            "in-progress" | "in_progress" => Some(Self::InProgress),
            "done" => Some(Self::Done),
            "blocked" => Some(Self::Blocked),
            "deferred" => Some(Self::Deferred),
            "wontdo" | "wont-do" | "wont_do" => Some(Self::WontDo),
            _ => None,
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::InProgress => "in-progress",
            Self::Done => "done",
            Self::Blocked => "blocked",
            Self::Deferred => "deferred",
            Self::WontDo => "wontdo",
        }
    }

    pub fn is_open(&self) -> bool {
        matches!(self, Self::Pending | Self::InProgress | Self::Blocked)
    }
}

/// Universal frontmatter present on all spec documents.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UniversalFrontmatter {
    pub id: SpecId,
    pub entity_type: EntityType,
    pub status: Status,
    pub summary: Option<String>,
    pub owners: Vec<String>,
    pub pinned_at: Option<String>,
    pub implemented: Option<String>,
    pub related: Vec<String>,
    pub supersedes: Option<String>,
    pub superseded_by: Option<String>,
}

/// Type-specific frontmatter fields.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TypeSpecificFields {
    Project,
    Requirement {
        level: Level,
        refines: Vec<String>,
        aspects: Vec<String>,
        categorized_under: Vec<String>,
        kind: Option<String>,
        level_monotonic: bool,
    },
    Invariant {
        enforcement: Vec<String>,
        applies_to: Vec<String>,
    },
    Interface {
        consumed_by: Vec<String>,
        provided_by: Vec<String>,
        stability: Stability,
    },
    Adr {
        decision_date: String,
        decided_by: Vec<String>,
    },
    Glossary,
    Topic,
    Scenario,
    Task {
        progress: Progress,
        refines: Vec<String>,
        aspects: Vec<String>,
        assignee: Option<String>,
        eta: Option<String>,
        blocked_by: Vec<String>,
        categorized_under: Vec<String>,
    },
}

/// Raw YAML frontmatter — flat struct for deserialization before validation.
#[derive(Debug, Deserialize)]
pub struct RawFrontmatter {
    pub id: String,
    #[serde(rename = "type")]
    pub entity_type: String,
    pub status: Option<String>,
    pub summary: Option<String>,
    pub owners: Option<Vec<String>>,
    pub pinned_at: Option<String>,
    pub implemented: Option<String>,
    pub related: Option<Vec<String>>,
    pub supersedes: Option<String>,
    pub superseded_by: Option<String>,
    // REQ fields
    pub level: Option<String>,
    pub refines: Option<Vec<String>>,
    pub aspects: Option<Vec<String>>,
    pub categorized_under: Option<Vec<String>>,
    pub kind: Option<String>,
    pub level_monotonic: Option<bool>,
    // INV fields
    pub enforcement: Option<Vec<String>>,
    pub applies_to: Option<Vec<String>>,
    // IFC fields
    pub consumed_by: Option<Vec<String>>,
    pub provided_by: Option<Vec<String>>,
    pub stability: Option<String>,
    // ADR fields
    pub decision_date: Option<String>,
    pub decided_by: Option<Vec<String>>,
    // TASK fields
    pub progress: Option<String>,
    pub assignee: Option<String>,
    pub eta: Option<String>,
    pub blocked_by: Option<Vec<String>>,
}
