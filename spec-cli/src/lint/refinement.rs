use std::collections::{BTreeMap, BTreeSet};

use petgraph::algo::is_cyclic_directed;
use petgraph::graph::DiGraph;

use crate::model::frontmatter::{Level, TypeSpecificFields};
use crate::model::id::EntityType;
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

    // Add durable requirement refinement edges (child -> parent).
    for doc in &registry.documents {
        let refines: &[String] = match &doc.type_fields {
            TypeSpecificFields::Requirement { refines, .. } => refines,
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

            if let Some(parent_doc) = registry.get_by_id(parent_doc_id) {
                if parent_doc.universal.entity_type != EntityType::Req {
                    diags.push(Diagnostic::error(
                        "R008",
                        format!(
                            "refinement target '{}' must resolve to a requirement",
                            refine_target
                        ),
                        doc.source_path.clone(),
                    ));
                    continue;
                }
            }

            // R008: Check that the clause exists on the parent
            if let Some(anchor) = refine_target.find('#').map(|p| &refine_target[p + 1..]) {
                if let Some(parent_doc) = registry.get_by_id(parent_doc_id) {
                    let has_clause = parent_doc.blocks.iter().any(|block| {
                        block.clauses.iter().any(|c| c.id == anchor) || block.id == anchor
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

    // R032: work-item links are typed, resolvable, and orthogonal.
    diags.extend(check_work_items(registry));

    diags
}

fn check_work_items(registry: &SpecRegistry) -> Vec<Diagnostic> {
    let mut diagnostics = Vec::new();
    for document in &registry.documents {
        let TypeSpecificFields::Task {
            addresses,
            groups,
            blocked_by,
            ..
        } = &document.type_fields
        else {
            continue;
        };
        for address in addresses {
            let (exists, _) = registry.reference_exists(address);
            if !exists {
                diagnostics.push(Diagnostic::error(
                    "R032",
                    format!("TASK address target '{address}' does not resolve"),
                    document.source_path.clone(),
                ));
                continue;
            }
            let (resolved, _) = registry.resolve_redirect(address);
            let target_id = resolved
                .split_once('#')
                .map(|(id, _)| id)
                .unwrap_or(&resolved);
            if registry
                .get_by_id(target_id)
                .is_some_and(|target| target.universal.entity_type == EntityType::Task)
            {
                diagnostics.push(Diagnostic::error(
                    "R032",
                    format!("TASK address target '{address}' is another work item"),
                    document.source_path.clone(),
                ));
            }
        }
        for group in groups {
            if registry
                .get_by_id(group)
                .is_none_or(|target| target.universal.entity_type != EntityType::Topic)
            {
                diagnostics.push(Diagnostic::error(
                    "R032",
                    format!("TASK group '{group}' must resolve to TOPIC"),
                    document.source_path.clone(),
                ));
            }
        }
        for blocker in blocked_by {
            if registry
                .get_by_id(blocker)
                .is_none_or(|target| target.universal.entity_type != EntityType::Task)
            {
                diagnostics.push(Diagnostic::error(
                    "R032",
                    format!("TASK blocker '{blocker}' must resolve to TASK"),
                    document.source_path.clone(),
                ));
            }
        }
    }
    diagnostics
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

                    if !child_levels.is_empty() && !child_levels.contains(&Level::Must) {
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

    // Collect durable requirement refinement targets.
    let mut refined_targets: BTreeSet<String> = BTreeSet::new();
    for doc in &registry.documents {
        let refines: &[String] = match &doc.type_fields {
            TypeSpecificFields::Requirement { refines, .. } => refines,
            _ => continue,
        };
        for r in refines {
            refined_targets.insert(r.clone());
        }
    }

    // Check each parent's clauses
    for doc in &registry.documents {
        if doc.universal.entity_type != EntityType::Req {
            continue;
        }
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
            TypeSpecificFields::Requirement {
                refines, aspects, ..
            } => (refines, aspects),
            _ => continue,
        };
        {
            if refines.len() > 1 && aspects.is_empty() {
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

    diags
}

#[cfg(test)]
mod tests {
    use super::*;

    fn registry_with_interface_and_task(task_refines: &str) -> SpecRegistry {
        let temp = tempfile::tempdir().unwrap();
        let specs = temp.keep();
        std::fs::write(
            specs.join("_config.toml"),
            "baseline = \"forge-spec-v0.6.0\"\nproject = \"PROJECT:demo\"\n",
        )
        .unwrap();
        std::fs::write(
            specs.join("_project.spec.md"),
            "---\nid: PROJECT:demo\ntype: project\nstatus: accepted\nsummary: Demo.\nowners: [dev]\n---\n\n# Demo\n",
        )
        .unwrap();
        std::fs::write(
            specs.join("interface.spec.md"),
            "---\nid: IFC:demo/service\ntype: interface\nstatus: accepted\nsummary: Service contract.\nowners: [dev]\nconsumed_by: [TASK:demo/implementation]\nprovided_by: [PROJECT:demo]\nstability: experimental\n---\n\n# Service\n\n:::{interface id=\"contract\" level=\"MUST\"}\n- {#c-call} Calls MUST be local.\n:::\n",
        )
        .unwrap();
        std::fs::write(
            specs.join("task.spec.md"),
            format!(
                "---\nid: TASK:demo/implementation\ntype: task\nstatus: accepted\nsummary: Implement the service.\nowners: [dev]\nprogress: done\naddresses: {task_refines}\nlabels: []\ngroups: []\nassignee: dev\neta:\nblocked_by: []\n---\n\n# Implementation\n"
            ),
        )
        .unwrap();
        SpecRegistry::load(&specs).unwrap()
    }

    #[test]
    fn coverage_ignores_interface_clauses() {
        let registry = registry_with_interface_and_task("[]");
        assert!(check_refinement(&registry)
            .iter()
            .all(|diagnostic| diagnostic.code != "R010"));
    }

    #[test]
    fn task_addresses_may_target_non_requirement_intent() {
        let registry = registry_with_interface_and_task("[IFC:demo/service#c-call]");
        let diagnostics = check_refinement(&registry);
        assert!(diagnostics
            .iter()
            .all(|diagnostic| diagnostic.code != "R008" && diagnostic.code != "R032"));
    }
}
