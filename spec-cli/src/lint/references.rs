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
                SpecReference::Source { .. } => {
                    // Source references are not validated against the registry
                    // (they reference files in the working tree, not specs)
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
