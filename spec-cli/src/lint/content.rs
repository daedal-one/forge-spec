use regex::Regex;
use std::sync::LazyLock;

use crate::model::block::BlockKind;
use crate::model::document::SpecDocument;

use super::diagnostic::Diagnostic;

static RFC2119_KEYWORD: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\b(MUST|SHOULD|MAY)\b").unwrap());

/// R016: Warn when a file contains more than a threshold number of typed blocks.
pub fn check_multi_entity(doc: &SpecDocument, threshold: usize) -> Vec<Diagnostic> {
    if doc.blocks.len() > threshold {
        vec![Diagnostic::warning(
            "R016",
            format!(
                "file contains {} typed blocks (threshold: {threshold})",
                doc.blocks.len()
            ),
            doc.source_path.clone(),
        )]
    } else {
        vec![]
    }
}

/// R017: Requirement blocks should contain at least one RFC 2119 keyword.
pub fn check_rfc2119_discipline(doc: &SpecDocument) -> Vec<Diagnostic> {
    let mut diags = Vec::new();

    for block in &doc.blocks {
        if block.kind == BlockKind::Requirement {
            // Check the block-level level attribute first
            let has_level_attr = block.level.is_some();
            let has_keyword = RFC2119_KEYWORD.is_match(&block.body);

            if !has_level_attr && !has_keyword {
                diags.push(
                    Diagnostic::warning(
                        "R017",
                        format!(
                            "requirement block '{}' contains no RFC 2119 keyword (MUST/SHOULD/MAY)",
                            block.id
                        ),
                        doc.source_path.clone(),
                    )
                    .at_line(block.start_line),
                );
            }
        }
    }

    diags
}
