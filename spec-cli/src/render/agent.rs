use crate::graph;
use crate::intellect::AdherenceSnapshot;
use crate::model::document::SpecDocument;
use crate::model::frontmatter::TypeSpecificFields;
use crate::model::id::EntityType;
use crate::model::registry::SpecRegistry;

use super::scope::{DetailLevel, ScopedEntry};

/// Render a set of scoped entries as an agent-optimized XML envelope.
pub fn render_agent(
    registry: &SpecRegistry,
    entries: &[ScopedEntry],
    adherence: Option<&AdherenceSnapshot>,
) -> String {
    let mut out = String::new();
    out.push_str("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n");
    let adherence_attributes = adherence.map_or_else(String::new, |snapshot| {
        format!(
            " intellect-provider=\"{}\" provider-version=\"{}\" workspace-head=\"{}\" worktree=\"{}\" adherence-complete=\"{}\"",
            escape_xml(&snapshot.provider),
            escape_xml(&snapshot.provider_version),
            escape_xml(&snapshot.workspace.head),
            escape_xml(&snapshot.workspace.worktree),
            snapshot.complete,
        )
    });
    if let Some(project_id) = registry.project_id() {
        out.push_str(&format!(
            "<specs project=\"{}\"{}>\n",
            escape_xml(&project_id),
            adherence_attributes,
        ));
    } else {
        out.push_str(&format!("<specs{}>\n", adherence_attributes));
    }

    for entry in entries {
        let Some(doc) = registry.get_by_id(&entry.id) else {
            out.push_str(&format!(
                "  <!-- spec not found: {} -->\n",
                escape_xml(&entry.id)
            ));
            continue;
        };

        match entry.detail {
            DetailLevel::Full => render_spec_full(doc, registry, adherence, &mut out),
            DetailLevel::Summary => render_spec_summary(doc, registry, adherence, &mut out),
            DetailLevel::IdOnly => {
                let state = adherence
                    .and_then(|snapshot| snapshot.get(&entry.id))
                    .map(|state| format!(" adherence=\"{}\"", state.state.as_str()))
                    .unwrap_or_default();
                out.push_str(&format!(
                    "  <spec id=\"{}\"{} />\n",
                    escape_xml(&entry.id),
                    state
                ));
            }
            DetailLevel::None => {}
        }
    }

    out.push_str("</specs>\n");
    out
}

fn render_spec_full(
    doc: &SpecDocument,
    registry: &SpecRegistry,
    adherence: Option<&AdherenceSnapshot>,
    out: &mut String,
) {
    let id = doc.id_str();
    let u = &doc.universal;

    // Opening tag with attributes
    let tag = match u.entity_type {
        EntityType::Project => "project",
        EntityType::Task => "work-item",
        _ => "spec",
    };
    out.push_str(&format!(
        "  <{tag} id=\"{}\" type=\"{}\" status=\"{}\" revision=\"{}\" baseline=\"{}\"",
        escape_xml(&id),
        u.entity_type.type_name(),
        u.status.as_str(),
        revision_for(doc),
        escape_xml(&registry.config.baseline),
    ));

    if let Some(implemented) = &u.implemented {
        out.push_str(&format!(" implemented=\"{}\"", escape_xml(implemented)));
    }
    if let TypeSpecificFields::Task {
        progress,
        completion_checkpoint,
        ..
    } = &doc.type_fields
    {
        out.push_str(&format!(" progress=\"{}\"", progress.as_str()));
        if let Some(checkpoint) = completion_checkpoint {
            out.push_str(&format!(
                " completion-checkpoint=\"{}\"",
                escape_xml(checkpoint)
            ));
        }
    }
    if let Some(state) = adherence.and_then(|snapshot| snapshot.get(&id)) {
        out.push_str(&format!(
            " adherence=\"{}\" adherence-complete=\"{}\"",
            state.state.as_str(),
            state.complete
        ));
    }

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

    if let TypeSpecificFields::Task {
        addresses,
        labels,
        blocked_by,
        groups,
        ..
    } = &doc.type_fields
    {
        render_values("addresses", "address", addresses, out);
        render_values("labels", "label", labels, out);
        render_values("blocked-by", "task", blocked_by, out);
        render_values("groups", "topic", groups, out);
    }

    render_adherence(&id, adherence, out);

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
    let children = if u.entity_type == EntityType::Project {
        graph::query::hierarchy_children(registry, &id)
    } else {
        graph::query::children(registry, &id)
    };
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

    out.push_str(&format!("  </{tag}>\n"));
}

fn render_spec_summary(
    doc: &SpecDocument,
    registry: &SpecRegistry,
    adherence: Option<&AdherenceSnapshot>,
    out: &mut String,
) {
    let id = doc.id_str();
    let summary = doc.universal.summary.as_deref().unwrap_or("");
    let tag = match doc.universal.entity_type {
        EntityType::Project => "project",
        EntityType::Task => "work-item",
        _ => "spec",
    };
    let adherence_attributes = adherence
        .and_then(|snapshot| snapshot.get(&id))
        .map(|state| {
            format!(
                " adherence=\"{}\" adherence-complete=\"{}\"",
                state.state.as_str(),
                state.complete
            )
        })
        .unwrap_or_default();
    let implemented_attribute = doc
        .universal
        .implemented
        .as_deref()
        .map(|commit| format!(" implemented=\"{}\"", escape_xml(commit)))
        .unwrap_or_default();
    out.push_str(&format!(
        "  <{tag} id=\"{}\" type=\"{}\" status=\"{}\" revision=\"{}\" baseline=\"{}\"{}{}>\n",
        escape_xml(&id),
        doc.universal.entity_type.type_name(),
        doc.universal.status.as_str(),
        revision_for(doc),
        escape_xml(&registry.config.baseline),
        implemented_attribute,
        adherence_attributes,
    ));
    if !summary.is_empty() {
        out.push_str(&format!(
            "    <summary>{}</summary>\n",
            escape_xml(summary.trim())
        ));
    }
    render_adherence(&id, adherence, out);
    out.push_str(&format!("  </{tag}>\n"));
}

fn render_adherence(id: &str, adherence: Option<&AdherenceSnapshot>, out: &mut String) {
    let Some((snapshot, state)) =
        adherence.and_then(|snapshot| snapshot.get(id).map(|state| (snapshot, state)))
    else {
        return;
    };
    out.push_str(&format!(
        "    <adherence state=\"{}\" complete=\"{}\" provider=\"{}\" intent-digest=\"{}\"{}{}>\n",
        state.state.as_str(),
        state.complete,
        escape_xml(&snapshot.provider),
        escape_xml(&state.intent_digest),
        state
            .attestation_id
            .as_ref()
            .map_or_else(String::new, |id| format!(
                " attestation-id=\"{}\"",
                escape_xml(id)
            )),
        state
            .checkpoint
            .as_ref()
            .map_or_else(String::new, |checkpoint| format!(
                " checkpoint=\"{}\"",
                escape_xml(checkpoint)
            )),
    ));
    for reason in &state.reasons {
        out.push_str(&format!("      <reason>{}</reason>\n", escape_xml(reason)));
    }
    for evidence in &state.evidence {
        out.push_str(&format!(
            "      <evidence>{}</evidence>\n",
            escape_xml(evidence)
        ));
    }
    out.push_str("    </adherence>\n");
}

fn revision_for(doc: &SpecDocument) -> String {
    crate::history::revision::for_path(&doc.source_path)
        .map(|revision| revision.to_string())
        .unwrap_or_else(|_| "unavailable".into())
}

fn render_values(container: &str, item: &str, values: &[String], out: &mut String) {
    if values.is_empty() {
        return;
    }
    out.push_str(&format!("    <{container}>\n"));
    for value in values {
        out.push_str(&format!("      <{item}>{}</{item}>\n", escape_xml(value)));
    }
    out.push_str(&format!("    </{container}>\n"));
}

fn escape_xml(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::intellect::{AdherenceState, SpecAdherence, WorkspaceState, INTELLECT_PROTOCOL};
    use crate::render::scope::{compute_scope, DetailLevel};

    #[test]
    fn renders_project_as_distinct_ambient_context() {
        let temp = tempfile::tempdir().unwrap();
        std::fs::write(
            temp.path().join("_config.toml"),
            "baseline = \"forge-spec-v0.6.0\"\nproject = \"PROJECT:demo\"\n",
        )
        .unwrap();
        std::fs::write(
            temp.path().join("_project.spec.md"),
            "---\nid: PROJECT:demo\ntype: project\nstatus: accepted\nsummary: Demo purpose.\nowners: [dev]\n---\n\n# Demo\n",
        )
        .unwrap();
        std::fs::write(
            temp.path().join("req.spec.md"),
            "---\nid: REQ:demo/work\ntype: requirement\nstatus: accepted\nsummary: Work.\nowners: [dev]\nlevel: MUST\nrefines: []\n---\n\n# Work\n",
        )
        .unwrap();
        let registry = SpecRegistry::load(temp.path()).unwrap();
        let scope = compute_scope(
            &registry,
            "REQ:demo/work",
            DetailLevel::None,
            DetailLevel::None,
            None,
        );
        let output = render_agent(&registry, &scope, None);

        assert!(output.contains("<specs project=\"PROJECT:demo\">"));
        let project = output.find("<project id=\"PROJECT:demo\"").unwrap();
        let requirement = output.find("<spec id=\"REQ:demo/work\"").unwrap();
        assert!(project < requirement);
        assert!(output.contains("<descendant id=\"REQ:demo/work\""));
    }

    #[test]
    fn renders_external_attestation_identity_and_intent_digest() {
        let temp = tempfile::tempdir().unwrap();
        std::fs::write(
            temp.path().join("_config.toml"),
            "baseline = \"forge-spec-v0.6.0\"\nproject = \"PROJECT:demo\"\n",
        )
        .unwrap();
        std::fs::write(
            temp.path().join("_project.spec.md"),
            "---\nid: PROJECT:demo\ntype: project\nstatus: accepted\nsummary: Demo purpose.\nowners: [dev]\n---\n\n# Demo\n",
        )
        .unwrap();
        std::fs::write(
            temp.path().join("req.spec.md"),
            "---\nid: REQ:demo/work\ntype: requirement\nstatus: accepted\nsummary: Work.\nowners: [dev]\nlevel: MUST\nrefines: []\n---\n\n# Work\n",
        )
        .unwrap();
        let registry = SpecRegistry::load(temp.path()).unwrap();
        let scope = compute_scope(
            &registry,
            "REQ:demo/work",
            DetailLevel::None,
            DetailLevel::None,
            None,
        );
        let snapshot = AdherenceSnapshot {
            schema: INTELLECT_PROTOCOL.into(),
            provider: "forge-intellect".into(),
            provider_version: "0.2.0".into(),
            workspace: WorkspaceState {
                root: temp.path().display().to_string(),
                head: "0123456789abcdef0123456789abcdef01234567".into(),
                worktree: "clean".into(),
            },
            complete: true,
            specifications: vec![SpecAdherence {
                id: "REQ:demo/work".into(),
                intent_digest: "intent-digest".into(),
                attestation_id: Some("attestation-id".into()),
                checkpoint: Some("0123456789abcdef0123456789abcdef01234567".into()),
                state: AdherenceState::Current,
                complete: true,
                reasons: vec!["evidence is current".into()],
                evidence: vec!["source:src/lib.rs".into()],
            }],
        };

        let output = render_agent(&registry, &scope, Some(&snapshot));
        assert!(output.contains("state=\"current\" complete=\"true\""));
        assert!(output.contains("intent-digest=\"intent-digest\""));
        assert!(output.contains("attestation-id=\"attestation-id\""));
        assert!(output.contains("checkpoint=\"0123456789abcdef0123456789abcdef01234567\""));
    }
}
