use crate::model::frontmatter::Status;
use crate::model::reference::SpecReference;
use crate::model::registry::SpecRegistry;
use crate::symbol::{SymbolError, SymbolService};

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
                SpecReference::Source(_) => {
                    // Source references are not validated against the registry
                    // (they reference files in the working tree, not specs)
                }
                SpecReference::Documentation(_) => {
                    // Validated against the configured documentation index.
                }
            }
        }
    }

    diags
}

/// R026-R029: documentation collections, explicit targets, headings, and
/// ordinary relative Markdown links resolve without changing spec semantics.
pub fn check_documentation_references(registry: &SpecRegistry) -> Vec<Diagnostic> {
    let mut diagnostics = registry
        .documentation
        .issues
        .iter()
        .map(|issue| {
            let mut diagnostic =
                Diagnostic::error(&issue.code, issue.message.clone(), issue.file.clone());
            if let Some(line) = issue.line {
                diagnostic = diagnostic.at_line(line);
            }
            diagnostic
        })
        .collect::<Vec<_>>();

    for document in &registry.documents {
        for located in &document.references {
            let SpecReference::Documentation(reference) = &located.reference else {
                continue;
            };
            if reference.path.is_empty()
                || std::path::Path::new(&reference.path).is_absolute()
                || std::path::Path::new(&reference.path)
                    .components()
                    .any(|component| matches!(component, std::path::Component::ParentDir))
            {
                diagnostics.push(
                    Diagnostic::error(
                        "R027",
                        format!("unsafe documentation target '{}'", reference.path),
                        document.source_path.clone(),
                    )
                    .at_line(located.line),
                );
                continue;
            }
            let Some(target_document) = registry.documentation.get(&reference.path) else {
                diagnostics.push(
                    Diagnostic::error(
                        "R027",
                        format!("documentation target '{}' is not enrolled", reference.path),
                        document.source_path.clone(),
                    )
                    .at_line(located.line),
                );
                continue;
            };
            if let crate::documentation::DocumentationTarget::Heading { segments } =
                &reference.target
            {
                let count = target_document
                    .headings
                    .iter()
                    .filter(|heading| heading.segments == *segments)
                    .count();
                if count != 1 {
                    diagnostics.push(
                        Diagnostic::error(
                            "R028",
                            format!(
                                "documentation heading '{}' does not resolve uniquely in '{}'",
                                segments.join(" / "),
                                reference.path
                            ),
                            document.source_path.clone(),
                        )
                        .at_line(located.line),
                    );
                }
            }
        }
    }
    for document in &registry.documentation.documents {
        for link in &document.links {
            let crate::documentation::DocumentationLinkTarget::Forge(SpecReference::Spec(target)) =
                &link.target
            else {
                continue;
            };
            let reference = target.to_string();
            let (exists, traversed) = registry.reference_exists(&reference);
            if traversed {
                let (resolved, _) = registry.resolve_redirect(&reference);
                diagnostics.push(
                    Diagnostic::info(
                        "R-redir",
                        format!(
                            "documentation reference '{reference}' traverses redirect to '{resolved}'"
                        ),
                        document.source_path.clone(),
                    )
                    .at_line(link.line),
                );
            }
            if !exists {
                diagnostics.push(
                    Diagnostic::error(
                        "R005",
                        format!("dangling specification reference: '{reference}'"),
                        document.source_path.clone(),
                    )
                    .at_line(link.line),
                );
            }
        }
    }
    diagnostics
}

/// R020-R023: source paths, symbols, providers, and line ranges resolve.
pub fn check_source_references(
    registry: &SpecRegistry,
    require_symbols: bool,
    allow_custom_lsp: bool,
) -> Vec<Diagnostic> {
    let service = SymbolService::new(&registry.specs_dir, allow_custom_lsp);
    let mut diagnostics = Vec::new();
    for document in &registry.documents {
        for located in &document.references {
            let SpecReference::Source(source) = &located.reference else {
                continue;
            };
            let outcome = match &service {
                Ok(service) => service.resolve(source),
                Err(error) => Err(error.clone()),
            };
            let Err(error) = outcome else {
                continue;
            };
            let message = error.to_string();
            let diagnostic = match error {
                SymbolError::UnsafePath(_) | SymbolError::MissingPath(_) => {
                    Diagnostic::error("R020", message, document.source_path.clone())
                }
                SymbolError::NotFound(_) => {
                    Diagnostic::error("R021", message, document.source_path.clone())
                }
                SymbolError::InvalidRange { .. } => {
                    Diagnostic::error("R023", message, document.source_path.clone())
                }
                SymbolError::ProviderUnavailable { .. }
                | SymbolError::UnsupportedLanguage(_)
                | SymbolError::Protocol(_) => {
                    if require_symbols {
                        Diagnostic::error("R022", message, document.source_path.clone())
                    } else {
                        Diagnostic::warning("R022", message, document.source_path.clone())
                    }
                }
            };
            diagnostics.push(diagnostic.at_line(located.line));
        }
    }
    for document in &registry.documentation.documents {
        for link in &document.links {
            let crate::documentation::DocumentationLinkTarget::Forge(SpecReference::Source(source)) =
                &link.target
            else {
                continue;
            };
            let outcome = match &service {
                Ok(service) => service.resolve(source),
                Err(error) => Err(error.clone()),
            };
            let Err(error) = outcome else {
                continue;
            };
            let message = error.to_string();
            let diagnostic = match error {
                SymbolError::UnsafePath(_) | SymbolError::MissingPath(_) => {
                    Diagnostic::error("R020", message, document.source_path.clone())
                }
                SymbolError::NotFound(_) => {
                    Diagnostic::error("R021", message, document.source_path.clone())
                }
                SymbolError::InvalidRange { .. } => {
                    Diagnostic::error("R023", message, document.source_path.clone())
                }
                SymbolError::ProviderUnavailable { .. }
                | SymbolError::UnsupportedLanguage(_)
                | SymbolError::Protocol(_) => {
                    if require_symbols {
                        Diagnostic::error("R022", message, document.source_path.clone())
                    } else {
                        Diagnostic::warning("R022", message, document.source_path.clone())
                    }
                }
            };
            diagnostics.push(diagnostic.at_line(link.line));
        }
    }
    diagnostics
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
