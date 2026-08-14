use crate::intellect::AdherenceSnapshot;
use crate::model::document::SpecDocument;
use crate::model::frontmatter::TypeSpecificFields;
use crate::model::registry::SpecRegistry;

use super::scope::{DetailLevel, ScopedEntry};

/// Render a set of scoped entries as human-readable Markdown.
pub fn render_human(
    registry: &SpecRegistry,
    entries: &[ScopedEntry],
    adherence: Option<&AdherenceSnapshot>,
) -> String {
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
            DetailLevel::Full => render_full(doc, registry, adherence, &mut out),
            DetailLevel::Summary => render_summary(doc, adherence, &mut out),
            DetailLevel::IdOnly => {
                out.push_str(&format!("- {}\n", entry.id));
            }
            DetailLevel::None => {}
        }
    }

    out
}

fn render_full(
    doc: &SpecDocument,
    registry: &SpecRegistry,
    adherence: Option<&AdherenceSnapshot>,
    out: &mut String,
) {
    // Header table
    render_frontmatter_table(doc, registry, adherence, out);
    out.push('\n');
    // Body
    out.push_str(&doc.body_raw);
    if !doc.body_raw.ends_with('\n') {
        out.push('\n');
    }
}

fn render_summary(doc: &SpecDocument, adherence: Option<&AdherenceSnapshot>, out: &mut String) {
    let id = doc.id_str();
    let summary = doc.universal.summary.as_deref().unwrap_or("(no summary)");
    let state = adherence
        .and_then(|snapshot| snapshot.get(&id))
        .map(|state| state.state.as_str())
        .unwrap_or("unavailable");
    out.push_str(&format!("### {id} [{state}]\n\n{summary}\n"));
}

fn render_frontmatter_table(
    doc: &SpecDocument,
    registry: &SpecRegistry,
    adherence: Option<&AdherenceSnapshot>,
    out: &mut String,
) {
    let u = &doc.universal;
    out.push_str(&format!("## {}\n\n", doc.id_str()));
    out.push_str("| Field | Value |\n");
    out.push_str("|-------|-------|\n");
    out.push_str(&format!("| **ID** | `{}` |\n", u.id));
    out.push_str(&format!("| **Type** | {} |\n", u.entity_type.type_name()));
    out.push_str(&format!("| **Status** | {} |\n", u.status.as_str()));
    let revision = crate::history::revision::for_path(&doc.source_path)
        .map(|revision| revision.to_string())
        .unwrap_or_else(|_| "unavailable".into());
    out.push_str(&format!("| **Revision** | {revision} |\n"));
    out.push_str(&format!(
        "| **Spec baseline** | `{}` |\n",
        registry.config.baseline
    ));
    if let Some(ref summary) = u.summary {
        out.push_str(&format!("| **Summary** | {} |\n", summary.trim()));
    }
    out.push_str(&format!("| **Owners** | {} |\n", u.owners.join(", ")));
    if let Some(ref sha) = u.pinned_at {
        out.push_str(&format!("| **Pinned at** | `{sha}` |\n"));
    }
    if let Some(ref sha) = u.implemented {
        out.push_str(&format!("| **Implemented** | `{sha}` |\n"));
    }
    if let Some(snapshot) = adherence {
        if let Some(state) = snapshot.get(&doc.id_str()) {
            out.push_str(&format!("| **Adherence** | {} |\n", state.state.as_str()));
            out.push_str(&format!(
                "| **Intellect provider** | {} {} |\n",
                snapshot.provider, snapshot.provider_version
            ));
            if !state.reasons.is_empty() {
                out.push_str(&format!(
                    "| **Adherence reason** | {} |\n",
                    state.reasons.join("; ")
                ));
            }
        }
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
                out.push_str(&format!("| **Refines** | {} |\n", refines.join(", ")));
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
                out.push_str(&format!("| **Applies to** | {} |\n", applies_to.join(", ")));
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
            out.push_str(&format!("| **Stability** | {:?} |\n", stability));
        }
        TypeSpecificFields::Adr {
            decision_date,
            decided_by,
        } => {
            out.push_str(&format!("| **Decision date** | {decision_date} |\n"));
            out.push_str(&format!("| **Decided by** | {} |\n", decided_by.join(", ")));
        }
        TypeSpecificFields::Task {
            progress,
            addresses,
            labels,
            assignee,
            eta,
            blocked_by,
            groups,
            completion_checkpoint,
        } => {
            out.push_str(&format!("| **Progress** | {} |\n", progress.as_str()));
            if !addresses.is_empty() {
                out.push_str(&format!("| **Addresses** | {} |\n", addresses.join(", ")));
            }
            if !labels.is_empty() {
                out.push_str(&format!("| **Labels** | {} |\n", labels.join(", ")));
            }
            if let Some(assignee) = assignee {
                out.push_str(&format!("| **Assignee** | {assignee} |\n"));
            }
            if let Some(eta) = eta {
                out.push_str(&format!("| **ETA** | {eta} |\n"));
            }
            if !blocked_by.is_empty() {
                out.push_str(&format!("| **Blocked by** | {} |\n", blocked_by.join(", ")));
            }
            if !groups.is_empty() {
                out.push_str(&format!("| **Work groups** | {} |\n", groups.join(", ")));
            }
            if let Some(checkpoint) = completion_checkpoint {
                out.push_str(&format!("| **Completion checkpoint** | `{checkpoint}` |\n"));
            }
        }
        _ => {}
    }
}
