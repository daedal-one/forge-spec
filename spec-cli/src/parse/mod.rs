pub mod anchors;
pub mod blocks;
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

    parse_content(path, &content)
}

/// Parse an in-memory spec document. Used by the language server so unsaved
/// editor buffers receive the same parsing and lint behavior as files on disk.
pub fn parse_content(path: &Path, content: &str) -> Result<SpecDocument> {
    let (yaml, body, body_start_line) = frontmatter::split_frontmatter(content)
        .with_context(|| format!("parsing frontmatter in {}", path.display()))?;

    let (universal, type_fields, _warnings) = frontmatter::parse_frontmatter(yaml)
        .with_context(|| format!("parsing YAML in {}", path.display()))?;

    let typed_blocks = blocks::extract_blocks(body, body_start_line);
    let mut refs = references::extract_references(body, body_start_line);
    let mut declared_refs = universal
        .model
        .as_ref()
        .map(|m| m.references())
        .unwrap_or_default();
    if let Some(f) = &universal.flow {
        declared_refs.extend(f.references());
    }
    declared_refs.sort();
    declared_refs.dedup();
    for r in declared_refs {
        if let Some(reference) = references::parse_spec_url(&format!("spec:{r}")) {
            refs.push(crate::model::reference::LocatedReference {
                reference,
                link_text: r.clone(),
                line: yaml.lines().position(|line| line.contains(&r)).unwrap_or(0) + 2,
            });
        }
    }
    let mut anchors = Vec::new();
    if let Some(m) = &universal.model {
        anchors.extend(m.subjects.iter().map(|s| s.id.clone()));
        anchors.extend(m.interactions.iter().map(|s| s.id.clone()));
    }
    if let Some(f) = &universal.flow {
        f.anchors(&mut anchors);
    }
    let model_anchor_lines = anchors
        .into_iter()
        .map(|id| {
            let line = yaml
                .lines()
                .position(|line| {
                    let line = line.trim().trim_start_matches("- ");
                    line.strip_prefix("id:")
                        .is_some_and(|value| value.trim().trim_matches(['\"', '\'']) == id)
                        || line.contains(&format!("\"id\":\"{id}\""))
                })
                .unwrap_or(0)
                + 2;
            (id, line)
        })
        .collect();

    Ok(SpecDocument {
        universal,
        type_fields,
        body_raw: body.to_string(),
        blocks: typed_blocks,
        references: refs,
        source_path: path.to_path_buf(),
        body_start_line,
        model_anchor_lines,
    })
}
