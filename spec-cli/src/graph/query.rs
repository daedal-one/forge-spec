use petgraph::Direction;

use super::build::SpecGraph;
use crate::model::frontmatter::TypeSpecificFields;
use crate::model::registry::SpecRegistry;

/// List direct refining children of a spec.
pub fn children(registry: &SpecRegistry, id: &str) -> Vec<String> {
    let graph = SpecGraph::refinement(registry);
    let Some(&node) = graph.node_map.get(id) else {
        return vec![];
    };

    let mut result: Vec<String> = graph
        .graph
        .neighbors_directed(node, Direction::Incoming)
        .map(|n| graph.graph[n].clone())
        .collect();
    result.sort();
    result.dedup();
    result
}

/// List direct refined-by parents of a spec.
pub fn ancestors(registry: &SpecRegistry, id: &str) -> Vec<String> {
    let graph = SpecGraph::refinement(registry);
    let Some(&node) = graph.node_map.get(id) else {
        return vec![];
    };

    let mut result: Vec<String> = graph
        .graph
        .neighbors_directed(node, Direction::Outgoing)
        .map(|n| graph.graph[n].clone())
        .collect();
    result.sort();
    result.dedup();
    result
}

/// List direct children in the synthesized project hierarchy.
pub fn hierarchy_children(registry: &SpecRegistry, id: &str) -> Vec<String> {
    let graph = SpecGraph::hierarchy(registry);
    let Some(&node) = graph.node_map.get(id) else {
        return vec![];
    };
    let mut result = graph
        .graph
        .neighbors_directed(node, Direction::Incoming)
        .map(|index| graph.graph[index].clone())
        .collect::<Vec<_>>();
    result.sort();
    result.dedup();
    result
}

/// List specs with no refinement relationships (no parents and no children).
pub fn orphans(registry: &SpecRegistry) -> Vec<String> {
    let graph = SpecGraph::refinement(registry);
    let mut result: Vec<String> = Vec::new();

    for (id, &node) in &graph.node_map {
        let has_parents = graph
            .graph
            .neighbors_directed(node, Direction::Outgoing)
            .next()
            .is_some();
        let has_children = graph
            .graph
            .neighbors_directed(node, Direction::Incoming)
            .next()
            .is_some();
        // Also check if referenced by or references anything
        if !has_parents && !has_children {
            result.push(id.clone());
        }
    }

    result.sort();
    result
}

/// Clause-by-clause coverage report for a spec.
pub struct CoverageEntry {
    pub clause_id: String,
    pub clause_text: String,
    pub refined_by: Vec<String>,
    /// Tasks that refine this clause, with their progress states.
    pub tasks: Vec<TaskCoverage>,
}

/// Coverage info for a single task refining a clause.
pub struct TaskCoverage {
    pub id: String,
    pub progress: crate::model::frontmatter::Progress,
}

pub fn coverage(registry: &SpecRegistry, id: &str) -> Vec<CoverageEntry> {
    let Some(doc) = registry.get_by_id(id) else {
        return vec![];
    };

    // Collect refining requirements and tasks pointing to this doc, keyed by anchor.
    let mut req_children: std::collections::BTreeMap<String, Vec<String>> =
        std::collections::BTreeMap::new();
    let mut task_children: std::collections::BTreeMap<String, Vec<TaskCoverage>> =
        std::collections::BTreeMap::new();

    for child_doc in &registry.documents {
        match &child_doc.type_fields {
            TypeSpecificFields::Requirement { refines, .. } => {
                for refine_target in refines {
                    if let Some(anchor_pos) = refine_target.find('#') {
                        let doc_id = &refine_target[..anchor_pos];
                        let anchor = &refine_target[anchor_pos + 1..];
                        if doc_id == id {
                            req_children
                                .entry(anchor.to_string())
                                .or_default()
                                .push(child_doc.id_str());
                        }
                    }
                }
            }
            TypeSpecificFields::Task { refines, progress, .. } => {
                for refine_target in refines {
                    if let Some(anchor_pos) = refine_target.find('#') {
                        let doc_id = &refine_target[..anchor_pos];
                        let anchor = &refine_target[anchor_pos + 1..];
                        if doc_id == id {
                            task_children
                                .entry(anchor.to_string())
                                .or_default()
                                .push(TaskCoverage {
                                    id: child_doc.id_str(),
                                    progress: *progress,
                                });
                        }
                    }
                }
            }
            _ => {}
        }
    }

    let mut entries = Vec::new();
    for block in &doc.blocks {
        for clause in &block.clauses {
            entries.push(CoverageEntry {
                clause_id: clause.id.clone(),
                clause_text: clause.text.clone(),
                refined_by: req_children.remove(&clause.id).unwrap_or_default(),
                tasks: task_children.remove(&clause.id).unwrap_or_default(),
            });
        }
    }

    entries
}
