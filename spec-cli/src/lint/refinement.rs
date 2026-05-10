use std::collections::{BTreeMap, BTreeSet};

use petgraph::graph::DiGraph;
use petgraph::algo::is_cyclic_directed;

use crate::model::frontmatter::{Level, TypeSpecificFields};
use crate::model::registry::SpecRegistry;

use super::diagnostic::Diagnostic;

/// Build a refinement graph and run R007-R010, R012.
pub fn check_refinement(registry: &SpecRegistry) -> Vec<Diagnostic> {
    let mut diags = Vec::new();

    // Build the refinement graph
    let mut graph = DiGraph::<String, ()>::new();
    let mut node_map: BTreeMap<String, petgraph::graph::NodeIndex> = BTreeMap::new();

    // Add all documents as nodes
    for doc in &registry.documents {
        let id = doc.id_str();
        let idx = graph.add_node(id.clone());
        node_map.insert(id, idx);
    }

    // Add refinement edges (child -> parent). Both REQ and TASK can refine.
    for doc in &registry.documents {
        let refines: &[String] = match &doc.type_fields {
            TypeSpecificFields::Requirement { refines, .. } => refines,
            TypeSpecificFields::Task { refines, .. } => refines,
            _ => continue,
        };
        let child_id = doc.id_str();
        let child_node = node_map[&child_id];

        for refine_target in refines {
            // Extract the doc ID (strip anchor)
            let parent_doc_id = if let Some(pos) = refine_target.find('#') {
                &refine_target[..pos]
            } else {
                refine_target.as_str()
            };

            if let Some(&parent_node) = node_map.get(parent_doc_id) {
                graph.add_edge(child_node, parent_node, ());
            }

            // R008: Check that the clause exists on the parent
            if let Some(anchor) = refine_target.find('#').map(|p| &refine_target[p + 1..]) {
                if let Some(parent_doc) = registry.get_by_id(parent_doc_id) {
                    let has_clause = parent_doc.blocks.iter().any(|block| {
                        block.clauses.iter().any(|c| c.id == anchor)
                            || block.id == anchor
                    });
                    if !has_clause {
                        diags.push(Diagnostic::error(
                            "R008",
                            format!(
                                "refinement target '{}' — clause '{}' not found on parent",
                                refine_target, anchor
                            ),
                            doc.source_path.clone(),
                        ));
                    }
                } else {
                    // Parent doc not found — R005 will catch this separately
                }
            }
        }
    }

    // R007: Acyclic refinement graph
    if is_cyclic_directed(&graph) {
        diags.push(Diagnostic::error(
            "R007",
            "refinement graph contains a cycle",
            registry.specs_dir.clone(),
        ));
    }

    // R009: Level monotonicity
    diags.extend(check_level_monotonicity(registry));

    // R010: Coverage
    diags.extend(check_coverage(registry));

    // R012: aspects required for multi-parent refinement
    diags.extend(check_aspects(registry));

    diags
}

/// R009: Level monotonicity — a MUST clause cannot be refined exclusively
/// by SHOULD/MAY children.
fn check_level_monotonicity(registry: &SpecRegistry) -> Vec<Diagnostic> {
    let mut diags = Vec::new();

    // For each parent clause, find all refining children and their levels
    for parent_doc in &registry.documents {
        if let TypeSpecificFields::Requirement {
            level: parent_level,
            level_monotonic,
            ..
        } = &parent_doc.type_fields
        {
            if !level_monotonic {
                continue;
            }

            let parent_id = parent_doc.id_str();

            // Find all clauses in this parent
            for block in &parent_doc.blocks {
                let clause_level = block
                    .level
                    .as_deref()
                    .and_then(Level::from_str_val)
                    .unwrap_or(*parent_level);

                if clause_level != Level::Must {
                    continue;
                }

                // Check clause anchors
                for clause in &block.clauses {
                    let clause_ref = format!("{}#{}", parent_id, clause.id);
                    let child_levels = find_refining_levels(registry, &clause_ref);

                    if !child_levels.is_empty()
                        && !child_levels.contains(&Level::Must)
                    {
                        diags.push(
                            Diagnostic::error(
                                "R009",
                                format!(
                                    "MUST clause '{}' is refined only by SHOULD/MAY children",
                                    clause_ref
                                ),
                                parent_doc.source_path.clone(),
                            )
                            .at_line(clause.line),
                        );
                    }
                }

                // Also check if the block id itself is referenced
                if !block.id.is_empty() && block.clauses.is_empty() {
                    let block_ref = format!("{}#{}", parent_id, block.id);
                    let child_levels = find_refining_levels(registry, &block_ref);
                    if !child_levels.is_empty() && !child_levels.contains(&Level::Must) {
                        diags.push(
                            Diagnostic::error(
                                "R009",
                                format!(
                                    "MUST block '{}' is refined only by SHOULD/MAY children",
                                    block_ref
                                ),
                                parent_doc.source_path.clone(),
                            )
                            .at_line(block.start_line),
                        );
                    }
                }
            }
        }
    }

    diags
}

fn find_refining_levels(registry: &SpecRegistry, clause_ref: &str) -> Vec<Level> {
    let mut levels = Vec::new();
    for doc in &registry.documents {
        if let TypeSpecificFields::Requirement {
            level, ref refines, ..
        } = doc.type_fields
        {
            if refines.iter().any(|r| r == clause_ref) {
                levels.push(level);
            }
        }
    }
    levels
}

/// R010: Every clause on a non-leaf parent has at least one refining child (warning).
fn check_coverage(registry: &SpecRegistry) -> Vec<Diagnostic> {
    let mut diags = Vec::new();

    // Collect all refines targets (REQ and TASK both refine).
    let mut refined_targets: BTreeSet<String> = BTreeSet::new();
    for doc in &registry.documents {
        let refines: &[String] = match &doc.type_fields {
            TypeSpecificFields::Requirement { refines, .. } => refines,
            TypeSpecificFields::Task { refines, .. } => refines,
            _ => continue,
        };
        for r in refines {
            refined_targets.insert(r.clone());
        }
    }

    // Check each parent's clauses
    for doc in &registry.documents {
        let doc_id = doc.id_str();
        for block in &doc.blocks {
            for clause in &block.clauses {
                let clause_ref = format!("{doc_id}#{}", clause.id);
                if !refined_targets.contains(&clause_ref) {
                    diags.push(
                        Diagnostic::warning(
                            "R010",
                            format!("clause '{}' has no refining children", clause_ref),
                            doc.source_path.clone(),
                        )
                        .at_line(clause.line),
                    );
                }
            }
        }
    }

    diags
}

/// R012: `aspects:` required when refinement references multiple parents.
fn check_aspects(registry: &SpecRegistry) -> Vec<Diagnostic> {
    let mut diags = Vec::new();

    for doc in &registry.documents {
        let (refines, aspects): (&[String], &[String]) = match &doc.type_fields {
            TypeSpecificFields::Requirement { refines, aspects, .. } => (refines, aspects),
            TypeSpecificFields::Task { refines, aspects, .. } => (refines, aspects),
            _ => continue,
        };
        {
            if refines.len() > 1 {
                // Collect distinct parent doc IDs
                let parent_ids: BTreeSet<&str> = refines
                    .iter()
                    .map(|r| {
                        if let Some(pos) = r.find('#') {
                            &r[..pos]
                        } else {
                            r.as_str()
                        }
                    })
                    .collect();

                if parent_ids.len() > 1 || refines.len() > 1 {
                    if aspects.is_empty() {
                        diags.push(Diagnostic::error(
                            "R012",
                            format!(
                                "multi-parent refinement requires 'aspects' field (refines {} targets)",
                                refines.len()
                            ),
                            doc.source_path.clone(),
                        ));
                    }
                }
            }
        }
    }

    diags
}
