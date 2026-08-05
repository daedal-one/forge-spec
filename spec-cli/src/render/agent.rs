use crate::graph;
use crate::model::document::SpecDocument;
use crate::model::frontmatter::TypeSpecificFields;
use crate::model::registry::SpecRegistry;

use super::scope::{DetailLevel, ScopedEntry};

/// Render a set of scoped entries as an agent-optimized XML envelope.
pub fn render_agent(registry: &SpecRegistry, entries: &[ScopedEntry]) -> String {
    let mut out = String::new();
    out.push_str("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n");
    out.push_str("<specs>\n");

    for entry in entries {
        let Some(doc) = registry.get_by_id(&entry.id) else {
            out.push_str(&format!(
                "  <!-- spec not found: {} -->\n",
                escape_xml(&entry.id)
            ));
            continue;
        };

        match entry.detail {
            DetailLevel::Full => render_spec_full(doc, registry, &mut out),
            DetailLevel::Summary => render_spec_summary(doc, registry, &mut out),
            DetailLevel::IdOnly => {
                out.push_str(&format!("  <spec id=\"{}\" />\n", escape_xml(&entry.id)));
            }
            DetailLevel::None => {}
        }
    }

    out.push_str("</specs>\n");
    out
}

fn render_spec_full(doc: &SpecDocument, registry: &SpecRegistry, out: &mut String) {
    let id = doc.id_str();
    let u = &doc.universal;

    // Opening tag with attributes
    out.push_str(&format!(
        "  <spec id=\"{}\" type=\"{}\" status=\"{}\" revision=\"{}\" baseline=\"{}\"",
        escape_xml(&id),
        u.entity_type.type_name(),
        u.status.as_str(),
        revision_for(doc),
        escape_xml(&registry.config.baseline),
    ));

    if let TypeSpecificFields::Requirement { level, .. } = &doc.type_fields {
        out.push_str(&format!(" level=\"{}\"", level.as_str()));
    }

    out.push_str(">\n");

    // Summary
    if let Some(ref summary) = u.summary {
        out.push_str(&format!(
            "    <summary>{}</summary>\n",
            escape_xml(summary.trim())
        ));
    }

    // Body
    out.push_str("    <body>\n");
    for line in doc.body_raw.lines() {
        out.push_str(&format!("      {}\n", escape_xml(line)));
    }
    out.push_str("    </body>\n");

    // Ancestors
    let ancestors = graph::query::ancestors(registry, &id);
    if !ancestors.is_empty() {
        out.push_str("    <ancestors>\n");
        for anc_id in &ancestors {
            if let Some(anc) = registry.get_by_id(anc_id) {
                let anc_summary = anc.universal.summary.as_deref().unwrap_or("");
                let level_attr =
                    if let TypeSpecificFields::Requirement { level, .. } = &anc.type_fields {
                        format!(" level=\"{}\"", level.as_str())
                    } else {
                        String::new()
                    };
                out.push_str(&format!(
                    "      <ancestor id=\"{}\"{}>",
                    escape_xml(anc_id),
                    level_attr,
                ));
                if !anc_summary.is_empty() {
                    out.push_str(&format!(
                        "\n        <summary>{}</summary>\n      ",
                        escape_xml(anc_summary.trim())
                    ));
                }
                out.push_str("</ancestor>\n");
            } else {
                out.push_str(&format!(
                    "      <ancestor id=\"{}\" />\n",
                    escape_xml(anc_id)
                ));
            }
        }
        out.push_str("    </ancestors>\n");
    }

    // Descendants
    let children = graph::query::children(registry, &id);
    if !children.is_empty() {
        out.push_str("    <descendants>\n");
        for child_id in &children {
            if let Some(child) = registry.get_by_id(child_id) {
                let level_attr =
                    if let TypeSpecificFields::Requirement { level, .. } = &child.type_fields {
                        format!(" level=\"{}\"", level.as_str())
                    } else {
                        String::new()
                    };
                out.push_str(&format!(
                    "      <descendant id=\"{}\"{}",
                    escape_xml(child_id),
                    level_attr,
                ));
                if let Some(ref s) = child.universal.summary {
                    out.push_str(&format!(
                        ">\n        <summary>{}</summary>\n      </descendant>\n",
                        escape_xml(s.trim())
                    ));
                } else {
                    out.push_str(" />\n");
                }
            } else {
                out.push_str(&format!(
                    "      <descendant id=\"{}\" />\n",
                    escape_xml(child_id)
                ));
            }
        }
        out.push_str("    </descendants>\n");
    }

    out.push_str("  </spec>\n");
}

fn render_spec_summary(doc: &SpecDocument, registry: &SpecRegistry, out: &mut String) {
    let id = doc.id_str();
    let summary = doc.universal.summary.as_deref().unwrap_or("");
    out.push_str(&format!(
        "  <spec id=\"{}\" type=\"{}\" status=\"{}\" revision=\"{}\" baseline=\"{}\">\n",
        escape_xml(&id),
        doc.universal.entity_type.type_name(),
        doc.universal.status.as_str(),
        revision_for(doc),
        escape_xml(&registry.config.baseline),
    ));
    if !summary.is_empty() {
        out.push_str(&format!(
            "    <summary>{}</summary>\n",
            escape_xml(summary.trim())
        ));
    }
    out.push_str("  </spec>\n");
}

fn revision_for(doc: &SpecDocument) -> String {
    crate::history::revision::for_path(&doc.source_path)
        .map(|revision| revision.to_string())
        .unwrap_or_else(|_| "unavailable".into())
}

fn escape_xml(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}
