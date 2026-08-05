use super::id::QualifiedAnchor;
use percent_encoding::{percent_decode_str, utf8_percent_encode, AsciiSet, CONTROLS};
use std::fmt;

const SYMBOL_SEGMENT_ENCODE_SET: &AsciiSet = &CONTROLS
    .add(b' ')
    .add(b'"')
    .add(b'#')
    .add(b'%')
    .add(b'/')
    .add(b'<')
    .add(b'>')
    .add(b'?')
    .add(b'`');

/// A repository-relative source target.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SourceTarget {
    /// The complete file.
    File,
    /// An inclusive, one-based line range.
    Lines { start: u32, end: u32 },
    /// A hierarchical code symbol. Segments correspond to nested
    /// `DocumentSymbol` names and are stored decoded in memory.
    Symbol { segments: Vec<String> },
}

/// A source file and the selector used within it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceReference {
    pub path: String,
    pub target: SourceTarget,
}

impl SourceReference {
    pub fn file(path: impl Into<String>) -> Self {
        Self {
            path: path.into(),
            target: SourceTarget::File,
        }
    }

    pub fn lines(path: impl Into<String>, start: u32, end: u32) -> Self {
        Self {
            path: path.into(),
            target: SourceTarget::Lines { start, end },
        }
    }

    pub fn symbol(path: impl Into<String>, segments: Vec<String>) -> Self {
        Self {
            path: path.into(),
            target: SourceTarget::Symbol { segments },
        }
    }

    pub fn symbol_name(&self) -> Option<String> {
        match &self.target {
            SourceTarget::Symbol { segments } => Some(segments.join("/")),
            _ => None,
        }
    }
}

/// A parsed `spec:` URL reference.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SpecReference {
    /// Reference to another spec or a clause within it.
    Spec(QualifiedAnchor),
    /// Reference to a source file (and optional line range).
    Source(SourceReference),
}

impl fmt::Display for SpecReference {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Spec(qa) => write!(f, "spec:{qa}"),
            Self::Source(source) => {
                write!(f, "spec:src:{}", source.path)?;
                match &source.target {
                    SourceTarget::File => Ok(()),
                    SourceTarget::Lines { start, end } if start == end => write!(f, ":{start}"),
                    SourceTarget::Lines { start, end } => write!(f, ":{start}-{end}"),
                    SourceTarget::Symbol { segments } => {
                        f.write_str("#symbol=")?;
                        for (index, segment) in segments.iter().enumerate() {
                            if index > 0 {
                                f.write_str("/")?;
                            }
                            write!(
                                f,
                                "{}",
                                utf8_percent_encode(segment, SYMBOL_SEGMENT_ENCODE_SET)
                            )?;
                        }
                        Ok(())
                    }
                }
            }
        }
    }
}

pub fn decode_symbol_segments(value: &str) -> Option<Vec<String>> {
    if value.is_empty() {
        return None;
    }
    value
        .split('/')
        .map(|segment| {
            if segment.is_empty() {
                return None;
            }
            percent_decode_str(segment)
                .decode_utf8()
                .ok()
                .map(|decoded| decoded.into_owned())
        })
        .collect()
}

/// A reference with its location in the source file.
#[derive(Debug, Clone)]
pub struct LocatedReference {
    pub reference: SpecReference,
    pub link_text: String,
    pub line: usize,
}
