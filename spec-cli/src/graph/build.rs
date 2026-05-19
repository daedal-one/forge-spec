use std::collections::BTreeMap;

use petgraph::graph::{DiGraph, NodeIndex};

use crate::model::frontmatter::TypeSpecificFields;
use crate::model::registry::SpecRegistry;

/// A spec graph with node/edge data suitable for querying and DOT output.
pub struct SpecGraph {
    pub graph: DiGraph<String, String>,
    pub node_map: BTreeMap<String, NodeIndex>,
}

impl SpecGraph {
    /// Build the refinement graph. Edges go from child → parent.
    pub fn refinement(registry: &SpecRegistry) -> Self {
        let mut graph = DiGraph::new();
        let mut node_map = BTreeMap::new();

        for doc in &registry.documents {
            let id = doc.id_str();
            let idx = graph.add_node(id.clone());
            node_map.insert(id, idx);
        }

        for doc in &registry.documents {
            // Both REQs and TASKs can refine other specs; the spec format
            // treats TASK refinement the same way (a TASK refines a clause
            // on its parent REQ).
            let (refines, aspects): (&[String], &[String]) = match &doc.type_fields {
                TypeSpecificFields::Requirement {
                    refines, aspects, ..
                } => (refines, aspects),
                TypeSpecificFields::Task {
                    refines, aspects, ..
                } => (refines, aspects),
                _ => continue,
            };

            let child_id = doc.id_str();
            let child_node = node_map[&child_id];

            for (i, refine_target) in refines.iter().enumerate() {
                let parent_doc_id = if let Some(pos) = refine_target.find('#') {
                    &refine_target[..pos]
                } else {
                    refine_target.as_str()
                };

                if let Some(&parent_node) = node_map.get(parent_doc_id) {
                    let label = if aspects.len() > i {
                        aspects[i].clone()
                    } else {
                        String::new()
                    };
                    graph.add_edge(child_node, parent_node, label);
                }
            }
        }

        Self { graph, node_map }
    }

    /// Build the categorization graph. Edges go from doc → topic.
    pub fn categorization(registry: &SpecRegistry) -> Self {
        let mut graph = DiGraph::new();
        let mut node_map = BTreeMap::new();

        for doc in &registry.documents {
            let id = doc.id_str();
            let idx = graph.add_node(id.clone());
            node_map.insert(id, idx);
        }

        for doc in &registry.documents {
            // Same story for categorization — TASKs can also be categorized
            // under a TOPIC and should appear as the topic's children.
            let categorized_under: &[String] = match &doc.type_fields {
                TypeSpecificFields::Requirement {
                    categorized_under, ..
                } => categorized_under,
                TypeSpecificFields::Task {
                    categorized_under, ..
                } => categorized_under,
                _ => continue,
            };

            let doc_id = doc.id_str();
            let doc_node = node_map[&doc_id];

            for topic_id in categorized_under {
                let topic_doc = if let Some(pos) = topic_id.find('#') {
                    &topic_id[..pos]
                } else {
                    topic_id.as_str()
                };
                if let Some(&topic_node) = node_map.get(topic_doc) {
                    graph.add_edge(doc_node, topic_node, String::new());
                }
            }
        }

        Self { graph, node_map }
    }
}
