pub mod content;
pub mod diagnostic;
pub mod references;
pub mod refinement;
pub mod structural;
pub mod trailers;

use crate::model::frontmatter::Status;
use crate::model::registry::SpecRegistry;

use self::diagnostic::{Diagnostic, Severity};

/// Run all lint checks on a registry.
pub fn lint_all(registry: &SpecRegistry) -> Vec<Diagnostic> {
    let mut diags = Vec::new();

    // Per-document checks
    for doc in &registry.documents {
        let is_draft = doc.universal.status == Status::Draft;
        let mut doc_diags = Vec::new();

        // Structural checks
        doc_diags.extend(structural::check_id_pattern(doc));
        doc_diags.extend(structural::check_type_matches_prefix(doc));
        doc_diags.extend(structural::check_universal_fields(doc));
        doc_diags.extend(structural::check_type_specific_fields(doc));
        doc_diags.extend(structural::check_unique_anchors(doc));

        // Content checks
        doc_diags.extend(content::check_multi_entity(doc, 10));
        doc_diags.extend(content::check_rfc2119_discipline(doc));

        // Draft status downgrades R002-R012 from error to warning
        if is_draft {
            for d in &mut doc_diags {
                let code_num: Option<u32> = d
                    .code
                    .strip_prefix('R')
                    .and_then(|s| s.parse().ok());
                if let Some(n) = code_num {
                    if (2..=12).contains(&n) && d.severity == Severity::Error {
                        d.downgrade();
                    }
                }
            }
        }

        diags.extend(doc_diags);
    }

    // Registry-wide checks
    diags.extend(structural::check_unique_ids(registry));
    diags.extend(references::check_references(registry));
    diags.extend(references::check_summary_on_referenced(registry));
    diags.extend(refinement::check_refinement(registry));
    diags.extend(trailers::check_trailer_references(registry));

    // Sort diagnostics by file path and line number
    diags.sort_by(|a, b| {
        a.file
            .cmp(&b.file)
            .then_with(|| a.line.cmp(&b.line))
            .then_with(|| a.code.cmp(&b.code))
    });

    diags
}
