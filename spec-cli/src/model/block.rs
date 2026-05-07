use std::fmt;

/// Kinds of typed fenced divs recognized in spec bodies.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BlockKind {
    Requirement,
    Invariant,
    Interface,
    Clause,
    Assumption,
    NonGoal,
    Example,
    GlossaryEntry,
}

impl BlockKind {
    pub fn from_tag(s: &str) -> Option<Self> {
        match s {
            "requirement" => Some(Self::Requirement),
            "invariant" => Some(Self::Invariant),
            "interface" => Some(Self::Interface),
            "clause" => Some(Self::Clause),
            "assumption" => Some(Self::Assumption),
            "non-goal" => Some(Self::NonGoal),
            "example" => Some(Self::Example),
            "glossary-entry" => Some(Self::GlossaryEntry),
            _ => None,
        }
    }

    pub fn tag(&self) -> &'static str {
        match self {
            Self::Requirement => "requirement",
            Self::Invariant => "invariant",
            Self::Interface => "interface",
            Self::Clause => "clause",
            Self::Assumption => "assumption",
            Self::NonGoal => "non-goal",
            Self::Example => "example",
            Self::GlossaryEntry => "glossary-entry",
        }
    }
}

impl fmt::Display for BlockKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.tag())
    }
}

/// A clause anchor like `{#c-lifetime}` inside a typed block.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClauseAnchor {
    pub id: String,
    pub text: String,
    pub line: usize,
}

/// A typed fenced div extracted from a spec body.
#[derive(Debug, Clone)]
pub struct TypedBlock {
    pub kind: BlockKind,
    pub id: String,
    pub level: Option<String>,
    pub body: String,
    pub clauses: Vec<ClauseAnchor>,
    pub start_line: usize,
    pub end_line: usize,
}
