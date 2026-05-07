use super::id::QualifiedAnchor;
use std::fmt;

/// A parsed `spec:` URL reference.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SpecReference {
    /// Reference to another spec or a clause within it.
    Spec(QualifiedAnchor),
    /// Reference to a source file (and optional line range).
    Source {
        path: String,
        lines: Option<(u32, u32)>,
    },
}

impl fmt::Display for SpecReference {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Spec(qa) => write!(f, "spec:{qa}"),
            Self::Source { path, lines: None } => write!(f, "spec:src:{path}"),
            Self::Source {
                path,
                lines: Some((start, end)),
            } => write!(f, "spec:src:{path}:{start}-{end}"),
        }
    }
}

/// A reference with its location in the source file.
#[derive(Debug, Clone)]
pub struct LocatedReference {
    pub reference: SpecReference,
    pub link_text: String,
    pub line: usize,
}
