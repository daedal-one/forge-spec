use std::path::Path;

use anyhow::Result;

use crate::graph;
use crate::model::registry::SpecRegistry;

pub fn relations(specs_dir: &Path, id: &str) -> Result<()> {
    let registry = SpecRegistry::load(specs_dir)?;
    let document = registry
        .get_by_id(id)
        .ok_or_else(|| anyhow::anyhow!("no spec with id '{id}'"))?;
    println!("Relations for {id}:");
    let ancestors = graph::query::ancestors(&registry, id);
    let children = graph::query::children(&registry, id);
    print_group("refines", &ancestors);
    print_group("refined by", &children);
    let categorized = match &document.type_fields {
        crate::model::frontmatter::TypeSpecificFields::Requirement {
            categorized_under, ..
        }
        | crate::model::frontmatter::TypeSpecificFields::Task {
            categorized_under, ..
        } => categorized_under.as_slice(),
        _ => &[],
    };
    print_group("categorized under", categorized);
    print_group("related", &document.universal.related);
    if let Some(value) = &document.universal.supersedes {
        print_group("supersedes", std::slice::from_ref(value));
    }
    if let Some(value) = &document.universal.superseded_by {
        print_group("superseded by", std::slice::from_ref(value));
    }
    Ok(())
}

fn print_group(label: &str, values: &[String]) {
    if values.is_empty() {
        println!("  {label}: (none)");
    } else {
        println!("  {label}:");
        for value in values {
            println!("    {value}");
        }
    }
}

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
        let status = match (entry.refined_by.is_empty(), entry.tasks.is_empty()) {
            (true, true) => "UNCOVERED".to_string(),
            (false, true) => "covered".to_string(),
            (_, false) => {
                use crate::model::frontmatter::Progress as P;
                // WontDo tasks are intentional non-work and excluded from the
                // denominator — counting them as outstanding would suggest
                // there's still something to do when the team has already
                // decided otherwise.
                let total = entry
                    .tasks
                    .iter()
                    .filter(|t| !matches!(t.progress, P::WontDo))
                    .count();
                let done = entry
                    .tasks
                    .iter()
                    .filter(|t| matches!(t.progress, P::Done))
                    .count();
                if total == 0 {
                    "wontdo".to_string()
                } else {
                    format!("tasks {done}/{total}")
                }
            }
        };
        println!(
            "  #{} ({}) — {}",
            entry.clause_id, status, entry.clause_text
        );
        for child in &entry.refined_by {
            println!("    refined by: {child}");
        }
        for task in &entry.tasks {
            println!("    task [{}] {}", task.progress.as_str(), task.id);
        }
    }

    Ok(())
}
