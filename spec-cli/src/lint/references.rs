use std::collections::BTreeSet;
use std::path::PathBuf;

use pulldown_cmark::{Event, HeadingLevel, Options, Parser, Tag, TagEnd};

use crate::model::frontmatter::Status;
use crate::model::reference::SpecReference;
use crate::model::registry::SpecRegistry;

use super::diagnostic::Diagnostic;

/// R005: Referenced specs exist.
/// R006: Reference does not point at deprecated spec (warning).
/// R-redir: Reference traverses a redirect (info).
pub fn check_references(registry: &SpecRegistry) -> Vec<Diagnostic> {
    let mut diags = Vec::new();

    for doc in &registry.documents {
        for loc_ref in &doc.references {
            match &loc_ref.reference {
                SpecReference::Spec(qa) => {
                    let ref_str = qa.to_string();
                    let (exists, traversed) = registry.reference_exists(&ref_str);

                    if traversed {
                        let (resolved, _) = registry.resolve_redirect(&ref_str);
                        diags.push(
                            Diagnostic::info(
                                "R-redir",
                                format!("reference '{ref_str}' traverses redirect to '{resolved}'"),
                                doc.source_path.clone(),
                            )
                            .at_line(loc_ref.line),
                        );
                    }

                    if !exists {
                        diags.push(
                            Diagnostic::error(
                                "R005",
                                format!("dangling reference: '{ref_str}'"),
                                doc.source_path.clone(),
                            )
                            .at_line(loc_ref.line),
                        );
                    } else {
                        // Check if target is deprecated (R006)
                        let (resolved, _) = registry.resolve_redirect(&ref_str);
                        // Extract the doc ID part (strip anchor)
                        let doc_id = if let Some(pos) = resolved.find('#') {
                            &resolved[..pos]
                        } else {
                            &resolved
                        };
                        if let Some(target) = registry.get_by_id(doc_id) {
                            if target.universal.status == Status::Deprecated {
                                diags.push(
                                    Diagnostic::warning(
                                        "R006",
                                        format!(
                                            "reference to deprecated spec: '{}'",
                                            target.id_str()
                                        ),
                                        doc.source_path.clone(),
                                    )
                                    .at_line(loc_ref.line),
                                );
                            }
                        }
                    }
                }
                SpecReference::Source { .. } | SpecReference::KnowledgeBase { .. } => {
                    // Source and knowledge-base references are not validated
                    // against the registry (handled separately by check_kb_references)
                }
            }
        }
    }

    diags
}

/// R011: Summary present on referenced specs.
pub fn check_summary_on_referenced(registry: &SpecRegistry) -> Vec<Diagnostic> {
    let mut diags = Vec::new();

    // Collect all referenced spec IDs
    let mut referenced_ids: std::collections::BTreeSet<String> = std::collections::BTreeSet::new();
    for doc in &registry.documents {
        for loc_ref in &doc.references {
            if let SpecReference::Spec(qa) = &loc_ref.reference {
                referenced_ids.insert(qa.spec_id.to_string());
            }
        }

        // Also check refines references
        if let crate::model::frontmatter::TypeSpecificFields::Requirement { ref refines, .. } =
            doc.type_fields
        {
            for r in refines {
                // Strip anchor to get doc ID
                let doc_id = if let Some(pos) = r.find('#') {
                    &r[..pos]
                } else {
                    r.as_str()
                };
                referenced_ids.insert(doc_id.to_string());
            }
        }
    }

    for id in &referenced_ids {
        if let Some(target) = registry.get_by_id(id) {
            if target.universal.summary.is_none() {
                diags.push(Diagnostic::error(
                    "R011",
                    format!("referenced spec '{id}' is missing 'summary' field"),
                    target.source_path.clone(),
                ));
            }
        }
    }

    diags
}

/// R018: kb: file exists at resolved path.
/// R019: kb: heading slug found in target markdown (warning).
/// R020: Knowledge base not configured (info, emitted once).
pub fn check_kb_references(registry: &SpecRegistry) -> Vec<Diagnostic> {
    let mut diags = Vec::new();

    // Collect all kb: references across documents
    let mut has_kb_refs = false;
    for doc in &registry.documents {
        for loc_ref in &doc.references {
            if matches!(&loc_ref.reference, SpecReference::KnowledgeBase { .. }) {
                has_kb_refs = true;
                break;
            }
        }
        if has_kb_refs {
            break;
        }
    }

    if !has_kb_refs {
        return diags;
    }

    let kb_root = match &registry.kb_root {
        Some(root) => root,
        None => {
            // R020: emit once for the whole registry
            diags.push(Diagnostic::info(
                "R020",
                "knowledge base not configured; kb: references cannot be validated \
                 (add [knowledge_base] to .specs/_config.toml)",
                registry.specs_dir.join("_config.toml"),
            ));
            return diags;
        }
    };

    // Cache extracted headings per file to avoid re-parsing
    let mut heading_cache: std::collections::HashMap<PathBuf, BTreeSet<String>> =
        std::collections::HashMap::new();

    for doc in &registry.documents {
        for loc_ref in &doc.references {
            if let SpecReference::KnowledgeBase {
                ref path,
                ref heading,
            } = loc_ref.reference
            {
                let full_path = kb_root.join(path);

                // R018: file must exist
                if !full_path.exists() {
                    diags.push(
                        Diagnostic::error(
                            "R018",
                            format!("knowledge-base file not found: '{path}'"),
                            doc.source_path.clone(),
                        )
                        .at_line(loc_ref.line),
                    );
                    continue;
                }

                // R019: heading must exist (if specified)
                if let Some(heading_slug) = heading {
                    let headings = heading_cache
                        .entry(full_path.clone())
                        .or_insert_with(|| extract_heading_slugs(&full_path));

                    if !headings.contains(heading_slug.as_str()) {
                        diags.push(
                            Diagnostic::warning(
                                "R019",
                                format!(
                                    "heading '#{heading_slug}' not found in '{path}'"
                                ),
                                doc.source_path.clone(),
                            )
                            .at_line(loc_ref.line),
                        );
                    }
                }
            }
        }
    }

    diags
}

/// Extract heading slugs from a markdown file using GitHub-style slugification.
fn extract_heading_slugs(path: &PathBuf) -> BTreeSet<String> {
    let mut slugs = BTreeSet::new();

    let content = match std::fs::read_to_string(path) {
        Ok(c) => c,
        Err(_) => return slugs,
    };

    // Skip YAML frontmatter if present
    let body = if content.starts_with("---") {
        if let Some(end) = content[3..].find("\n---") {
            &content[end + 7..]
        } else {
            &content
        }
    } else {
        &content
    };

    let parser = Parser::new_ext(body, Options::all());
    let mut in_heading = false;
    let mut heading_text = String::new();

    for event in parser {
        match event {
            Event::Start(Tag::Heading { level, .. })
                if matches!(
                    level,
                    HeadingLevel::H1
                        | HeadingLevel::H2
                        | HeadingLevel::H3
                        | HeadingLevel::H4
                        | HeadingLevel::H5
                        | HeadingLevel::H6
                ) =>
            {
                in_heading = true;
                heading_text.clear();
            }
            Event::Text(text) if in_heading => {
                heading_text.push_str(&text);
            }
            Event::Code(code) if in_heading => {
                heading_text.push_str(&code);
            }
            Event::End(TagEnd::Heading(_)) if in_heading => {
                in_heading = false;
                slugs.insert(slugify_heading(&heading_text));
            }
            _ => {}
        }
    }

    slugs
}

/// GitHub-style heading slugification:
/// lowercase, replace spaces/runs-of-whitespace with hyphens,
/// strip non-alphanumeric except hyphens and underscores.
fn slugify_heading(text: &str) -> String {
    let lower = text.to_lowercase();
    let mut slug = String::with_capacity(lower.len());
    let mut prev_was_sep = false;

    for ch in lower.chars() {
        if ch.is_alphanumeric() || ch == '_' {
            slug.push(ch);
            prev_was_sep = false;
        } else if ch == ' ' || ch == '\t' || ch == '-' {
            if !prev_was_sep && !slug.is_empty() {
                slug.push('-');
                prev_was_sep = true;
            }
        }
        // Other characters are stripped
    }

    // Trim trailing hyphen
    if slug.ends_with('-') {
        slug.pop();
    }

    slug
}
