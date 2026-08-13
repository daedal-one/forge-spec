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

    /// Build the project hierarchy used for navigation. Explicit refinement
    /// and categorization remain distinct semantic relations; this view merges
    /// them and attaches only otherwise-unplaced documents to PROJECT.
    /// Edges go from child → parent.
    pub fn hierarchy(registry: &SpecRegistry) -> Self {
        let mut graph = DiGraph::new();
        let mut node_map = BTreeMap::new();

        for doc in &registry.documents {
            let id = doc.id_str();
            let idx = graph.add_node(id.clone());
            node_map.insert(id, idx);
        }

        let mut placed = std::collections::BTreeSet::new();
        for doc in &registry.documents {
            let child_id = doc.id_str();
            let child_node = node_map[&child_id];
            let (refines, aspects, categorized_under): (&[String], &[String], &[String]) =
                match &doc.type_fields {
                    TypeSpecificFields::Requirement {
                        refines,
                        aspects,
                        categorized_under,
                        ..
                    } => (refines, aspects, categorized_under),
                    TypeSpecificFields::Task {
                        refines,
                        aspects,
                        categorized_under,
                        ..
                    } => (refines, aspects, categorized_under),
                    _ => (&[], &[], &[]),
                };

            for (index, target) in refines.iter().enumerate() {
                if !registry.reference_exists(target).0 {
                    continue;
                }
                let (resolved, _) = registry.resolve_redirect(target);
                let parent_id = document_id(&resolved);
                let Some(&parent_node) = node_map.get(parent_id) else {
                    continue;
                };
                let label = aspects
                    .get(index)
                    .map(|aspect| format!("refines: {aspect}"))
                    .unwrap_or_else(|| "refines".to_string());
                graph.add_edge(child_node, parent_node, label);
                placed.insert(child_id.clone());
            }

            for target in categorized_under {
                if !registry.reference_exists(target).0 {
                    continue;
                }
                let (resolved, _) = registry.resolve_redirect(target);
                let parent_id = document_id(&resolved);
                let Some(&parent_node) = node_map.get(parent_id) else {
                    continue;
                };
                graph.add_edge(child_node, parent_node, "categorized".to_string());
                placed.insert(child_id.clone());
            }
        }

        if let Some(project_id) = registry.project_id() {
            let project_node = node_map[&project_id];
            for doc in &registry.documents {
                let id = doc.id_str();
                if id != project_id && !placed.contains(&id) {
                    graph.add_edge(node_map[&id], project_node, "project".to_string());
                }
            }
        }

        Self { graph, node_map }
    }
}

fn document_id(reference: &str) -> &str {
    reference
        .split_once('#')
        .map(|(document, _)| document)
        .unwrap_or(reference)
}

#[cfg(test)]
mod tests {
    use crate::model::registry::SpecRegistry;

    fn write(path: &std::path::Path, content: &str) {
        std::fs::write(path, content).unwrap();
    }

    #[test]
    fn hierarchy_attaches_only_explicit_roots_to_project() {
        let temp = tempfile::tempdir().unwrap();
        write(
            &temp.path().join("_config.toml"),
            "baseline = \"forge-spec-v0.5.0\"\nproject = \"PROJECT:demo\"\n",
        );
        write(
            &temp.path().join("_project.spec.md"),
            "---\nid: PROJECT:demo\ntype: project\nstatus: accepted\nsummary: Demo.\nowners: [dev]\n---\n\n# Demo\n",
        );
        write(
            &temp.path().join("topic.spec.md"),
            "---\nid: TOPIC:demo/core\ntype: topic\nstatus: accepted\nsummary: Core.\nowners: [dev]\n---\n\n# Core\n",
        );
        write(
            &temp.path().join("root.spec.md"),
            "---\nid: REQ:demo/root\ntype: requirement\nstatus: accepted\nsummary: Root.\nowners: [dev]\nlevel: MUST\nrefines: []\ncategorized_under: [TOPIC:demo/core]\n---\n\n# Root\n",
        );
        write(
            &temp.path().join("child.spec.md"),
            "---\nid: REQ:demo/child\ntype: requirement\nstatus: accepted\nsummary: Child.\nowners: [dev]\nlevel: MUST\nrefines: [REQ:demo/root]\n---\n\n# Child\n",
        );
        write(
            &temp.path().join("glossary.spec.md"),
            "---\nid: GLO:demo/terms\ntype: glossary\nstatus: accepted\nsummary: Terms.\nowners: [dev]\n---\n\n# Terms\n",
        );
        write(
            &temp.path().join("dangling.spec.md"),
            "---\nid: REQ:demo/dangling\ntype: requirement\nstatus: draft\nsummary: Dangling.\nowners: [dev]\nlevel: MUST\nrefines: [REQ:demo/root#missing]\n---\n\n# Dangling\n",
        );

        let registry = SpecRegistry::load(temp.path()).unwrap();
        assert_eq!(
            crate::graph::query::hierarchy_children(&registry, "PROJECT:demo"),
            vec!["GLO:demo/terms", "REQ:demo/dangling", "TOPIC:demo/core"]
        );
        assert_eq!(
            crate::graph::query::hierarchy_children(&registry, "TOPIC:demo/core"),
            vec!["REQ:demo/root"]
        );
        assert_eq!(
            crate::graph::query::hierarchy_children(&registry, "REQ:demo/root"),
            vec!["REQ:demo/child"]
        );
    }
}
