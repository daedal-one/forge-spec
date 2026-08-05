use std::path::Path;

use anyhow::Result;

use crate::model::config::CURRENT_SPEC_BASELINE;
use crate::model::registry::SpecRegistry;
use walkdir::WalkDir;

pub fn run(specs_dir: &Path) -> Result<()> {
    let format_updates = migrate_format_headers(specs_dir)?;
    ensure_spec_config(specs_dir)?;
    let registry = SpecRegistry::load(specs_dir)?;

    if registry.redirects.is_empty() {
        if format_updates == 0 {
            println!("No format or redirect migrations needed.");
        } else {
            println!("Migrated {format_updates} document(s) to {CURRENT_SPEC_BASELINE}.");
        }
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

fn ensure_spec_config(specs_dir: &Path) -> Result<()> {
    let path = specs_dir.join("_config.toml");
    if !path.exists() {
        std::fs::write(path, format!("baseline = \"{CURRENT_SPEC_BASELINE}\"\n"))?;
    }
    Ok(())
}

fn migrate_format_headers(specs_dir: &Path) -> Result<usize> {
    let mut updates = 0;
    for entry in WalkDir::new(specs_dir)
        .into_iter()
        .filter_map(|entry| entry.ok())
        .filter(|entry| {
            entry.file_type().is_file()
                && entry
                    .file_name()
                    .to_str()
                    .map(|name| name.ends_with(".spec.md"))
                    .unwrap_or(false)
        })
    {
        let path = entry.path();
        let content = std::fs::read_to_string(path)?;
        let migrated = remove_derived_frontmatter(&content)?;
        if migrated != content {
            std::fs::write(path, migrated)?;
            updates += 1;
        }
    }
    Ok(updates)
}

fn remove_derived_frontmatter(content: &str) -> Result<String> {
    let close = content
        .strip_prefix("---")
        .and_then(|rest| rest.find("\n---").map(|offset| offset + 3))
        .ok_or_else(|| anyhow::anyhow!("invalid spec frontmatter"))?;
    let (frontmatter, body) = content.split_at(close);
    let mut output = String::with_capacity(content.len());
    for line in frontmatter.split_inclusive('\n') {
        if !line.starts_with("version:")
            && !line.starts_with("SpecBaseline:")
            && !line.starts_with("spec_baseline:")
        {
            output.push_str(line);
        }
    }
    output.push_str(body);
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn removes_legacy_document_version() {
        let input = "---\nid: REQ:a/b\nversion: 1.4.0\nowners: [c]\n---\nBody\n";
        let migrated = remove_derived_frontmatter(input).unwrap();
        assert!(!migrated.contains("version:"));
        assert!(!migrated.contains("SpecBaseline:"));
        assert!(migrated.ends_with("---\nBody\n"));
    }

    #[test]
    fn removes_all_legacy_version_keys() {
        let input = "---\nid: REQ:a/b\nversion: 7\nspec_baseline: old\n---\nBody\n";
        let migrated = remove_derived_frontmatter(input).unwrap();
        assert!(!migrated.contains("version:"));
        assert!(!migrated.contains("spec_baseline:"));
    }

    #[test]
    fn creates_tree_level_baseline_without_overwriting_existing_config() {
        let temp = tempfile::tempdir().unwrap();
        ensure_spec_config(temp.path()).unwrap();
        let path = temp.path().join("_config.toml");
        assert_eq!(
            std::fs::read_to_string(&path).unwrap(),
            "baseline = \"forge-spec-v0.2.0\"\n"
        );
        std::fs::write(&path, "baseline = \"forge-spec-v9.0.0\"\n").unwrap();
        ensure_spec_config(temp.path()).unwrap();
        assert!(std::fs::read_to_string(path).unwrap().contains("v9.0.0"));
    }
}
