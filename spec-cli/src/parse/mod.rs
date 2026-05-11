pub mod anchors;
pub mod blocks;
pub mod config;
pub mod frontmatter;
pub mod redirects;
pub mod references;

use std::path::Path;

use anyhow::{Context, Result};

use crate::model::document::SpecDocument;

/// Parse a single `.spec.md` file into a `SpecDocument`.
pub fn parse_document(path: &Path) -> Result<SpecDocument> {
    let content =
        std::fs::read_to_string(path).with_context(|| format!("reading {}", path.display()))?;

    let (yaml, body, body_start_line) = frontmatter::split_frontmatter(&content)
        .with_context(|| format!("parsing frontmatter in {}", path.display()))?;

    let (universal, type_fields, _warnings) = frontmatter::parse_frontmatter(yaml)
        .with_context(|| format!("parsing YAML in {}", path.display()))?;

    let typed_blocks = blocks::extract_blocks(body, body_start_line);
    let refs = references::extract_references(body, body_start_line);

    Ok(SpecDocument {
        universal,
        type_fields,
        body_raw: body.to_string(),
        blocks: typed_blocks,
        references: refs,
        source_path: path.to_path_buf(),
        body_start_line,
    })
}
