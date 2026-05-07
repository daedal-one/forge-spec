use std::path::Path;

use anyhow::{bail, Result};

use crate::cli::RenderTarget;
use crate::model::registry::SpecRegistry;
use crate::render::scope::{compute_scope, DetailLevel};
use crate::render::{agent, human};

pub fn run(
    specs_dir: &Path,
    id_or_query: &str,
    target: &RenderTarget,
    depth: Option<usize>,
    ancestors: &str,
    descendants: &str,
) -> Result<()> {
    let registry = SpecRegistry::load(specs_dir)?;

    // Resolve the query — support exact ID or simple glob
    let focal_ids = resolve_query(&registry, id_or_query)?;

    if focal_ids.is_empty() {
        bail!("no specs match '{id_or_query}'");
    }

    let ancestor_detail = DetailLevel::from_str_val(ancestors);
    let descendant_detail = DetailLevel::from_str_val(descendants);

    for focal_id in &focal_ids {
        let entries = compute_scope(&registry, focal_id, ancestor_detail, descendant_detail, depth);

        let output = match target {
            RenderTarget::Human => human::render_human(&registry, &entries),
            RenderTarget::Agent => agent::render_agent(&registry, &entries),
        };

        print!("{output}");
    }

    Ok(())
}

fn resolve_query(registry: &SpecRegistry, query: &str) -> Result<Vec<String>> {
    // Exact ID match
    if registry.get_by_id(query).is_some() {
        return Ok(vec![query.to_string()]);
    }

    // Simple glob: REQ:auth/* matches REQ:auth/anything
    if query.contains('*') {
        let prefix = query.trim_end_matches('*');
        let matches: Vec<String> = registry
            .id_index
            .keys()
            .filter(|id| id.starts_with(prefix))
            .cloned()
            .collect();
        if matches.is_empty() {
            bail!("no specs match pattern '{query}'");
        }
        return Ok(matches);
    }

    bail!("spec not found: '{query}'")
}
