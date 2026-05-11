use std::collections::BTreeMap;
use std::path::Path;

use anyhow::{bail, Result};
use walkdir::WalkDir;

use crate::model::reference::SpecReference;
use crate::model::registry::SpecRegistry;
use crate::parse::references::extract_references;

/// A reverse reference from a knowledge-base file to a spec.
struct KbBacklink {
    kb_file: String,
    spec_id: String,
    line: usize,
    source: BacklinkSource,
}

enum BacklinkSource {
    Body,
    Frontmatter,
}

/// Scan the configured knowledge-base vault for references to specs.
pub fn run(specs_dir: &Path) -> Result<()> {
    let registry = SpecRegistry::load(specs_dir)?;

    let kb_root = match &registry.kb_root {
        Some(root) => root.clone(),
        None => {
            bail!(
                "knowledge base not configured. \
                 Add [knowledge_base] section to .specs/_config.toml"
            );
        }
    };

    if !kb_root.exists() {
        bail!(
            "knowledge base directory not found: {}",
            kb_root.display()
        );
    }

    let mut backlinks: Vec<KbBacklink> = Vec::new();
    let mut dangling: Vec<KbBacklink> = Vec::new();

    // Walk all .md files in the vault
    for entry in WalkDir::new(&kb_root)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| {
            e.file_type().is_file()
                && e.path()
                    .extension()
                    .map_or(false, |ext| ext == "md")
        })
    {
        let path = entry.path();
        let content = match std::fs::read_to_string(path) {
            Ok(c) => c,
            Err(_) => continue,
        };

        let rel_path = path
            .strip_prefix(&kb_root)
            .unwrap_or(path)
            .display()
            .to_string();

        // Parse frontmatter for `specs:` field
        if let Some(fm_refs) = extract_specs_frontmatter(&content) {
            for spec_id in fm_refs {
                let (exists, _) = registry.reference_exists(&spec_id);
                let bl = KbBacklink {
                    kb_file: rel_path.clone(),
                    spec_id: spec_id.clone(),
                    line: 0,
                    source: BacklinkSource::Frontmatter,
                };
                if exists {
                    backlinks.push(bl);
                } else {
                    dangling.push(bl);
                }
            }
        }

        // Parse body for spec: links
        let body = skip_frontmatter(&content);
        let body_start = content[..content.len() - body.len()]
            .lines()
            .count()
            + 1;
        let refs = extract_references(body, body_start);
        for loc_ref in &refs {
            if let SpecReference::Spec(qa) = &loc_ref.reference {
                let ref_str = qa.to_string();
                let (exists, _) = registry.reference_exists(&ref_str);
                let bl = KbBacklink {
                    kb_file: rel_path.clone(),
                    spec_id: ref_str,
                    line: loc_ref.line,
                    source: BacklinkSource::Body,
                };
                if exists {
                    backlinks.push(bl);
                } else {
                    dangling.push(bl);
                }
            }
        }
    }

    // Group backlinks by spec ID
    let mut by_spec: BTreeMap<String, Vec<&KbBacklink>> = BTreeMap::new();
    for bl in &backlinks {
        by_spec.entry(bl.spec_id.clone()).or_default().push(bl);
    }

    // Report
    if by_spec.is_empty() && dangling.is_empty() {
        println!("No knowledge-base references to specs found.");
        return Ok(());
    }

    if !by_spec.is_empty() {
        println!("Backlinks from knowledge base → specs:\n");
        for (spec_id, refs) in &by_spec {
            println!("  {spec_id}");
            for bl in refs {
                let loc = match bl.source {
                    BacklinkSource::Frontmatter => format!("  {} (frontmatter)", bl.kb_file),
                    BacklinkSource::Body => format!("  {}:{}", bl.kb_file, bl.line),
                };
                println!("    ← {loc}");
            }
        }
    }

    if !dangling.is_empty() {
        println!("\nDangling references (spec not found):\n");
        for bl in &dangling {
            let loc = match bl.source {
                BacklinkSource::Frontmatter => format!("{} (frontmatter)", bl.kb_file),
                BacklinkSource::Body => format!("{}:{}", bl.kb_file, bl.line),
            };
            println!("  {loc} → {} (not found)", bl.spec_id);
        }
    }

    Ok(())
}

/// Extract spec IDs from a `specs:` YAML frontmatter field.
fn extract_specs_frontmatter(content: &str) -> Option<Vec<String>> {
    if !content.starts_with("---") {
        return None;
    }
    let rest = &content[3..];
    let end = rest.find("\n---")?;
    let yaml = &rest[..end];

    // Simple line-by-line parsing for `specs:` list
    let mut in_specs = false;
    let mut ids = Vec::new();

    for line in yaml.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("specs:") {
            in_specs = true;
            // Check for inline list: specs: [REQ:foo, INV:bar]
            let after = trimmed.strip_prefix("specs:").unwrap().trim();
            if after.starts_with('[') && after.ends_with(']') {
                let inner = &after[1..after.len() - 1];
                for item in inner.split(',') {
                    let id = item.trim().trim_matches('"').trim_matches('\'');
                    if !id.is_empty() {
                        ids.push(id.to_string());
                    }
                }
                in_specs = false;
            }
        } else if in_specs {
            if trimmed.starts_with("- ") {
                let id = trimmed
                    .strip_prefix("- ")
                    .unwrap()
                    .trim()
                    .trim_matches('"')
                    .trim_matches('\'');
                if !id.is_empty() {
                    ids.push(id.to_string());
                }
            } else if !trimmed.is_empty() {
                // End of the list
                in_specs = false;
            }
        }
    }

    if ids.is_empty() {
        None
    } else {
        Some(ids)
    }
}

/// Skip YAML frontmatter and return the body portion.
fn skip_frontmatter(content: &str) -> &str {
    if !content.starts_with("---") {
        return content;
    }
    let rest = &content[3..];
    if let Some(end) = rest.find("\n---") {
        let after = end + 4; // skip "\n---"
        if after < rest.len() {
            &rest[after..]
        } else {
            ""
        }
    } else {
        content
    }
}
