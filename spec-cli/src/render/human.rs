use crate::model::document::SpecDocument;
use crate::model::frontmatter::TypeSpecificFields;
use crate::model::reference::SpecReference;
use crate::model::registry::SpecRegistry;

use super::scope::{DetailLevel, ScopedEntry};

/// Render a set of scoped entries as human-readable Markdown.
pub fn render_human(registry: &SpecRegistry, entries: &[ScopedEntry]) -> String {
    let mut out = String::new();

    for (i, entry) in entries.iter().enumerate() {
        if i > 0 {
            out.push_str("\n---\n\n");
        }

        let Some(doc) = registry.get_by_id(&entry.id) else {
            out.push_str(&format!("<!-- spec not found: {} -->\n", entry.id));
            continue;
        };

        match entry.detail {
            DetailLevel::Full => render_full(doc, &mut out),
            DetailLevel::Summary => render_summary(doc, &mut out),
            DetailLevel::IdOnly => {
                out.push_str(&format!("- {}\n", entry.id));
            }
            DetailLevel::None => {}
        }
    }

    out
}

fn render_full(doc: &SpecDocument, out: &mut String) {
    // Header table
    render_frontmatter_table(doc, out);
    out.push('\n');
    // Body
    out.push_str(&doc.body_raw);
    if !doc.body_raw.ends_with('\n') {
        out.push('\n');
    }
    // Knowledge-base references
    render_kb_refs(doc, out);
}

fn render_kb_refs(doc: &SpecDocument, out: &mut String) {
    let kb_refs: Vec<_> = doc
        .references
        .iter()
        .filter_map(|lr| match &lr.reference {
            SpecReference::KnowledgeBase { path, heading } => {
                Some((path.clone(), heading.clone(), lr.link_text.clone()))
            }
            _ => None,
        })
        .collect();

    if kb_refs.is_empty() {
        return;
    }

    out.push_str("\n### Knowledge base references\n\n");
    for (path, heading, link_text) in &kb_refs {
        let display = if link_text.is_empty() { path } else { link_text };
        match heading {
            Some(h) => out.push_str(&format!("- [{display}](spec:kb:{path}#{h})\n")),
            None => out.push_str(&format!("- [{display}](spec:kb:{path})\n")),
        }
    }
}

fn render_summary(doc: &SpecDocument, out: &mut String) {
    let id = doc.id_str();
    let summary = doc
        .universal
        .summary
        .as_deref()
        .unwrap_or("(no summary)");
    out.push_str(&format!("### {id}\n\n{summary}\n"));
}

fn render_frontmatter_table(doc: &SpecDocument, out: &mut String) {
    let u = &doc.universal;
    out.push_str(&format!("## {}\n\n", doc.id_str()));
    out.push_str("| Field | Value |\n");
    out.push_str("|-------|-------|\n");
    out.push_str(&format!("| **ID** | `{}` |\n", u.id));
    out.push_str(&format!(
        "| **Type** | {} |\n",
        u.entity_type.type_name()
    ));
    out.push_str(&format!("| **Status** | {} |\n", u.status.as_str()));
    out.push_str(&format!("| **Version** | {} |\n", u.version));
    if let Some(ref summary) = u.summary {
        out.push_str(&format!("| **Summary** | {} |\n", summary.trim()));
    }
    out.push_str(&format!("| **Owners** | {} |\n", u.owners.join(", ")));
    if let Some(ref sha) = u.pinned_at {
        out.push_str(&format!("| **Pinned at** | `{sha}` |\n"));
    }

    // Type-specific fields
    match &doc.type_fields {
        TypeSpecificFields::Requirement {
            level,
            refines,
            categorized_under,
            kind,
            ..
        } => {
            out.push_str(&format!("| **Level** | {} |\n", level.as_str()));
            if !refines.is_empty() {
                out.push_str(&format!(
                    "| **Refines** | {} |\n",
                    refines.join(", ")
                ));
            }
            if !categorized_under.is_empty() {
                out.push_str(&format!(
                    "| **Categorized under** | {} |\n",
                    categorized_under.join(", ")
                ));
            }
            if let Some(k) = kind {
                out.push_str(&format!("| **Kind** | {k} |\n"));
            }
        }
        TypeSpecificFields::Invariant {
            enforcement,
            applies_to,
        } => {
            if !enforcement.is_empty() {
                out.push_str(&format!(
                    "| **Enforcement** | {} |\n",
                    enforcement.join(", ")
                ));
            }
            if !applies_to.is_empty() {
                out.push_str(&format!(
                    "| **Applies to** | {} |\n",
                    applies_to.join(", ")
                ));
            }
        }
        TypeSpecificFields::Interface {
            consumed_by,
            provided_by,
            stability,
        } => {
            if !consumed_by.is_empty() {
                out.push_str(&format!(
                    "| **Consumed by** | {} |\n",
                    consumed_by.join(", ")
                ));
            }
            if !provided_by.is_empty() {
                out.push_str(&format!(
                    "| **Provided by** | {} |\n",
                    provided_by.join(", ")
                ));
            }
            out.push_str(&format!(
                "| **Stability** | {:?} |\n",
                stability
            ));
        }
        TypeSpecificFields::Adr {
            decision_date,
            decided_by,
        } => {
            out.push_str(&format!("| **Decision date** | {decision_date} |\n"));
            out.push_str(&format!(
                "| **Decided by** | {} |\n",
                decided_by.join(", ")
            ));
        }
        _ => {}
    }
}
