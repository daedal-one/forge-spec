use std::path::Path;

use anyhow::Result;

use crate::model::registry::SpecRegistry;

pub fn run(specs_dir: &Path) -> Result<()> {
    let registry = SpecRegistry::load(specs_dir)?;

    if registry.redirects.is_empty() {
        println!("No redirects to apply.");
        return Ok(());
    }

    println!(
        "Processing {} redirect(s)...",
        registry.redirects.len()
    );

    let mut total_rewrites = 0;

    for doc in &registry.documents {
        let content = std::fs::read_to_string(&doc.source_path)?;
        let mut new_content = content.clone();
        let mut doc_rewrites = 0;

        for redirect in &registry.redirects {
            // Rewrite spec: references in body
            let from_ref = format!("spec:{}", redirect.from);
            let to_ref = format!("spec:{}", redirect.to);
            if new_content.contains(&from_ref) {
                new_content = new_content.replace(&from_ref, &to_ref);
                doc_rewrites += 1;
            }

            // Rewrite bare references in frontmatter (refines, related, etc.)
            // These appear as plain strings, not as spec: URLs
            if new_content.contains(&redirect.from) {
                new_content = new_content.replace(&redirect.from, &redirect.to);
                doc_rewrites += 1;
            }
        }

        if new_content != content {
            std::fs::write(&doc.source_path, new_content)?;
            println!(
                "  {} — {} rewrite(s)",
                doc.source_path.display(),
                doc_rewrites
            );
            total_rewrites += doc_rewrites;
        }
    }

    if total_rewrites == 0 {
        println!("No references needed rewriting.");
    } else {
        println!("Applied {total_rewrites} rewrite(s) total.");
    }

    Ok(())
}
