use regex::Regex;
use std::collections::BTreeMap;
use std::sync::LazyLock;

use crate::model::document::SpecDocument;
use crate::model::frontmatter::TypeSpecificFields;
use crate::model::id::EntityType;
use crate::model::registry::SpecRegistry;

use super::diagnostic::Diagnostic;

static ID_PATTERN: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^(REQ|INV|IFC|ADR|GLO|TOPIC|SCN):[a-z0-9][\w-]*/[a-z0-9][\w-]*$").unwrap()
});

/// R001: ID matches `<TYPE>:namespace/slug` pattern.
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
    // id, type, status, version are validated during parsing — if we got here they exist.
    diags
}

/// R004: Type-specific frontmatter fields present.
pub fn check_type_specific_fields(doc: &SpecDocument) -> Vec<Diagnostic> {
    let mut diags = Vec::new();

    match (&doc.universal.entity_type, &doc.type_fields) {
        (EntityType::Req, TypeSpecificFields::Requirement { level: _, .. }) => {
            // level has a default; nothing strictly required beyond universal
        }
        (EntityType::Adr, TypeSpecificFields::Adr { decision_date, decided_by }) => {
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
                        format!("duplicate anchor '{}' (first at line {prev_line})", block.id),
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
