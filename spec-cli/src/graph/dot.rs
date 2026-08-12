use super::build::SpecGraph;
use crate::model::registry::SpecRegistry;

/// Generate DOT output for a graph.
pub fn render_dot(graph: &SpecGraph, registry: &SpecRegistry, title: &str) -> String {
    let mut out = String::new();
    out.push_str(&format!("digraph \"{title}\" {{\n"));
    out.push_str("  rankdir=BT;\n");
    out.push_str("  node [shape=box, style=filled, fillcolor=\"#e8e8e8\"];\n\n");

    // Nodes
    for (id, &node_idx) in &graph.node_map {
        let has_edges = graph.graph.neighbors_undirected(node_idx).next().is_some();
        if !has_edges {
            continue; // Skip isolated nodes
        }

        let summary = registry
            .get_by_id(id)
            .and_then(|doc| doc.universal.summary.as_deref())
            .unwrap_or("");
        let label = if summary.is_empty() {
            id.clone()
        } else {
            let short = if summary.len() > 60 {
                format!("{}...", &summary[..57])
            } else {
                summary.to_string()
            };
            format!("{id}\\n{short}")
        };
        out.push_str(&format!(
            "  \"{}\" [label=\"{}\"];\n",
            id,
            label.replace('"', "\\\"")
        ));
    }

    out.push('\n');

    // Edges
    for edge in graph.graph.edge_indices() {
        let (src, dst) = graph.graph.edge_endpoints(edge).unwrap();
        let src_id = &graph.graph[src];
        let dst_id = &graph.graph[dst];
        let label = &graph.graph[edge];

        if label.is_empty() {
            out.push_str(&format!("  \"{src_id}\" -> \"{dst_id}\";\n"));
        } else {
            out.push_str(&format!(
                "  \"{src_id}\" -> \"{dst_id}\" [label=\"{label}\"];\n"
            ));
        }
    }

    out.push_str("}\n");
    out
}
