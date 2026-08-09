use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use super::block::TypedBlock;
use super::frontmatter::{TypeSpecificFields, UniversalFrontmatter};
use super::reference::LocatedReference;

/// A fully parsed spec document.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpecDocument {
    pub universal: UniversalFrontmatter,
    pub type_fields: TypeSpecificFields,
    pub body_raw: String,
    pub blocks: Vec<TypedBlock>,
    pub references: Vec<LocatedReference>,
    pub source_path: PathBuf,
    /// Line number where the body starts (after frontmatter closing `---`).
    pub body_start_line: usize,
}

impl SpecDocument {
    /// The full document ID as a string.
    pub fn id_str(&self) -> String {
        self.universal.id.to_string()
    }

    /// Collect all anchors defined in this document (block IDs + clause anchors).
    pub fn anchors(&self) -> Vec<String> {
        let mut result = Vec::new();
        for block in &self.blocks {
            result.push(block.id.clone());
            for clause in &block.clauses {
                result.push(clause.id.clone());
            }
        }
        result
    }
}
