use std::path::Path;

use anyhow::Result;

use crate::cli::GraphView;
use crate::graph::build::SpecGraph;
use crate::graph::dot::render_dot;
use crate::model::registry::SpecRegistry;

pub fn run(specs_dir: &Path, view: GraphView) -> Result<()> {
    let registry = SpecRegistry::load(specs_dir)?;
    let (graph, label) = match view {
        GraphView::Hierarchy => (SpecGraph::hierarchy(&registry), "project hierarchy"),
        GraphView::Refinement => (SpecGraph::refinement(&registry), "refinement"),
        GraphView::Categorization => (SpecGraph::categorization(&registry), "categorization"),
        GraphView::Work => (SpecGraph::work(&registry), "work items"),
    };
    print!("{}", render_dot(&graph, &registry, label));
    Ok(())
}
