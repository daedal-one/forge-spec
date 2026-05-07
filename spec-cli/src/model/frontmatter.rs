use serde::Deserialize;

use super::id::{EntityType, SpecId};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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

/// Universal frontmatter present on all spec documents.
#[derive(Debug, Clone)]
pub struct UniversalFrontmatter {
    pub id: SpecId,
    pub entity_type: EntityType,
    pub status: Status,
    pub version: String,
    pub summary: Option<String>,
    pub owners: Vec<String>,
    pub pinned_at: Option<String>,
    pub related: Vec<String>,
    pub supersedes: Option<String>,
    pub superseded_by: Option<String>,
}

/// Type-specific frontmatter fields.
#[derive(Debug, Clone)]
pub enum TypeSpecificFields {
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
}

/// Raw YAML frontmatter — flat struct for deserialization before validation.
#[derive(Debug, Deserialize)]
pub struct RawFrontmatter {
    pub id: String,
    #[serde(rename = "type")]
    pub entity_type: String,
    pub status: Option<String>,
    pub version: Option<String>,
    pub summary: Option<String>,
    pub owners: Option<Vec<String>>,
    pub pinned_at: Option<String>,
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
}
