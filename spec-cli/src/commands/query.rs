use std::path::Path;

use anyhow::Result;

use crate::graph;
use crate::model::registry::SpecRegistry;

pub fn children(specs_dir: &Path, id: &str) -> Result<()> {
    let registry = SpecRegistry::load(specs_dir)?;
    let children = graph::query::children(&registry, id);

    if children.is_empty() {
        println!("No children found for '{id}'");
    } else {
        println!("Children of {id}:");
        for child in &children {
            let summary = registry
                .get_by_id(child)
                .and_then(|d| d.universal.summary.as_deref())
                .unwrap_or("");
            if summary.is_empty() {
                println!("  {child}");
            } else {
                println!("  {child} — {}", summary.trim());
            }
        }
    }

    Ok(())
}

pub fn ancestors(specs_dir: &Path, id: &str) -> Result<()> {
    let registry = SpecRegistry::load(specs_dir)?;
    let ancestors = graph::query::ancestors(&registry, id);

    if ancestors.is_empty() {
        println!("No ancestors found for '{id}'");
    } else {
        println!("Ancestors of {id}:");
        for anc in &ancestors {
            let summary = registry
                .get_by_id(anc)
                .and_then(|d| d.universal.summary.as_deref())
                .unwrap_or("");
            if summary.is_empty() {
                println!("  {anc}");
            } else {
                println!("  {anc} — {}", summary.trim());
            }
        }
    }

    Ok(())
}

pub fn orphans(specs_dir: &Path) -> Result<()> {
    let registry = SpecRegistry::load(specs_dir)?;
    let orphans = graph::query::orphans(&registry);

    if orphans.is_empty() {
        println!("No orphan specs found.");
    } else {
        println!("Orphan specs (no refinement relationships):");
        for id in &orphans {
            println!("  {id}");
        }
    }

    Ok(())
}

pub fn coverage(specs_dir: &Path, id: &str) -> Result<()> {
    let registry = SpecRegistry::load(specs_dir)?;
    let entries = graph::query::coverage(&registry, id);

    if entries.is_empty() {
        println!("No clauses found for '{id}'");
        return Ok(());
    }

    println!("Coverage for {id}:");
    for entry in &entries {
        let status = if entry.refined_by.is_empty() {
            "UNCOVERED"
        } else {
            "covered"
        };
        println!("  #{} ({}) — {}", entry.clause_id, status, entry.clause_text);
        for child in &entry.refined_by {
            println!("    refined by: {child}");
        }
    }

    Ok(())
}
