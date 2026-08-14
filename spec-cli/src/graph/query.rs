use std::collections::{BTreeMap, VecDeque};

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

/// List refining descendants up to `max_depth`, ordered by depth then ID.
pub fn descendants(registry: &SpecRegistry, id: &str, max_depth: usize) -> Vec<(String, usize)> {
    walk(
        &SpecGraph::refinement(registry),
        id,
        Direction::Incoming,
        max_depth,
    )
}

/// List refined-by ancestors up to `max_depth`, ordered by depth then ID.
pub fn transitive_ancestors(
    registry: &SpecRegistry,
    id: &str,
    max_depth: usize,
) -> Vec<(String, usize)> {
    walk(
        &SpecGraph::refinement(registry),
        id,
        Direction::Outgoing,
        max_depth,
    )
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

/// List synthesized hierarchy descendants up to `max_depth`.
pub fn hierarchy_descendants(
    registry: &SpecRegistry,
    id: &str,
    max_depth: usize,
) -> Vec<(String, usize)> {
    walk(
        &SpecGraph::hierarchy(registry),
        id,
        Direction::Incoming,
        max_depth,
    )
}

fn walk(
    graph: &SpecGraph,
    id: &str,
    direction: Direction,
    max_depth: usize,
) -> Vec<(String, usize)> {
    let Some(&start) = graph.node_map.get(id) else {
        return Vec::new();
    };
    if max_depth == 0 {
        return Vec::new();
    }

    let mut distances = BTreeMap::<String, usize>::new();
    let mut queue = VecDeque::from([(start, 0usize)]);
    while let Some((node, depth)) = queue.pop_front() {
        if depth == max_depth {
            continue;
        }
        let mut neighbors = graph
            .graph
            .neighbors_directed(node, direction)
            .collect::<Vec<_>>();
        neighbors.sort_by(|left, right| graph.graph[*left].cmp(&graph.graph[*right]));
        for neighbor in neighbors {
            let next_depth = depth + 1;
            let child_id = graph.graph[neighbor].clone();
            let is_shorter = match distances.get(&child_id) {
                Some(known) => next_depth < *known,
                None => true,
            };
            if is_shorter {
                distances.insert(child_id, next_depth);
                queue.push_back((neighbor, next_depth));
            }
        }
    }
    let mut result = distances.into_iter().collect::<Vec<_>>();
    result.sort_by(|(left_id, left_depth), (right_id, right_depth)| {
        left_depth
            .cmp(right_depth)
            .then_with(|| left_id.cmp(right_id))
    });
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
}

pub fn coverage(registry: &SpecRegistry, id: &str) -> Vec<CoverageEntry> {
    let Some(doc) = registry.get_by_id(id) else {
        return vec![];
    };

    // Collect only durable refining requirements, keyed by anchor.
    let mut req_children: std::collections::BTreeMap<String, Vec<String>> =
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
            });
        }
    }

    entries
}

/// List work items that address one durable specification or exact anchor.
pub fn addressed_by(registry: &SpecRegistry, target: &str) -> Vec<String> {
    let target_document = target.split_once('#').map(|(id, _)| id).unwrap_or(target);
    let exact_anchor = target.contains('#');
    let mut tasks = registry
        .documents
        .iter()
        .filter_map(|document| {
            let TypeSpecificFields::Task { addresses, .. } = &document.type_fields else {
                return None;
            };
            addresses
                .iter()
                .any(|address| {
                    if exact_anchor {
                        address == target
                    } else {
                        address.split_once('#').map(|(id, _)| id).unwrap_or(address)
                            == target_document
                    }
                })
                .then(|| document.id_str())
        })
        .collect::<Vec<_>>();
    tasks.sort();
    tasks.dedup();
    tasks
}
