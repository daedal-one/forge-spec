use once_cell::sync::Lazy;
use regex::Regex;
use std::collections::BTreeMap;

use crate::model::config::CURRENT_SPEC_BASELINE;
use crate::model::document::SpecDocument;
use crate::model::frontmatter::TypeSpecificFields;
use crate::model::id::EntityType;
use crate::model::registry::SpecRegistry;

use super::diagnostic::Diagnostic;

static ID_PATTERN: Lazy<Regex> = Lazy::new(|| {
    Regex::new(
        r"^(PROJECT:[a-z0-9][\w-]*|(REQ|INV|IFC|ADR|GLO|TOPIC|SCN|TASK):[a-z0-9][\w-]*/[a-z0-9][\w-]*)$",
    )
    .unwrap()
});

/// R001: ID matches `PROJECT:<slug>` or `<TYPE>:namespace/slug`.
pub fn check_id_pattern(doc: &SpecDocument) -> Vec<Diagnostic> {
    let id_str = doc.id_str();
    if !ID_PATTERN.is_match(&id_str) {
        vec![Diagnostic::error(
            "R001",
            format!("invalid spec ID format: '{id_str}'"),
            doc.source_path.clone(),
        )]
    } else {
        vec![]
    }
}

/// R002: Type matches ID prefix.
pub fn check_type_matches_prefix(doc: &SpecDocument) -> Vec<Diagnostic> {
    let id_prefix = doc.universal.id.entity_type;
    let declared_type = doc.universal.entity_type;
    if id_prefix != declared_type {
        vec![Diagnostic::error(
            "R002",
            format!(
                "type '{}' does not match ID prefix '{}'",
                declared_type.type_name(),
                id_prefix.prefix()
            ),
            doc.source_path.clone(),
        )]
    } else {
        vec![]
    }
}

/// R003: All universal frontmatter fields present.
pub fn check_universal_fields(doc: &SpecDocument) -> Vec<Diagnostic> {
    let mut diags = Vec::new();
    let u = &doc.universal;

    if u.owners.is_empty() {
        diags.push(Diagnostic::error(
            "R003",
            "missing required field: owners (must be non-empty)",
            doc.source_path.clone(),
        ));
    }
    diags
}

/// R024: the spec tree declares a supported forge-spec baseline once.
pub fn check_spec_config(registry: &SpecRegistry) -> Vec<Diagnostic> {
    let path = registry.specs_dir.join("_config.toml");
    if !registry.config.declared {
        return vec![Diagnostic::warning(
            "R024",
            format!(
                "missing _config.toml; run `spec migrate plan --target agent`, then `spec migrate apply` to {CURRENT_SPEC_BASELINE}"
            ),
            path,
        )];
    }
    if registry.config.baseline != CURRENT_SPEC_BASELINE {
        return vec![Diagnostic::error(
            "R024",
            format!(
                "baseline '{}' is not the active baseline {CURRENT_SPEC_BASELINE}; run `spec migrate plan --target agent` or upgrade the CLI",
                registry.config.baseline
            ),
            path,
        )];
    }
    Vec::new()
}

/// R025: the tree has exactly one configured PROJECT document.
pub fn check_project_root(registry: &SpecRegistry) -> Vec<Diagnostic> {
    if registry.config.baseline != CURRENT_SPEC_BASELINE {
        return Vec::new();
    }

    let config_path = registry.specs_dir.join("_config.toml");
    let project_documents = registry
        .documents
        .iter()
        .filter(|document| document.universal.entity_type == EntityType::Project)
        .collect::<Vec<_>>();

    if project_documents.len() != 1 {
        return vec![Diagnostic::error(
            "R025",
            format!(
                "expected exactly one PROJECT document, found {}; run `spec init` for a new tree or `spec migrate apply` for an older tree",
                project_documents.len()
            ),
            config_path,
        )];
    }

    let Some(configured_id) = registry.config.project.as_deref() else {
        return vec![Diagnostic::error(
            "R025",
            "missing `project` in _config.toml",
            config_path,
        )];
    };

    let Ok(project_id) = configured_id.parse::<crate::model::id::SpecId>() else {
        return vec![Diagnostic::error(
            "R025",
            format!("invalid configured project ID: '{configured_id}'"),
            config_path,
        )];
    };
    if project_id.entity_type != EntityType::Project {
        return vec![Diagnostic::error(
            "R025",
            format!("configured project must use a PROJECT: ID, found '{configured_id}'"),
            config_path,
        )];
    }

    let document = project_documents[0];
    if document.id_str() != configured_id {
        return vec![Diagnostic::error(
            "R025",
            format!(
                "configured project '{configured_id}' does not select the tree's PROJECT document '{}'",
                document.id_str()
            ),
            config_path,
        )];
    }

    if document
        .universal
        .summary
        .as_deref()
        .map(str::trim)
        .unwrap_or_default()
        .is_empty()
    {
        return vec![Diagnostic::error(
            "R025",
            "PROJECT document requires a non-empty summary",
            document.source_path.clone(),
        )];
    }

    let mut diagnostics = Vec::new();
    for child in &registry.documents {
        let refines: &[String] = match &child.type_fields {
            TypeSpecificFields::Requirement { refines, .. }
            | TypeSpecificFields::Task { refines, .. } => refines,
            _ => continue,
        };
        if refines.iter().any(|target| {
            target.split_once('#').map(|(id, _)| id).unwrap_or(target) == configured_id
        }) {
            diagnostics.push(Diagnostic::error(
                "R025",
                format!(
                    "'{}' refines PROJECT; project containment is implicit and has no satisfaction semantics",
                    child.id_str()
                ),
                child.source_path.clone(),
            ));
        }
    }

    diagnostics
}

/// R004: Type-specific frontmatter fields present.
pub fn check_type_specific_fields(doc: &SpecDocument) -> Vec<Diagnostic> {
    let mut diags = Vec::new();

    match (&doc.universal.entity_type, &doc.type_fields) {
        (EntityType::Project, TypeSpecificFields::Project) => {}
        (EntityType::Req, TypeSpecificFields::Requirement { level: _, .. }) => {
            // level has a default; nothing strictly required beyond universal
        }
        (
            EntityType::Adr,
            TypeSpecificFields::Adr {
                decision_date,
                decided_by,
            },
        ) => {
            if decision_date.is_empty() {
                diags.push(Diagnostic::error(
                    "R004",
                    "ADR missing required field: decision_date",
                    doc.source_path.clone(),
                ));
            }
            if decided_by.is_empty() {
                diags.push(Diagnostic::error(
                    "R004",
                    "ADR missing required field: decided_by",
                    doc.source_path.clone(),
                ));
            }
        }
        (EntityType::Ifc, TypeSpecificFields::Interface { .. }) => {
            // stability has a default
        }
        (
            EntityType::Task,
            TypeSpecificFields::Task {
                refines,
                blocked_by,
                progress,
                ..
            },
        ) => {
            // R018: a task that has no parent to refine and no upstream blockers
            // is dangling — surface as a warning so authors can attach it.
            if refines.is_empty() && blocked_by.is_empty() {
                diags.push(Diagnostic::warning(
                    "R018",
                    "TASK has neither `refines:` nor `blocked_by:` — task will not appear in any coverage report",
                    doc.source_path.clone(),
                ));
            }
            // R019: deferred and wontdo tasks with no summary explanation
            // are easy to lose track of; warn rather than error.
            use crate::model::frontmatter::Progress as P;
            if matches!(progress, P::Deferred | P::WontDo)
                && doc
                    .universal
                    .summary
                    .as_deref()
                    .map(str::trim)
                    .unwrap_or("")
                    .is_empty()
            {
                let msg = match progress {
                    P::Deferred => "TASK is deferred without a summary explaining why",
                    P::WontDo => "TASK is wontdo without a summary explaining why",
                    _ => unreachable!(),
                };
                diags.push(Diagnostic::warning("R019", msg, doc.source_path.clone()));
            }
        }
        _ => {}
    }

    diags
}

/// R014: No two documents share an ID.
pub fn check_unique_ids(registry: &SpecRegistry) -> Vec<Diagnostic> {
    let mut seen: BTreeMap<String, Vec<&SpecDocument>> = BTreeMap::new();
    for doc in &registry.documents {
        seen.entry(doc.id_str()).or_default().push(doc);
    }

    let mut diags = Vec::new();
    for (id, docs) in &seen {
        if docs.len() > 1 {
            for doc in docs {
                diags.push(Diagnostic::error(
                    "R014",
                    format!("duplicate spec ID: '{id}'"),
                    doc.source_path.clone(),
                ));
            }
        }
    }
    diags
}

/// R015: No two anchors share a (doc, anchor) pair.
pub fn check_unique_anchors(doc: &SpecDocument) -> Vec<Diagnostic> {
    let mut seen: BTreeMap<&str, usize> = BTreeMap::new();
    let mut diags = Vec::new();

    for block in &doc.blocks {
        if !block.id.is_empty() {
            if let Some(&prev_line) = seen.get(block.id.as_str()) {
                diags.push(
                    Diagnostic::error(
                        "R015",
                        format!(
                            "duplicate anchor '{}' (first at line {prev_line})",
                            block.id
                        ),
                        doc.source_path.clone(),
                    )
                    .at_line(block.start_line),
                );
            } else {
                seen.insert(&block.id, block.start_line);
            }
        }

        for clause in &block.clauses {
            if let Some(&prev_line) = seen.get(clause.id.as_str()) {
                diags.push(
                    Diagnostic::error(
                        "R015",
                        format!(
                            "duplicate anchor '{}' (first at line {prev_line})",
                            clause.id
                        ),
                        doc.source_path.clone(),
                    )
                    .at_line(clause.line),
                );
            } else {
                seen.insert(&clause.id, clause.line);
            }
        }
    }

    diags
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validates_the_configured_singleton_project() {
        let temp = tempfile::tempdir().unwrap();
        std::fs::write(
            temp.path().join("_config.toml"),
            "baseline = \"forge-spec-v0.4.0\"\nproject = \"PROJECT:demo\"\n",
        )
        .unwrap();
        std::fs::write(
            temp.path().join("_project.spec.md"),
            "---\nid: PROJECT:demo\ntype: project\nstatus: accepted\nsummary: Demo.\nowners: [dev]\n---\n\n# Demo\n",
        )
        .unwrap();
        let registry = SpecRegistry::load(temp.path()).unwrap();
        assert!(check_project_root(&registry).is_empty());
    }

    #[test]
    fn rejects_a_missing_project_document() {
        let temp = tempfile::tempdir().unwrap();
        std::fs::write(
            temp.path().join("_config.toml"),
            "baseline = \"forge-spec-v0.4.0\"\nproject = \"PROJECT:demo\"\n",
        )
        .unwrap();
        let registry = SpecRegistry::load(temp.path()).unwrap();
        let diagnostics = check_project_root(&registry);
        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].code, "R025");
        assert!(diagnostics[0].message.contains("exactly one PROJECT"));
    }

    #[test]
    fn rejects_refinement_of_project_context() {
        let temp = tempfile::tempdir().unwrap();
        std::fs::write(
            temp.path().join("_config.toml"),
            "baseline = \"forge-spec-v0.4.0\"\nproject = \"PROJECT:demo\"\n",
        )
        .unwrap();
        std::fs::write(
            temp.path().join("_project.spec.md"),
            "---\nid: PROJECT:demo\ntype: project\nstatus: accepted\nsummary: Demo.\nowners: [dev]\n---\n\n# Demo\n",
        )
        .unwrap();
        std::fs::write(
            temp.path().join("child.spec.md"),
            "---\nid: REQ:demo/child\ntype: requirement\nstatus: accepted\nsummary: Child.\nowners: [dev]\nlevel: MUST\nrefines: [PROJECT:demo]\n---\n\n# Child\n",
        )
        .unwrap();
        let registry = SpecRegistry::load(temp.path()).unwrap();
        let diagnostics = check_project_root(&registry);
        assert_eq!(diagnostics.len(), 1);
        assert!(diagnostics[0].message.contains("refines PROJECT"));
    }
}
