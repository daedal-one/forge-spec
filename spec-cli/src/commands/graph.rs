use std::path::Path;

use anyhow::Result;

use crate::graph::build::SpecGraph;
use crate::graph::dot::render_dot;
use crate::model::registry::SpecRegistry;

pub fn run(
    specs_dir: &Path,
    hierarchy: bool,
    refinement: bool,
    categorization: bool,
) -> Result<()> {
    let registry = SpecRegistry::load(specs_dir)?;

    let show_hierarchy = hierarchy || (!refinement && !categorization);

    if show_hierarchy {
        let graph = SpecGraph::hierarchy(&registry);
        let dot = render_dot(&graph, &registry, "project hierarchy");
        print!("{dot}");
    }

    if refinement {
        let graph = SpecGraph::refinement(&registry);
        let dot = render_dot(&graph, &registry, "refinement");
        print!("{dot}");
    }

    if categorization {
        let graph = SpecGraph::categorization(&registry);
        let dot = render_dot(&graph, &registry, "categorization");
        print!("{dot}");
    }

    Ok(())
}
